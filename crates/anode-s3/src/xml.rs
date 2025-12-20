// S3 XML response generation
// This module handles XML serialization for S3 API responses

use crate::error::{S3Error, S3Result};
use crate::multipart::{CompletePart, ListPartsResult, ListUploadsResult, UploadedPart};
use anode_protocol::ObjectManifest;
use bytes::Bytes;

/// Information about a bucket
#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub name: String,
    pub creation_date: String,
}

/// Result of a ListObjectsV2 operation
#[derive(Debug, Clone)]
pub struct ListObjectsResult {
    pub name: String,
    pub prefix: Option<String>,
    pub max_keys: u32,
    pub is_truncated: bool,
    pub contents: Vec<ObjectInfo>,
    pub key_count: u32,
    pub continuation_token: Option<String>,
    pub next_continuation_token: Option<String>,
}

/// Information about an object
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    pub etag: String,
    pub storage_class: String,
}

/// Result of a delete operation
#[derive(Debug, Clone)]
pub struct DeleteResult {
    pub key: String,
    pub success: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// Result of a complete multipart upload
#[derive(Debug, Clone)]
pub struct CompleteMultipartResult {
    pub bucket: String,
    pub key: String,
    pub etag: String,
    pub location: String,
}

/// Generates XML for ListBuckets response
pub fn list_buckets_xml(buckets: &[BucketInfo]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Owner>
        <ID>anode-owner</ID>
        <DisplayName>anode</DisplayName>
    </Owner>
    <Buckets>"#);

    for bucket in buckets {
        xml.push_str(&format!(
            r#"
        <Bucket>
            <Name>{}</Name>
            <CreationDate>{}</CreationDate>
        </Bucket>"#,
            xml_escape(&bucket.name),
            xml_escape(&bucket.creation_date)
        ));
    }

    xml.push_str(
        r#"
    </Buckets>
</ListAllMyBucketsResult>"#,
    );

    xml
}

/// Generates XML for ListObjectsV2 response
pub fn list_objects_v2_xml(result: &ListObjectsResult) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);

    xml.push_str(&format!(
        r#"
    <Name>{}</Name>
    <KeyCount>{}</KeyCount>
    <MaxKeys>{}</MaxKeys>
    <IsTruncated>{}</IsTruncated>"#,
        xml_escape(&result.name),
        result.key_count,
        result.max_keys,
        result.is_truncated
    ));

    if let Some(prefix) = &result.prefix {
        xml.push_str(&format!(
            r#"
    <Prefix>{}</Prefix>"#,
            xml_escape(prefix)
        ));
    }

    if let Some(token) = &result.continuation_token {
        xml.push_str(&format!(
            r#"
    <ContinuationToken>{}</ContinuationToken>"#,
            xml_escape(token)
        ));
    }

    if let Some(next_token) = &result.next_continuation_token {
        xml.push_str(&format!(
            r#"
    <NextContinuationToken>{}</NextContinuationToken>"#,
            xml_escape(next_token)
        ));
    }

    for obj in &result.contents {
        xml.push_str(&format!(
            r#"
    <Contents>
        <Key>{}</Key>
        <LastModified>{}</LastModified>
        <ETag>{}</ETag>
        <Size>{}</Size>
        <StorageClass>{}</StorageClass>
    </Contents>"#,
            xml_escape(&obj.key),
            xml_escape(&obj.last_modified),
            xml_escape(&obj.etag),
            obj.size,
            xml_escape(&obj.storage_class)
        ));
    }

    xml.push_str("\n</ListBucketResult>");
    xml
}

/// Generates XML for error responses
pub fn error_xml(code: &str, message: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
</Error>"#,
        xml_escape(code),
        xml_escape(message)
    )
}

/// Generates XML for CompleteMultipartUpload response
pub fn complete_multipart_xml(result: &CompleteMultipartResult) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Location>{}</Location>
    <Bucket>{}</Bucket>
    <Key>{}</Key>
    <ETag>{}</ETag>
</CompleteMultipartUploadResult>"#,
        xml_escape(&result.location),
        xml_escape(&result.bucket),
        xml_escape(&result.key),
        xml_escape(&result.etag)
    )
}

