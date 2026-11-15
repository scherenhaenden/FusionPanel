#![forbid(unsafe_code)]

use backup_destinations::s3_version_pruner::*;
use chrono::Utc;

#[test]
fn test_parse_list_object_versions_xml() {
    let xml = r#"
<?xml version="1.0" encoding="UTF-8"?>
<ListVersionsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Name>my-bucket</Name>
    <Prefix></Prefix>
    <KeyMarker></KeyMarker>
    <VersionIdMarker></VersionIdMarker>
    <MaxKeys>1000</MaxKeys>
    <IsTruncated>false</IsTruncated>
    <Version>
        <Key>file1.txt</Key>
        <VersionId>v1</VersionId>
        <IsLatest>true</IsLatest>
        <LastModified>2023-01-02T10:00:00Z</LastModified>
        <ETag>"abc"</ETag>
        <Size>10</Size>
        <StorageClass>STANDARD</StorageClass>
        <Owner>
            <ID>owner1</ID>
        </Owner>
    </Version>
    <DeleteMarker>
        <Key>file1.txt</Key>
        <VersionId>v2</VersionId>
        <IsLatest>false</IsLatest>
        <LastModified>2023-01-01T10:00:00Z</LastModified>
        <Owner>
            <ID>owner1</ID>
        </Owner>
    </DeleteMarker>
    <Version>
        <Key>test&amp;amp;.txt</Key>
        <VersionId>v3</VersionId>
        <IsLatest>true</IsLatest>
        <LastModified>2023-01-03T10:00:00Z</LastModified>
    </Version>
</ListVersionsResult>
"#;

    let versions = parse_list_object_versions_xml(xml).unwrap();
    assert_eq!(versions.len(), 3);

    assert_eq!(versions[0].key, "file1.txt");
    assert_eq!(versions[0].version_id, "v1");
    assert_eq!(versions[0].is_latest, true);
    assert_eq!(versions[0].is_delete_marker, false);

    assert_eq!(versions[1].key, "file1.txt");
    assert_eq!(versions[1].version_id, "v2");
    assert_eq!(versions[1].is_latest, false);
    assert_eq!(versions[1].is_delete_marker, true);

    assert_eq!(versions[2].key, "test&amp;.txt");
    assert_eq!(versions[2].version_id, "v3");
    assert_eq!(versions[2].is_latest, true);
    assert_eq!(versions[2].is_delete_marker, false);
}

#[test]
fn test_identify_prune_targets() {
    let now = chrono::DateTime::parse_from_rfc3339("2023-01-15T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let versions = vec![
        // file1: delete marker is latest, noncurrent version is older than retention (7 days)
        ObjectVersion {
            key: "file1".to_string(),
            version_id: "v1".to_string(),
            is_latest: false,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-01T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: false,
        },
        ObjectVersion {
            key: "file1".to_string(),
            version_id: "v2".to_string(),
            is_latest: true,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-05T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: true,
        },
        // file2: delete marker is latest, noncurrent version is NOT older than retention
        ObjectVersion {
            key: "file2".to_string(),
            version_id: "v1".to_string(),
            is_latest: false,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-10T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: false,
        },
        ObjectVersion {
            key: "file2".to_string(),
            version_id: "v2".to_string(),
            is_latest: true,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-12T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: true,
        },
        // file3: normal version is latest, noncurrent version is older than retention
        ObjectVersion {
            key: "file3".to_string(),
            version_id: "v1".to_string(),
            is_latest: false,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-01T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: false,
        },
        ObjectVersion {
            key: "file3".to_string(),
            version_id: "v2".to_string(),
            is_latest: true,
            last_modified: chrono::DateTime::parse_from_rfc3339("2023-01-14T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            is_delete_marker: false,
        },
    ];

    let prune_targets = identify_prune_targets(&versions, 7, now);

    // file1's noncurrent version is pruned. Since all noncurrent versions are pruned, the latest delete marker is also pruned.
    // file2's noncurrent version is NOT pruned. Since not all noncurrent versions are pruned, the latest delete marker is NOT pruned.
    // file3's noncurrent version is pruned. The latest version is not a delete marker, so it is NOT pruned.

    assert_eq!(prune_targets.len(), 3);

    let k1 = prune_targets.iter().find(|p| p.key == "file1" && p.version_id == "v1");
    assert!(k1.is_some());
    let k1dm = prune_targets.iter().find(|p| p.key == "file1" && p.version_id == "v2");
    assert!(k1dm.is_some());

    let k3 = prune_targets.iter().find(|p| p.key == "file3" && p.version_id == "v1");
    assert!(k3.is_some());
}

#[test]
fn test_generate_delete_objects_payloads() {
    let mut targets = Vec::new();
    let now = Utc::now();
    for i in 0..1500 {
        targets.push(ObjectVersion {
            key: format!("file&{}.txt", i),
            version_id: format!("v{}", i),
            is_latest: false,
            last_modified: now,
            is_delete_marker: false,
        });
    }

    let payloads = generate_delete_objects_payloads(&targets);
    assert_eq!(payloads.len(), 2);

    let first_payload = &payloads[0];
    assert!(first_payload.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(first_payload.contains("<Delete>"));
    assert!(first_payload.contains("<Key>file&amp;0.txt</Key>"));
    assert!(first_payload.contains("<VersionId>v0</VersionId>"));
    assert!(first_payload.contains("</Delete>"));

    // Check that we have 1000 objects in the first payload
    let obj_count = first_payload.matches("<Object>").count();
    assert_eq!(obj_count, 1000);

    let second_payload = &payloads[1];
    let obj_count2 = second_payload.matches("<Object>").count();
    assert_eq!(obj_count2, 500);
}
