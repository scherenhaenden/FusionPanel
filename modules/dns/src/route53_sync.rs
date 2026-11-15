#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const MAX_CHANGES_PER_BATCH: usize = 1000;
pub const MAX_RECORDS_PER_BATCH: usize = 1000;
pub const MAX_PAYLOAD_BYTES: usize = 32000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Route53Action {
    CREATE,
    DELETE,
    UPSERT,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route53ResourceRecord {
    #[serde(rename = "Value")]
    pub value: String,
}

impl Route53ResourceRecord {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route53ResourceRecordSet {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub record_type: String,
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(rename = "ResourceRecords")]
    pub resource_records: Vec<Route53ResourceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route53Change {
    #[serde(rename = "Action")]
    pub action: Route53Action,
    #[serde(rename = "ResourceRecordSet")]
    pub resource_record_set: Route53ResourceRecordSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route53ChangeBatch {
    #[serde(rename = "Changes")]
    pub changes: Vec<Route53Change>,
    #[serde(rename = "Comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Route53ChangeResourceRecordSetsRequest {
    #[serde(rename = "ChangeBatch")]
    pub change_batch: Route53ChangeBatch,
}

pub struct Route53Formatter;

impl Route53Formatter {
    /// Correctly quotes and escapes TXT record values.
    /// Values longer than 255 bytes are chunked into multiple quoted strings separated by spaces.
    pub fn quote_txt_value(value: &str) -> String {
        let escaped = value.replace('"', "\\\"");
        let mut result = String::new();
        let mut current_chunk = String::new();

        for ch in escaped.chars() {
            if current_chunk.len() + ch.len_utf8() > 255 {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push('"');
                result.push_str(&current_chunk);
                result.push('"');
                current_chunk.clear();
            }
            current_chunk.push(ch);
        }

        if !current_chunk.is_empty() || result.is_empty() {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push('"');
            result.push_str(&current_chunk);
            result.push('"');
        }

        result
    }

    /// Serializes a request to JSON format.
    pub fn to_json(request: &Route53ChangeResourceRecordSetsRequest) -> String {
        serde_json::to_string(request).unwrap_or_default()
    }

    /// Serializes a request to XML format.
    pub fn to_xml(request: &Route53ChangeResourceRecordSetsRequest) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<ChangeResourceRecordSetsRequest xmlns=\"https://route53.amazonaws.com/doc/2013-04-01/\">\n");
        xml.push_str("  <ChangeBatch>\n");
        if let Some(comment) = &request.change_batch.comment {
            xml.push_str(&format!(
                "    <Comment>{}</Comment>\n",
                Self::escape_xml(comment)
            ));
        }
        xml.push_str("    <Changes>\n");
        for change in &request.change_batch.changes {
            xml.push_str("      <Change>\n");
            xml.push_str(&format!("        <Action>{:?}</Action>\n", change.action));
            xml.push_str("        <ResourceRecordSet>\n");
            xml.push_str(&format!(
                "          <Name>{}</Name>\n",
                Self::escape_xml(&change.resource_record_set.name)
            ));
            xml.push_str(&format!(
                "          <Type>{}</Type>\n",
                Self::escape_xml(&change.resource_record_set.record_type)
            ));
            if let Some(ttl) = change.resource_record_set.ttl {
                xml.push_str(&format!("          <TTL>{}</TTL>\n", ttl));
            }
            xml.push_str("          <ResourceRecords>\n");
            for record in &change.resource_record_set.resource_records {
                xml.push_str("            <ResourceRecord>\n");
                xml.push_str(&format!(
                    "              <Value>{}</Value>\n",
                    Self::escape_xml(&record.value)
                ));
                xml.push_str("            </ResourceRecord>\n");
            }
            xml.push_str("          </ResourceRecords>\n");
            xml.push_str("        </ResourceRecordSet>\n");
            xml.push_str("      </Change>\n");
        }
        xml.push_str("    </Changes>\n");
        xml.push_str("  </ChangeBatch>\n");
        xml.push_str("</ChangeResourceRecordSetsRequest>");
        xml
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Batches a list of changes into multiple requests, ensuring limits:
    /// - Max 1000 changes per batch.
    /// - Max 1000 total resource records per batch.
    /// - Max 32000 bytes per batch (payload size in either JSON or XML).
    pub fn batch_changes(
        changes: Vec<Route53Change>,
        comment: Option<String>,
    ) -> Vec<Route53ChangeResourceRecordSetsRequest> {
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();

        for change in changes {
            current_batch.push(change.clone());

            let request = Route53ChangeResourceRecordSetsRequest {
                change_batch: Route53ChangeBatch {
                    changes: current_batch.clone(),
                    comment: comment.clone(),
                },
            };

            let xml_len = Self::to_xml(&request).len();
            let json_len = Self::to_json(&request).len();
            let max_payload_len = xml_len.max(json_len);

            let total_records: usize = current_batch
                .iter()
                .map(|c| c.resource_record_set.resource_records.len())
                .sum();

            let over_size = max_payload_len > MAX_PAYLOAD_BYTES;
            let over_records = total_records > MAX_RECORDS_PER_BATCH;
            let over_changes = current_batch.len() > MAX_CHANGES_PER_BATCH;

            if (over_size || over_records || over_changes) && current_batch.len() > 1 {
                current_batch.pop();
                batches.push(Route53ChangeResourceRecordSetsRequest {
                    change_batch: Route53ChangeBatch {
                        changes: current_batch.clone(),
                        comment: comment.clone(),
                    },
                });

                current_batch.clear();
                current_batch.push(change);
            }
        }

        if !current_batch.is_empty() {
            batches.push(Route53ChangeResourceRecordSetsRequest {
                change_batch: Route53ChangeBatch {
                    changes: current_batch,
                    comment: comment.clone(),
                },
            });
        }

        batches
    }
}