/// Generates XML for InitiateMultipartUpload response
pub fn initiate_multipart_xml(bucket: &str, key: &str, upload_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>{}</Bucket>
    <Key>{}</Key>
    <UploadId>{}</UploadId>
</InitiateMultipartUploadResult>"#,
        xml_escape(bucket),
        xml_escape(key),
        xml_escape(upload_id)
    )
}

/// Generates XML for CopyObject response
pub fn copy_object_xml(etag: &str, last_modified: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CopyObjectResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <LastModified>{}</LastModified>
    <ETag>{}</ETag>
</CopyObjectResult>"#,
        xml_escape(last_modified),
        xml_escape(etag)
    )
}

/// Generates XML for DeleteObjects response
pub fn delete_objects_xml(results: &[DeleteResult]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );

    for result in results {
        if result.success {
            xml.push_str(&format!(
                r#"
    <Deleted>
        <Key>{}</Key>
    </Deleted>"#,
                xml_escape(&result.key)
            ));
        } else {
            xml.push_str(&format!(
                r#"
    <Error>
        <Key>{}</Key>
        <Code>{}</Code>
        <Message>{}</Message>
    </Error>"#,
                xml_escape(&result.key),
                xml_escape(result.error_code.as_deref().unwrap_or("InternalError")),
                xml_escape(result.error_message.as_deref().unwrap_or("Unknown error"))
            ));
        }
    }

    xml.push_str("\n</DeleteResult>");
    xml
}

/// Parses XML for DeleteObjects request
pub fn parse_delete_objects_xml(xml: &str) -> Result<DeleteObjectsRequest, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut objects = Vec::new();
    let mut quiet = false;
    let mut current_key: Option<String> = None;
    let mut current_version_id: Option<String> = None;
    let mut in_object = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"Object" => {
                        in_object = true;
                        current_key = None;
                        current_version_id = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"Object" => {
                        if let Some(key) = current_key.take() {
                            objects.push(DeleteObjectInfo {
                                key,
                                version_id: current_version_id.take(),
                            });
                        }
                        in_object = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|e| e.to_string())?.to_string();

                // Get the tag name from the reader's position
                if let Some(tag_name) = reader.decoder().decode(e.as_ref()).ok() {
                    // This is a simplification - we need to track the current element
                    // For now, we'll handle this differently
                }
            }
            Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"Quiet" => quiet = true,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parsing XML: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(DeleteObjectsRequest { objects, quiet })
}

/// Parses XML for DeleteObjects request (alternative implementation)
pub fn parse_delete_objects_request(xml_body: &str) -> Result<DeleteObjectsRequest, String> {
    // Simple regex-based parsing for now
    // In production, use a proper XML parser

    let mut objects = Vec::new();
    let quiet = xml_body.contains("<Quiet>true</Quiet>");

    // Find all <Object> blocks
    let mut start = 0;
    while let Some(obj_start) = xml_body[start..].find("<Object>") {
        let obj_start = start + obj_start;
        if let Some(obj_end) = xml_body[obj_start..].find("</Object>") {
            let obj_end = obj_start + obj_end + "</Object>".len();
            let obj_xml = &xml_body[obj_start..obj_end];

            // Extract Key
            if let Some(key) = extract_xml_tag(obj_xml, "Key") {
                let version_id = extract_xml_tag(obj_xml, "VersionId");
                objects.push(DeleteObjectInfo { key, version_id });
            }

            start = obj_end;
        } else {
            break;
        }
    }

    Ok(DeleteObjectsRequest { objects, quiet })
}

/// Helper function to extract content from an XML tag
fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)? + start;

    Some(xml[start..end].to_string())
}

/// Request to delete multiple objects
#[derive(Debug, Clone)]
pub struct DeleteObjectsRequest {
    pub objects: Vec<DeleteObjectInfo>,
    pub quiet: bool,
}

/// Object to delete
#[derive(Debug, Clone)]
pub struct DeleteObjectInfo {
    pub key: String,
    pub version_id: Option<String>,
}

