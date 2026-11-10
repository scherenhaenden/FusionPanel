#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectVersion {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub last_modified: DateTime<Utc>,
    pub is_delete_marker: bool,
}

pub fn parse_list_object_versions_xml(xml: &str) -> Result<Vec<ObjectVersion>, &'static str> {
    let mut versions = Vec::new();

    let mut remaining = xml;
    while !remaining.is_empty() {
        let v_start = remaining.find("<Version>");
        let d_start = remaining.find("<DeleteMarker>");

        match (v_start, d_start) {
            (None, None) => break,
            (Some(v), None) => {
                remaining = &remaining[v..];
                let end = remaining.find("</Version>").ok_or("Missing </Version>")?;
                versions.push(parse_element(&remaining[..end + 10], false)?);
                remaining = &remaining[end + 10..];
            }
            (None, Some(d)) => {
                remaining = &remaining[d..];
                let end = remaining.find("</DeleteMarker>").ok_or("Missing </DeleteMarker>")?;
                versions.push(parse_element(&remaining[..end + 15], true)?);
                remaining = &remaining[end + 15..];
            }
            (Some(v), Some(d)) => {
                if v < d {
                    remaining = &remaining[v..];
                    let end = remaining.find("</Version>").ok_or("Missing </Version>")?;
                    versions.push(parse_element(&remaining[..end + 10], false)?);
                    remaining = &remaining[end + 10..];
                } else {
                    remaining = &remaining[d..];
                    let end = remaining.find("</DeleteMarker>").ok_or("Missing </DeleteMarker>")?;
                    versions.push(parse_element(&remaining[..end + 15], true)?);
                    remaining = &remaining[end + 15..];
                }
            }
        }
    }

    Ok(versions)
}

fn parse_element(xml: &str, is_delete_marker: bool) -> Result<ObjectVersion, &'static str> {
    let key = extract_tag(xml, "Key")?;
    let version_id = extract_tag(xml, "VersionId")?;
    let is_latest = extract_tag(xml, "IsLatest")? == "true";
    let last_modified_str = extract_tag(xml, "LastModified")?;
    let last_modified = chrono::DateTime::parse_from_rfc3339(last_modified_str)
        .map_err(|_| "Invalid LastModified")?
        .with_timezone(&Utc);

    Ok(ObjectVersion {
        key: unescape_xml(key),
        version_id: unescape_xml(version_id),
        is_latest,
        last_modified,
        is_delete_marker,
    })
}

fn extract_tag<'a>(xml: &'a str, tag: &str) -> Result<&'a str, &'static str> {
    let open_tag = format!("<{}>", tag);
    let close_tag = format!("</{}>", tag);

    let start = xml.find(&open_tag).ok_or("Missing tag")? + open_tag.len();
    let end_search = &xml[start..];
    let end = end_search.find(&close_tag).ok_or("Missing close tag")? + start;
    Ok(&xml[start..end])
}

fn unescape_xml(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            while let Some(&next_c) = chars.peek() {
                chars.next();
                if next_c == ';' {
                    break;
                }
                entity.push(next_c);
            }
            match entity.as_str() {
                "lt" => res.push('<'),
                "gt" => res.push('>'),
                "quot" => res.push('"'),
                "apos" => res.push('\''),
                "amp" => res.push('&'),
                _ => {
                    res.push('&');
                    res.push_str(&entity);
                    res.push(';');
                }
            }
        } else {
            res.push(c);
        }
    }
    res
}

fn escape_xml(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => res.push_str("&lt;"),
            '>' => res.push_str("&gt;"),
            '"' => res.push_str("&quot;"),
            '\'' => res.push_str("&apos;"),
            '&' => res.push_str("&amp;"),
            _ => res.push(c),
        }
    }
    res
}

pub fn identify_prune_targets(
    versions: &[ObjectVersion],
    retention_days: i64,
    now: DateTime<Utc>,
) -> Vec<ObjectVersion> {
    let mut by_key: HashMap<String, Vec<ObjectVersion>> = HashMap::new();
    for v in versions {
        by_key.entry(v.key.clone()).or_default().push(v.clone());
    }

    let mut prune_targets = Vec::new();
    let retention_duration = chrono::Duration::try_days(retention_days).unwrap();

    for (_, key_versions) in by_key {
        let mut all_noncurrent_pruned = true;
        let mut local_prune = Vec::new();

        let mut latest_is_delete_marker = false;
        let mut latest_version = None;

        for v in &key_versions {
            if !v.is_latest {
                if now.signed_duration_since(v.last_modified) > retention_duration {
                    local_prune.push(v.clone());
                } else {
                    all_noncurrent_pruned = false;
                }
            } else {
                latest_version = Some(v.clone());
                if v.is_delete_marker {
                    latest_is_delete_marker = true;
                }
            }
        }

        if latest_is_delete_marker && all_noncurrent_pruned {
            if let Some(v) = latest_version {
                local_prune.push(v);
            }
        }

        prune_targets.extend(local_prune);
    }

    prune_targets.sort_by(|a, b| a.key.cmp(&b.key).then(a.version_id.cmp(&b.version_id)));
    prune_targets
}

pub fn generate_delete_objects_payloads(targets: &[ObjectVersion]) -> Vec<String> {
    let mut payloads = Vec::new();

    for chunk in targets.chunks(1000) {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<Delete>\n");
        for t in chunk {
            xml.push_str("  <Object>\n");
            xml.push_str(&format!("    <Key>{}</Key>\n", escape_xml(&t.key)));
            xml.push_str(&format!("    <VersionId>{}</VersionId>\n", escape_xml(&t.version_id)));
            xml.push_str("  </Object>\n");
        }
        xml.push_str("</Delete>");
        payloads.push(xml);
    }

    payloads
}
