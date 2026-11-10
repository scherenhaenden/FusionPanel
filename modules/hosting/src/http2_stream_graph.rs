#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamNode {
    pub stream_id: u32,
    pub parent_stream_id: u32,
    pub weight: u8,
    pub children: Vec<u32>,
    pub uri: Option<String>,
}

#[derive(Debug, Default)]
pub struct Http2StreamGraph {
    pub nodes: HashMap<u32, StreamNode>,
}

impl Http2StreamGraph {
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        // Root node (stream 0)
        nodes.insert(0, StreamNode {
            stream_id: 0,
            parent_stream_id: 0,
            weight: 16,
            children: Vec::new(),
            uri: None,
        });
        Self { nodes }
    }

    pub fn add_stream(&mut self, stream_id: u32, parent_stream_id: u32, weight: u8, exclusive: bool, uri: Option<String>) {
        if stream_id == 0 || self.nodes.contains_key(&stream_id) {
            return;
        }

        let actual_parent = if self.nodes.contains_key(&parent_stream_id) {
            parent_stream_id
        } else {
            0
        };

        self.nodes.insert(stream_id, StreamNode {
            stream_id,
            parent_stream_id: actual_parent,
            weight,
            children: Vec::new(),
            uri,
        });

        if exclusive {
            let previous_children = std::mem::replace(&mut self.nodes.get_mut(&actual_parent).unwrap().children, vec![stream_id]);
            self.nodes.get_mut(&stream_id).unwrap().children.extend(previous_children.iter().copied());
            for child_id in previous_children {
                self.nodes.get_mut(&child_id).unwrap().parent_stream_id = stream_id;
            }
        } else {
            self.nodes.get_mut(&actual_parent).unwrap().children.push(stream_id);
        }
    }

    fn reparent_stream_internal(&mut self, stream_id: u32, new_parent_id: u32, exclusive: bool) {
        let old_parent_id = self.nodes.get(&stream_id).unwrap().parent_stream_id;

        // Remove from old parent
        self.nodes.get_mut(&old_parent_id).unwrap().children.retain(|&id| id != stream_id);

        let mut previous_children = Vec::new();
        if exclusive {
            previous_children = std::mem::replace(&mut self.nodes.get_mut(&new_parent_id).unwrap().children, vec![stream_id]);
        } else {
            self.nodes.get_mut(&new_parent_id).unwrap().children.push(stream_id);
        }

        let stream = self.nodes.get_mut(&stream_id).unwrap();
        stream.parent_stream_id = new_parent_id;
        stream.children.extend(previous_children.iter().copied());

        for child_id in &previous_children {
            self.nodes.get_mut(child_id).unwrap().parent_stream_id = stream_id;
        }
    }

    pub fn reparent_stream(&mut self, stream_id: u32, new_parent_id: u32, exclusive: bool) {
        if stream_id == 0 || stream_id == new_parent_id || !self.nodes.contains_key(&stream_id) {
            return;
        }

        let actual_parent = if self.nodes.contains_key(&new_parent_id) {
            new_parent_id
        } else {
            0
        };

        // Check for cycle
        let mut curr = actual_parent;
        let mut is_cycle = false;
        while curr != 0 {
            if curr == stream_id {
                is_cycle = true;
                break;
            }
            curr = self.nodes.get(&curr).unwrap().parent_stream_id;
        }

        if is_cycle {
            let stream_prev_parent = self.nodes.get(&stream_id).unwrap().parent_stream_id;
            self.reparent_stream_internal(actual_parent, stream_prev_parent, false);
        }

        self.reparent_stream_internal(stream_id, actual_parent, exclusive);
    }

    pub fn generate_nginx_push_directives(&self, max_pushes: usize) -> Vec<String> {
        let mut candidates: Vec<&StreamNode> = self.nodes.values()
            .filter(|n| n.uri.is_some())
            .collect();

        candidates.sort_by(|a, b| {
            let depth_a = self.get_depth(a.stream_id);
            let depth_b = self.get_depth(b.stream_id);

            if depth_a != depth_b {
                depth_a.cmp(&depth_b)
            } else {
                b.weight.cmp(&a.weight)
            }
        });

        let mut pushes = Vec::new();
        let mut pushed_uris = HashSet::new();

        for candidate in candidates {
            if pushes.len() >= max_pushes {
                break;
            }
            if let Some(uri) = &candidate.uri {
                if !pushed_uris.contains(uri) {
                    let is_critical = uri.ends_with(".css") || uri.ends_with(".js");

                    if is_critical {
                        pushes.push(format!("http2_push {};", uri));
                        pushed_uris.insert(uri.clone());
                    }
                }
            }
        }

        pushes
    }

    fn get_depth(&self, mut stream_id: u32) -> usize {
        let mut depth = 0;
        while stream_id != 0 {
            if let Some(node) = self.nodes.get(&stream_id) {
                stream_id = node.parent_stream_id;
                depth += 1;
            } else {
                break;
            }
        }
        depth
    }
}