/// Generates XML for batch delete response
pub fn batch_delete_xml(deleted: &[BatchDeletedObject], errors: &[BatchDeleteError]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );

    for obj in deleted {
        xml.push_str(&format!(
            r#"
    <Deleted>
        <Key>{}</Key>"#,
            xml_escape(&obj.key)
        ));

        if let Some(version_id) = &obj.version_id {
            xml.push_str(&format!(
                r#"
        <VersionId>{}</VersionId>"#,
                xml_escape(version_id)
            ));
        }

        if obj.delete_marker {
            xml.push_str(
                r#"
        <DeleteMarker>true</DeleteMarker>"#,
            );
        }

        if let Some(dm_version_id) = &obj.delete_marker_version_id {
            xml.push_str(&format!(
                r#"
        <DeleteMarkerVersionId>{}</DeleteMarkerVersionId>"#,
                xml_escape(dm_version_id)
            ));
        }

        xml.push_str(
            r#"
    </Deleted>"#,
        );
    }

    for error in errors {
        xml.push_str(&format!(
            r#"
    <Error>
        <Key>{}</Key>
        <Code>{}</Code>
        <Message>{}</Message>"#,
            xml_escape(&error.key),
            xml_escape(&error.code),
            xml_escape(&error.message)
        ));

        if let Some(version_id) = &error.version_id {
            xml.push_str(&format!(
                r#"
        <VersionId>{}</VersionId>"#,
                xml_escape(version_id)
            ));
        }

        xml.push_str(
            r#"
    </Error>"#,
        );
    }

    xml.push_str("\n</DeleteResult>");
    xml
}

/// Deleted object information for batch delete response
#[derive(Debug, Clone)]
pub struct BatchDeletedObject {
    pub key: String,
    pub version_id: Option<String>,
    pub delete_marker: bool,
    pub delete_marker_version_id: Option<String>,
}

/// Error information for batch delete response
#[derive(Debug, Clone)]
pub struct BatchDeleteError {
    pub key: String,
    pub version_id: Option<String>,
    pub code: String,
    pub message: String,
}

/// Converts an ObjectManifest to ObjectInfo
pub fn object_manifest_to_info(manifest: &ObjectManifest) -> ObjectInfo {
    ObjectInfo {
        key: manifest.object_key.key.clone(),
        size: manifest.size,
        last_modified: format_timestamp(manifest.modified_at),
        etag: format!("\"{}\"", manifest.etag),
        storage_class: "STANDARD".to_string(),
    }
}

/// Formats a Unix timestamp to ISO 8601 format
pub fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    // Format as ISO 8601: YYYY-MM-DDTHH:MM:SS.000Z
    // This is a simplified version; for production, use chrono or time crate
    format!("{:?}", datetime)
        .replace("SystemTime { tv_sec: ", "")
        .replace(", tv_nsec: 0 }", "")
        .split_once(' ')
        .map(|(secs, _)| {
            let secs: i64 = secs.parse().unwrap_or(0);
            // Simplified ISO 8601 format
            let days_since_epoch = secs / 86400;
            let year = 1970 + (days_since_epoch / 365);
            format!("{:04}-01-01T00:00:00.000Z", year)
        })
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string())
}

/// Generates XML for ListParts response
pub fn list_parts_xml(result: &ListPartsResult) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListPartsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>{}</Bucket>
    <Key>{}</Key>
    <UploadId>{}</UploadId>
    <StorageClass>STANDARD</StorageClass>
    <PartNumberMarker>0</PartNumberMarker>
    <MaxParts>1000</MaxParts>
    <IsTruncated>{}</IsTruncated>"#,
        xml_escape(&result.bucket),
        xml_escape(&result.key),
        xml_escape(&result.upload_id),
        result.is_truncated
    );

    if let Some(marker) = result.next_part_number_marker {
        xml.push_str(&format!(
            r#"
    <NextPartNumberMarker>{}</NextPartNumberMarker>"#,
            marker
        ));
    }

    for part in &result.parts {
        xml.push_str(&format!(
            r#"
    <Part>
        <PartNumber>{}</PartNumber>
        <LastModified>{}</LastModified>
        <ETag>"{}"</ETag>
        <Size>{}</Size>
    </Part>"#,
            part.part_number,
            part.uploaded_at.to_rfc3339(),
            xml_escape(&part.etag),
            part.size
        ));
    }

    xml.push_str("\n</ListPartsResult>");
    xml
}

