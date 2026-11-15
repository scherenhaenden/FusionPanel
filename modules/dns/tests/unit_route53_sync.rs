#![forbid(unsafe_code)]

use dns::route53_sync::*;

#[test]
fn test_txt_value_quoting() {
    assert_eq!(Route53Formatter::quote_txt_value("hello"), "\"hello\"");
    assert_eq!(
        Route53Formatter::quote_txt_value("hello world"),
        "\"hello world\""
    );
    assert_eq!(
        Route53Formatter::quote_txt_value("hello \"world\""),
        "\"hello \\\"world\\\"\""
    );

    let long_str = "a".repeat(300);
    let expected = format!("\"{}\" \"{}\"", "a".repeat(255), "a".repeat(45));
    assert_eq!(Route53Formatter::quote_txt_value(&long_str), expected);
}

#[test]
fn test_to_xml() {
    let request = Route53ChangeResourceRecordSetsRequest {
        change_batch: Route53ChangeBatch {
            changes: vec![Route53Change {
                action: Route53Action::CREATE,
                resource_record_set: Route53ResourceRecordSet {
                    name: "example.com.".to_string(),
                    record_type: "A".to_string(),
                    ttl: Some(300),
                    resource_records: vec![Route53ResourceRecord::new("1.2.3.4")],
                },
            }],
            comment: Some("Test comment <&>".to_string()),
        },
    };

    let xml = Route53Formatter::to_xml(&request);
    assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<Comment>Test comment &lt;&amp;&gt;</Comment>"));
    assert!(xml.contains("<Action>CREATE</Action>"));
    assert!(xml.contains("<Name>example.com.</Name>"));
    assert!(xml.contains("<Type>A</Type>"));
    assert!(xml.contains("<TTL>300</TTL>"));
    assert!(xml.contains("<Value>1.2.3.4</Value>"));
}

#[test]
fn test_to_json() {
    let request = Route53ChangeResourceRecordSetsRequest {
        change_batch: Route53ChangeBatch {
            changes: vec![Route53Change {
                action: Route53Action::DELETE,
                resource_record_set: Route53ResourceRecordSet {
                    name: "example.com.".to_string(),
                    record_type: "A".to_string(),
                    ttl: Some(300),
                    resource_records: vec![Route53ResourceRecord::new("1.2.3.4")],
                },
            }],
            comment: None,
        },
    };

    let json = Route53Formatter::to_json(&request);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["ChangeBatch"]["Changes"][0]["Action"], "DELETE");
    assert_eq!(
        parsed["ChangeBatch"]["Changes"][0]["ResourceRecordSet"]["Name"],
        "example.com."
    );
    assert_eq!(
        parsed["ChangeBatch"]["Changes"][0]["ResourceRecordSet"]["Type"],
        "A"
    );
    assert_eq!(
        parsed["ChangeBatch"]["Changes"][0]["ResourceRecordSet"]["TTL"],
        300
    );
    assert_eq!(
        parsed["ChangeBatch"]["Changes"][0]["ResourceRecordSet"]["ResourceRecords"][0]["Value"],
        "1.2.3.4"
    );
}

#[test]
fn test_batch_changes_limit() {
    // Let's create a large number of changes and verify that:
    // 1) All changes are present in the batches
    // 2) No batch exceeds max changes, max records, or max payload size.
    let mut changes = Vec::new();
    for i in 0..2005 {
        changes.push(Route53Change {
            action: Route53Action::UPSERT,
            resource_record_set: Route53ResourceRecordSet {
                name: format!("{}.example.com.", i),
                record_type: "A".to_string(),
                ttl: Some(300),
                resource_records: vec![
                    Route53ResourceRecord::new("1.2.3.4"),
                    Route53ResourceRecord::new("1.2.3.5"),
                ],
            },
        });
    }

    let batches = Route53Formatter::batch_changes(changes.clone(), None);

    let mut total_changes_in_batches = 0;

    for batch in batches {
        let req = Route53ChangeResourceRecordSetsRequest {
            change_batch: batch.change_batch.clone(),
        };

        let batch_changes = &batch.change_batch.changes;

        // Limits checks
        assert!(batch_changes.len() <= MAX_CHANGES_PER_BATCH);

        let total_records: usize = batch_changes
            .iter()
            .map(|c| c.resource_record_set.resource_records.len())
            .sum();
        assert!(total_records <= MAX_RECORDS_PER_BATCH);

        let xml_len = Route53Formatter::to_xml(&req).len();
        let json_len = Route53Formatter::to_json(&req).len();
        assert!(xml_len <= MAX_PAYLOAD_BYTES);
        assert!(json_len <= MAX_PAYLOAD_BYTES);

        total_changes_in_batches += batch_changes.len();
    }

    assert_eq!(total_changes_in_batches, 2005);
}
