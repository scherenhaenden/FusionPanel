#![forbid(unsafe_code)]

#[path = "../src/http2_stream_graph.rs"]
pub mod http2_stream_graph;

use http2_stream_graph::Http2StreamGraph;

#[test]
fn test_graph_initialization() {
    let graph = Http2StreamGraph::new();
    assert_eq!(graph.nodes.len(), 1);
    let root = graph.nodes.get(&0).unwrap();
    assert_eq!(root.stream_id, 0);
    assert_eq!(root.parent_stream_id, 0);
    assert_eq!(root.children.len(), 0);
}

#[test]
fn test_add_stream_non_exclusive() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 16, false, None);
    graph.add_stream(3, 0, 16, false, None);

    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.nodes.get(&0).unwrap().children, vec![1, 3]);
    assert_eq!(graph.nodes.get(&1).unwrap().parent_stream_id, 0);
    assert_eq!(graph.nodes.get(&3).unwrap().parent_stream_id, 0);
}

#[test]
fn test_add_stream_exclusive() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 16, false, None);
    graph.add_stream(3, 0, 16, false, None);

    // Stream 5 is added as exclusive child of 0
    graph.add_stream(5, 0, 16, true, None);

    let root = graph.nodes.get(&0).unwrap();
    assert_eq!(root.children, vec![5]);

    let stream_5 = graph.nodes.get(&5).unwrap();
    assert_eq!(stream_5.parent_stream_id, 0);
    assert_eq!(stream_5.children, vec![1, 3]);

    assert_eq!(graph.nodes.get(&1).unwrap().parent_stream_id, 5);
    assert_eq!(graph.nodes.get(&3).unwrap().parent_stream_id, 5);
}

#[test]
fn test_add_stream_unknown_parent() {
    let mut graph = Http2StreamGraph::new();
    // Parent 99 doesn't exist, should default to 0
    graph.add_stream(1, 99, 16, false, None);

    assert_eq!(graph.nodes.get(&1).unwrap().parent_stream_id, 0);
    assert_eq!(graph.nodes.get(&0).unwrap().children, vec![1]);
}

#[test]
fn test_reparent_stream_non_exclusive() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 16, false, None);
    graph.add_stream(3, 0, 16, false, None);
    graph.add_stream(5, 1, 16, false, None);

    // Reparent 5 to 3
    graph.reparent_stream(5, 3, false);

    assert_eq!(graph.nodes.get(&1).unwrap().children, vec![]);
    assert_eq!(graph.nodes.get(&3).unwrap().children, vec![5]);
    assert_eq!(graph.nodes.get(&5).unwrap().parent_stream_id, 3);
}

#[test]
fn test_reparent_stream_exclusive() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 16, false, None);
    graph.add_stream(3, 1, 16, false, None);
    graph.add_stream(5, 0, 16, false, None);

    // Stream 1 has child 3
    // Stream 5 is sibling to 1
    // Reparent 1 to 5 exclusively
    graph.reparent_stream(1, 5, true);

    assert_eq!(graph.nodes.get(&0).unwrap().children, vec![5]);
    assert_eq!(graph.nodes.get(&5).unwrap().children, vec![1]);
    assert_eq!(graph.nodes.get(&1).unwrap().parent_stream_id, 5);
    assert_eq!(graph.nodes.get(&1).unwrap().children, vec![3]);
}

#[test]
fn test_reparent_stream_cycle() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 16, false, None);
    graph.add_stream(3, 1, 16, false, None);
    graph.add_stream(5, 3, 16, false, None);

    // 0 -> 1 -> 3 -> 5
    // Reparent 1 to 5 (creates cycle)
    graph.reparent_stream(1, 5, false);

    // 5 moves to 1's old parent (0)
    // 1 moves to 5
    // 0 -> 5 -> 1 -> 3

    assert_eq!(graph.nodes.get(&0).unwrap().children, vec![5]);
    assert_eq!(graph.nodes.get(&5).unwrap().parent_stream_id, 0);
    assert_eq!(graph.nodes.get(&5).unwrap().children, vec![1]);
    assert_eq!(graph.nodes.get(&1).unwrap().parent_stream_id, 5);
    assert_eq!(graph.nodes.get(&1).unwrap().children, vec![3]);
    assert_eq!(graph.nodes.get(&3).unwrap().parent_stream_id, 1);
}

#[test]
fn test_generate_nginx_push_directives() {
    let mut graph = Http2StreamGraph::new();

    // High priority JS and CSS
    graph.add_stream(1, 0, 255, false, Some("/core.js".to_string()));
    graph.add_stream(3, 0, 200, false, Some("/main.css".to_string()));

    // Deeper dependencies
    graph.add_stream(5, 1, 100, false, Some("/util.js".to_string()));
    graph.add_stream(7, 3, 50, false, Some("/font.woff2".to_string())); // Not CSS/JS, shouldn't push

    // Same priority as /util.js but deeper
    graph.add_stream(9, 5, 100, false, Some("/deep.js".to_string()));

    let pushes = graph.generate_nginx_push_directives(10);

    // Should contain CSS/JS but not WOFF2, ordered by depth then weight
    assert_eq!(pushes.len(), 4);
    assert_eq!(pushes[0], "http2_push /core.js;");
    assert_eq!(pushes[1], "http2_push /main.css;");
    assert_eq!(pushes[2], "http2_push /util.js;");
    assert_eq!(pushes[3], "http2_push /deep.js;");
}

#[test]
fn test_generate_nginx_push_directives_limit() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 100, false, Some("/1.css".to_string()));
    graph.add_stream(3, 0, 90, false, Some("/2.css".to_string()));
    graph.add_stream(5, 0, 80, false, Some("/3.css".to_string()));

    let pushes = graph.generate_nginx_push_directives(2);

    assert_eq!(pushes.len(), 2);
    assert_eq!(pushes[0], "http2_push /1.css;");
    assert_eq!(pushes[1], "http2_push /2.css;");
}

#[test]
fn test_generate_nginx_push_directives_duplicates() {
    let mut graph = Http2StreamGraph::new();
    graph.add_stream(1, 0, 100, false, Some("/shared.css".to_string()));
    graph.add_stream(3, 0, 90, false, Some("/shared.css".to_string()));

    let pushes = graph.generate_nginx_push_directives(10);

    assert_eq!(pushes.len(), 1);
    assert_eq!(pushes[0], "http2_push /shared.css;");
}