/// Generates XML for ListMultipartUploads response
pub fn list_multipart_uploads_xml(result: &ListUploadsResult) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListMultipartUploadsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>{}</Bucket>
    <KeyMarker></KeyMarker>
    <UploadIdMarker></UploadIdMarker>
    <MaxUploads>1000</MaxUploads>
    <IsTruncated>{}</IsTruncated>"#,
        xml_escape(&result.bucket),
        result.is_truncated
    );

    if let Some(key_marker) = &result.next_key_marker {
        xml.push_str(&format!(
            r#"
    <NextKeyMarker>{}</NextKeyMarker>"#,
            xml_escape(key_marker)
        ));
    }

    if let Some(upload_id_marker) = &result.next_upload_id_marker {
        xml.push_str(&format!(
            r#"
    <NextUploadIdMarker>{}</NextUploadIdMarker>"#,
            xml_escape(upload_id_marker)
        ));
    }

    for upload in &result.uploads {
        xml.push_str(&format!(
            r#"
    <Upload>
        <Key>{}</Key>
        <UploadId>{}</UploadId>
        <Initiated>{}</Initiated>
        <StorageClass>STANDARD</StorageClass>
    </Upload>"#,
            xml_escape(&upload.key),
            xml_escape(&upload.upload_id),
            upload.initiated.to_rfc3339()
        ));
    }

    xml.push_str("\n</ListMultipartUploadsResult>");
    xml
}

/// Parses CompleteMultipartUpload XML request body
pub fn parse_complete_multipart_upload(body: &Bytes) -> S3Result<Vec<CompletePart>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let xml = std::str::from_utf8(body)
        .map_err(|e| S3Error::InvalidRequest(format!("Invalid UTF-8: {}", e)))?;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut parts = Vec::new();
    let mut current_part_number: Option<u32> = None;
    let mut current_etag: Option<String> = None;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"Part" => {
                        current_part_number = None;
                        current_etag = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"Part" => {
                        if let (Some(part_number), Some(etag)) = (current_part_number, current_etag.take()) {
                            parts.push(CompletePart {
                                part_number,
                                etag: etag.trim_matches('"').to_string(),
                            });
                        }
                    }
                    b"PartNumber" => {
                        // Part number will be captured in Text event
                    }
                    b"ETag" => {
                        // ETag will be captured in Text event
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape()
                    .map_err(|e| S3Error::InvalidRequest(format!("XML parse error: {}", e)))?
                    .to_string();

                // Determine what tag we're in by checking the reader state
                // This is a simplified approach - in production use proper state tracking
                if text.chars().all(|c| c.is_ascii_digit()) {
                    // Likely a part number
                    if let Ok(num) = text.parse::<u32>() {
                        current_part_number = Some(num);
                    }
                } else if !text.trim().is_empty() {
                    // Likely an ETag
                    current_etag = Some(text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(S3Error::InvalidRequest(format!("XML parse error: {}", e)));
            }
            _ => {}
        }

        buf.clear();
    }

    if parts.is_empty() {
        return Err(S3Error::InvalidRequest(
            "CompleteMultipartUpload request must contain at least one part".to_string(),
        ));
    }

    Ok(parts)
}

/// Escapes special XML characters
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        let result = xml_escape("test & <tag> \"quotes\" 'single'");
        assert_eq!(
            result,
            "test &amp; &lt;tag&gt; &quot;quotes&quot; &apos;single&apos;"
        );
    }

    #[test]
    fn test_list_buckets_xml() {
        let buckets = vec![
            BucketInfo {
                name: "bucket1".to_string(),
                creation_date: "2023-01-01T00:00:00.000Z".to_string(),
            },
            BucketInfo {
                name: "bucket2".to_string(),
                creation_date: "2023-01-02T00:00:00.000Z".to_string(),
            },
        ];

        let xml = list_buckets_xml(&buckets);
        assert!(xml.contains("<Name>bucket1</Name>"));
        assert!(xml.contains("<Name>bucket2</Name>"));
        assert!(xml.contains("<ListAllMyBucketsResult"));
    }

    #[test]
    fn test_list_objects_v2_xml() {
        let result = ListObjectsResult {
            name: "test-bucket".to_string(),
            prefix: Some("prefix/".to_string()),
            max_keys: 1000,
            is_truncated: false,
            contents: vec![ObjectInfo {
                key: "test.txt".to_string(),
                size: 1024,
                last_modified: "2023-01-01T00:00:00.000Z".to_string(),
                etag: "\"abc123\"".to_string(),
                storage_class: "STANDARD".to_string(),
            }],
            key_count: 1,
            continuation_token: None,
            next_continuation_token: None,
        };

        let xml = list_objects_v2_xml(&result);
        assert!(xml.contains("<Name>test-bucket</Name>"));
        assert!(xml.contains("<Key>test.txt</Key>"));
        assert!(xml.contains("<Size>1024</Size>"));
    }

    #[test]
    fn test_error_xml() {
        let xml = error_xml("NoSuchBucket", "The specified bucket does not exist");
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Message>The specified bucket does not exist</Message>"));
    }

    #[test]
    fn test_initiate_multipart_xml() {
        let xml = initiate_multipart_xml("my-bucket", "my-key", "upload-123");
        assert!(xml.contains("<Bucket>my-bucket</Bucket>"));
        assert!(xml.contains("<Key>my-key</Key>"));
        assert!(xml.contains("<UploadId>upload-123</UploadId>"));
    }

    #[test]
    fn test_complete_multipart_xml() {
        let result = CompleteMultipartResult {
            bucket: "my-bucket".to_string(),
            key: "my-key".to_string(),
            etag: "\"abc123\"".to_string(),
            location: "https://my-bucket.s3.amazonaws.com/my-key".to_string(),
        };

        let xml = complete_multipart_xml(&result);
        assert!(xml.contains("<Bucket>my-bucket</Bucket>"));
        assert!(xml.contains("<Key>my-key</Key>"));
        assert!(xml.contains("<ETag>&quot;abc123&quot;</ETag>"));
    }

    #[test]
    fn test_copy_object_xml() {
        let xml = copy_object_xml("\"abc123\"", "2023-01-01T00:00:00.000Z");
        assert!(xml.contains("<ETag>&quot;abc123&quot;</ETag>"));
        assert!(xml.contains("<LastModified>2023-01-01T00:00:00.000Z</LastModified>"));
    }

    #[test]
    fn test_delete_objects_xml() {
        let results = vec![
            DeleteResult {
                key: "key1".to_string(),
                success: true,
                error_code: None,
                error_message: None,
            },
            DeleteResult {
                key: "key2".to_string(),
                success: false,
                error_code: Some("AccessDenied".to_string()),
                error_message: Some("Access denied".to_string()),
            },
        ];

        let xml = delete_objects_xml(&results);
        assert!(xml.contains("<Deleted>"));
        assert!(xml.contains("<Key>key1</Key>"));
        assert!(xml.contains("<Error>"));
        assert!(xml.contains("<Key>key2</Key>"));
        assert!(xml.contains("<Code>AccessDenied</Code>"));
    }

    #[test]
    fn test_parse_delete_objects_request() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Object>
        <Key>key1</Key>
    </Object>
    <Object>
        <Key>key2</Key>
        <VersionId>v1</VersionId>
    </Object>
    <Quiet>true</Quiet>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 2);
        assert_eq!(request.objects[0].key, "key1");
        assert!(request.objects[0].version_id.is_none());
        assert_eq!(request.objects[1].key, "key2");
        assert_eq!(request.objects[1].version_id, Some("v1".to_string()));
        assert!(request.quiet);
    }

    #[test]
    fn test_parse_delete_objects_request_no_quiet() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Object>
        <Key>test-key</Key>
    </Object>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 1);
        assert_eq!(request.objects[0].key, "test-key");
        assert!(!request.quiet);
    }

    #[test]
    fn test_extract_xml_tag() {
        assert_eq!(
            extract_xml_tag("<Key>test</Key>", "Key"),
            Some("test".to_string())
        );
        assert_eq!(
            extract_xml_tag("<VersionId>v1</VersionId>", "VersionId"),
            Some("v1".to_string())
        );
        assert_eq!(extract_xml_tag("<Key>test</Key>", "VersionId"), None);
    }
}
