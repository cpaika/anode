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

/// Information about an object version
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub last_modified: String,
    pub etag: String,
    pub size: u64,
    pub storage_class: String,
    pub is_delete_marker: bool,
}

/// Result of a ListObjectVersions operation
#[derive(Debug, Clone)]
pub struct ListVersionsResult {
    pub name: String,
    pub prefix: Option<String>,
    pub max_keys: u32,
    pub is_truncated: bool,
    pub versions: Vec<VersionInfo>,
    pub delete_markers: Vec<VersionInfo>,
    pub key_marker: Option<String>,
    pub version_id_marker: Option<String>,
    pub next_key_marker: Option<String>,
    pub next_version_id_marker: Option<String>,
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
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Owner>
        <ID>anode-owner</ID>
        <DisplayName>anode</DisplayName>
    </Owner>
    <Buckets>"#,
    );

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

/// Generates XML for ListObjectVersions response
pub fn list_versions_xml(result: &ListVersionsResult) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListVersionsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );

    xml.push_str(&format!(
        r#"
    <Name>{}</Name>
    <MaxKeys>{}</MaxKeys>
    <IsTruncated>{}</IsTruncated>"#,
        xml_escape(&result.name),
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

    if let Some(key_marker) = &result.key_marker {
        xml.push_str(&format!(
            r#"
    <KeyMarker>{}</KeyMarker>"#,
            xml_escape(key_marker)
        ));
    }

    if let Some(version_id_marker) = &result.version_id_marker {
        xml.push_str(&format!(
            r#"
    <VersionIdMarker>{}</VersionIdMarker>"#,
            xml_escape(version_id_marker)
        ));
    }

    if let Some(next_key_marker) = &result.next_key_marker {
        xml.push_str(&format!(
            r#"
    <NextKeyMarker>{}</NextKeyMarker>"#,
            xml_escape(next_key_marker)
        ));
    }

    if let Some(next_version_id_marker) = &result.next_version_id_marker {
        xml.push_str(&format!(
            r#"
    <NextVersionIdMarker>{}</NextVersionIdMarker>"#,
            xml_escape(next_version_id_marker)
        ));
    }

    // Output versions (both regular versions and delete markers)
    for version in &result.versions {
        if version.is_delete_marker {
            xml.push_str(&format!(
                r#"
    <DeleteMarker>
        <Key>{}</Key>
        <VersionId>{}</VersionId>
        <IsLatest>{}</IsLatest>
        <LastModified>{}</LastModified>
        <Owner>
            <ID>anode-owner</ID>
            <DisplayName>anode</DisplayName>
        </Owner>
    </DeleteMarker>"#,
                xml_escape(&version.key),
                xml_escape(&version.version_id),
                version.is_latest,
                xml_escape(&version.last_modified)
            ));
        } else {
            xml.push_str(&format!(
                r#"
    <Version>
        <Key>{}</Key>
        <VersionId>{}</VersionId>
        <IsLatest>{}</IsLatest>
        <LastModified>{}</LastModified>
        <ETag>{}</ETag>
        <Size>{}</Size>
        <StorageClass>{}</StorageClass>
        <Owner>
            <ID>anode-owner</ID>
            <DisplayName>anode</DisplayName>
        </Owner>
    </Version>"#,
                xml_escape(&version.key),
                xml_escape(&version.version_id),
                version.is_latest,
                xml_escape(&version.last_modified),
                xml_escape(&version.etag),
                version.size,
                xml_escape(&version.storage_class)
            ));
        }
    }

    xml.push_str("\n</ListVersionsResult>");
    xml
}

/// Generates XML for ListObjectsV2 response
pub fn list_objects_v2_xml(result: &ListObjectsResult) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );

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
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"Object" => {
                    in_object = true;
                    current_key = None;
                    current_version_id = None;
                },
                _ => {},
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"Object" => {
                    if let Some(key) = current_key.take() {
                        objects.push(DeleteObjectInfo {
                            key,
                            version_id: current_version_id.take(),
                        });
                    }
                    in_object = false;
                },
                _ => {},
            },
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|e| e.to_string())?.to_string();

                // Get the tag name from the reader's position
                if let Some(tag_name) = reader.decoder().decode(e.as_ref()).ok() {
                    // This is a simplification - we need to track the current element
                    // For now, we'll handle this differently
                }
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"Quiet" => quiet = true,
                _ => {},
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parsing XML: {}", e)),
            _ => {},
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
        key: manifest.key.key.clone(),
        size: manifest.total_size,
        last_modified: format_timestamp(manifest.modified_at as u64),
        etag: format!("\"{}\"", hex::encode(manifest.checksum)),
        storage_class: "STANDARD".to_string(),
    }
}

/// Converts an ObjectManifest to VersionInfo
pub fn object_manifest_to_version_info(manifest: &ObjectManifest) -> VersionInfo {
    VersionInfo {
        key: manifest.key.key.clone(),
        version_id: manifest.version_id.clone(),
        is_latest: false, // TODO: Determine if this is the latest version
        last_modified: format_timestamp(manifest.modified_at as u64),
        etag: format!("\"{}\"", hex::encode(manifest.checksum)),
        size: manifest.total_size,
        storage_class: "STANDARD".to_string(),
        is_delete_marker: manifest.is_delete_marker,
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
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"Part" => {
                    current_part_number = None;
                    current_etag = None;
                },
                _ => {},
            },
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"Part" => {
                        if let (Some(part_number), Some(etag)) =
                            (current_part_number, current_etag.take())
                        {
                            parts.push(CompletePart {
                                part_number,
                                etag: etag.trim_matches('"').to_string(),
                            });
                        }
                    },
                    b"PartNumber" => {
                        // Part number will be captured in Text event
                    },
                    b"ETag" => {
                        // ETag will be captured in Text event
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(e)) => {
                let text = e
                    .unescape()
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
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(S3Error::InvalidRequest(format!("XML parse error: {}", e)));
            },
            _ => {},
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

// ============================================================================
// Lifecycle Configuration Types
// ============================================================================

/// Lifecycle configuration for a bucket
#[derive(Debug, Clone)]
pub struct LifecycleConfiguration {
    pub rules: Vec<LifecycleRule>,
}

impl LifecycleConfiguration {
    /// Validates the lifecycle configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.rules.is_empty() {
            return Err("Lifecycle configuration must have at least one rule".to_string());
        }

        if self.rules.len() > 1000 {
            return Err("Lifecycle configuration cannot have more than 1000 rules".to_string());
        }

        // Check for duplicate rule IDs
        let mut ids = std::collections::HashSet::new();
        for rule in &self.rules {
            if let Some(id) = &rule.id {
                if !ids.insert(id.clone()) {
                    return Err(format!("Duplicate rule ID: {}", id));
                }
            }
        }

        // Validate each rule
        for rule in &self.rules {
            rule.validate()?;
        }

        Ok(())
    }
}

/// A lifecycle rule
#[derive(Debug, Clone)]
pub struct LifecycleRule {
    /// Rule ID (optional, auto-generated if not provided)
    pub id: Option<String>,
    /// Rule status (Enabled or Disabled)
    pub status: LifecycleRuleStatus,
    /// Filter for objects this rule applies to
    pub filter: Option<LifecycleFilter>,
    /// Expiration configuration
    pub expiration: Option<LifecycleExpiration>,
    /// Noncurrent version expiration configuration
    pub noncurrent_version_expiration: Option<NoncurrentVersionExpiration>,
    /// Abort incomplete multipart upload configuration
    pub abort_incomplete_multipart_upload: Option<AbortIncompleteMultipartUpload>,
}

impl LifecycleRule {
    /// Validates the lifecycle rule
    pub fn validate(&self) -> Result<(), String> {
        // At least one action must be specified
        if self.expiration.is_none()
            && self.noncurrent_version_expiration.is_none()
            && self.abort_incomplete_multipart_upload.is_none()
        {
            return Err("Rule must have at least one action".to_string());
        }

        // Validate expiration if present
        if let Some(exp) = &self.expiration {
            exp.validate()?;
        }

        // Validate noncurrent version expiration if present
        if let Some(nve) = &self.noncurrent_version_expiration {
            nve.validate()?;
        }

        // Validate abort incomplete multipart upload if present
        if let Some(aim) = &self.abort_incomplete_multipart_upload {
            aim.validate()?;
        }

        Ok(())
    }
}

/// Lifecycle rule status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleRuleStatus {
    Enabled,
    Disabled,
}

/// Filter for lifecycle rules
#[derive(Debug, Clone)]
pub struct LifecycleFilter {
    /// Prefix filter (objects with this prefix match the rule)
    pub prefix: Option<String>,
}

/// Expiration configuration
#[derive(Debug, Clone)]
pub struct LifecycleExpiration {
    /// Expire objects after this many days
    pub days: Option<u32>,
    /// Expire objects on this specific date
    pub date: Option<String>,
    /// Delete expired object delete markers
    pub expired_object_delete_marker: Option<bool>,
}

impl LifecycleExpiration {
    /// Validates the expiration configuration
    pub fn validate(&self) -> Result<(), String> {
        // Must have either days or date, but not both
        match (&self.days, &self.date) {
            (Some(_), Some(_)) => Err("Expiration cannot have both Days and Date".to_string()),
            (None, None) if self.expired_object_delete_marker != Some(true) => {
                Err("Expiration must have Days or Date".to_string())
            },
            (Some(days), None) if *days == 0 => {
                Err("Expiration Days must be greater than 0".to_string())
            },
            _ => Ok(()),
        }
    }
}

/// Noncurrent version expiration configuration
#[derive(Debug, Clone)]
pub struct NoncurrentVersionExpiration {
    /// Delete noncurrent versions after this many days
    pub noncurrent_days: u32,
}

impl NoncurrentVersionExpiration {
    /// Validates the noncurrent version expiration configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.noncurrent_days == 0 {
            return Err("NoncurrentDays must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Abort incomplete multipart upload configuration
#[derive(Debug, Clone)]
pub struct AbortIncompleteMultipartUpload {
    /// Abort uploads after this many days since initiation
    pub days_after_initiation: u32,
}

impl AbortIncompleteMultipartUpload {
    /// Validates the abort incomplete multipart upload configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.days_after_initiation == 0 {
            return Err("DaysAfterInitiation must be greater than 0".to_string());
        }
        Ok(())
    }
}

/// Parses lifecycle configuration from XML
pub fn parse_lifecycle_configuration(xml: &str) -> Result<LifecycleConfiguration, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut rules = Vec::new();
    let mut current_rule: Option<LifecycleRule> = None;
    let mut current_filter: Option<LifecycleFilter> = None;
    let mut current_expiration: Option<LifecycleExpiration> = None;
    let mut current_noncurrent_expiration: Option<NoncurrentVersionExpiration> = None;
    let mut current_abort_multipart: Option<AbortIncompleteMultipartUpload> = None;
    let mut current_element_stack: Vec<String> = Vec::new();
    let mut current_text = String::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_element_stack.push(tag_name.clone());

                match tag_name.as_str() {
                    "Rule" => {
                        current_rule = Some(LifecycleRule {
                            id: None,
                            status: LifecycleRuleStatus::Disabled,
                            filter: None,
                            expiration: None,
                            noncurrent_version_expiration: None,
                            abort_incomplete_multipart_upload: None,
                        });
                    },
                    "Filter" => {
                        current_filter = Some(LifecycleFilter { prefix: None });
                    },
                    "Expiration" => {
                        current_expiration = Some(LifecycleExpiration {
                            days: None,
                            date: None,
                            expired_object_delete_marker: None,
                        });
                    },
                    "NoncurrentVersionExpiration" => {
                        current_noncurrent_expiration =
                            Some(NoncurrentVersionExpiration { noncurrent_days: 0 });
                    },
                    "AbortIncompleteMultipartUpload" => {
                        current_abort_multipart = Some(AbortIncompleteMultipartUpload {
                            days_after_initiation: 0,
                        });
                    },
                    _ => {},
                }
            },
            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "Rule" => {
                        if let Some(rule) = current_rule.take() {
                            rules.push(rule);
                        }
                    },
                    "Filter" => {
                        if let (Some(rule), Some(filter)) =
                            (current_rule.as_mut(), current_filter.take())
                        {
                            rule.filter = Some(filter);
                        }
                    },
                    "Expiration" => {
                        if let (Some(rule), Some(exp)) =
                            (current_rule.as_mut(), current_expiration.take())
                        {
                            rule.expiration = Some(exp);
                        }
                    },
                    "NoncurrentVersionExpiration" => {
                        if let (Some(rule), Some(nve)) =
                            (current_rule.as_mut(), current_noncurrent_expiration.take())
                        {
                            rule.noncurrent_version_expiration = Some(nve);
                        }
                    },
                    "AbortIncompleteMultipartUpload" => {
                        if let (Some(rule), Some(aim)) =
                            (current_rule.as_mut(), current_abort_multipart.take())
                        {
                            rule.abort_incomplete_multipart_upload = Some(aim);
                        }
                    },
                    "ID" => {
                        if let Some(rule) = current_rule.as_mut() {
                            rule.id = Some(current_text.trim().to_string());
                        }
                    },
                    "Status" => {
                        if let Some(rule) = current_rule.as_mut() {
                            rule.status = match current_text.trim() {
                                "Enabled" => LifecycleRuleStatus::Enabled,
                                _ => LifecycleRuleStatus::Disabled,
                            };
                        }
                    },
                    "Prefix" => {
                        if let Some(filter) = current_filter.as_mut() {
                            filter.prefix = Some(current_text.trim().to_string());
                        }
                    },
                    "Days" => {
                        let days = current_text
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| format!("Invalid Days value: {}", current_text))?;
                        if let Some(exp) = current_expiration.as_mut() {
                            exp.days = Some(days);
                        }
                    },
                    "Date" => {
                        if let Some(exp) = current_expiration.as_mut() {
                            exp.date = Some(current_text.trim().to_string());
                        }
                    },
                    "ExpiredObjectDeleteMarker" => {
                        if let Some(exp) = current_expiration.as_mut() {
                            exp.expired_object_delete_marker = Some(current_text.trim() == "true");
                        }
                    },
                    "NoncurrentDays" => {
                        let days = current_text.trim().parse::<u32>().map_err(|_| {
                            format!("Invalid NoncurrentDays value: {}", current_text)
                        })?;
                        if let Some(nve) = current_noncurrent_expiration.as_mut() {
                            nve.noncurrent_days = days;
                        }
                    },
                    "DaysAfterInitiation" => {
                        let days = current_text.trim().parse::<u32>().map_err(|_| {
                            format!("Invalid DaysAfterInitiation value: {}", current_text)
                        })?;
                        if let Some(aim) = current_abort_multipart.as_mut() {
                            aim.days_after_initiation = days;
                        }
                    },
                    _ => {},
                }

                current_element_stack.pop();
                current_text.clear();
            },
            Ok(Event::Text(e)) => {
                current_text = e.unescape().map_err(|e| e.to_string())?.to_string();
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parsing XML: {}", e)),
            _ => {},
        }
        buf.clear();
    }

    Ok(LifecycleConfiguration { rules })
}

// ============================================================================
// Object Expiration Evaluation
// ============================================================================

/// Checks if an object should be expired based on lifecycle rules
pub fn should_expire_object(
    object_key: &str,
    object_modified_at: i64,
    is_current_version: bool,
    rules: &[LifecycleRule],
    current_time: i64,
) -> bool {
    for rule in rules {
        // Skip disabled rules
        if rule.status != LifecycleRuleStatus::Enabled {
            continue;
        }

        // Check if filter matches
        if let Some(filter) = &rule.filter {
            if let Some(prefix) = &filter.prefix {
                if !object_key.starts_with(prefix) {
                    continue;
                }
            }
        }

        // Check expiration for current versions
        if is_current_version {
            if let Some(expiration) = &rule.expiration {
                if is_expired_by_rule(object_modified_at, expiration, current_time) {
                    return true;
                }
            }
        } else {
            // Check noncurrent version expiration
            if let Some(noncurrent_exp) = &rule.noncurrent_version_expiration {
                let days_since_noncurrent = (current_time - object_modified_at) / 86400;
                if days_since_noncurrent >= noncurrent_exp.noncurrent_days as i64 {
                    return true;
                }
            }
        }
    }

    false
}

/// Checks if an object is expired based on a specific expiration rule
fn is_expired_by_rule(
    object_modified_at: i64,
    expiration: &LifecycleExpiration,
    current_time: i64,
) -> bool {
    // Check expiration by days
    if let Some(days) = expiration.days {
        let days_since_modified = (current_time - object_modified_at) / 86400;
        if days_since_modified >= days as i64 {
            return true;
        }
    }

    // Check expiration by date
    if let Some(date_str) = &expiration.date {
        // Parse ISO 8601 date: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS.000Z
        if let Some(expiration_timestamp) = parse_iso8601_date(date_str) {
            if current_time >= expiration_timestamp {
                return true;
            }
        }
    }

    false
}

/// Parses an ISO 8601 date string to Unix timestamp
fn parse_iso8601_date(date_str: &str) -> Option<i64> {
    // Simple parser for ISO 8601 dates
    // Supports: YYYY-MM-DD and YYYY-MM-DDTHH:MM:SS.000Z

    // Remove timezone and milliseconds for simplicity
    let date_part = date_str.split('T').next()?.trim();
    let parts: Vec<&str> = date_part.split('-').collect();

    if parts.len() != 3 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let month: i32 = parts[1].parse().ok()?;
    let day: i32 = parts[2].parse().ok()?;

    // Validate ranges
    if month < 1 || month > 12 || day < 1 || day > 31 {
        return None;
    }

    // Simple approximation: calculate days since epoch
    // This is a simplified calculation for testing purposes
    let mut days_since_epoch = 0i64;

    // Add days for complete years
    for y in 1970..year {
        if is_leap_year(y) {
            days_since_epoch += 366;
        } else {
            days_since_epoch += 365;
        }
    }

    // Add days for complete months in current year
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        let mut days = days_in_month[(m - 1) as usize];
        if m == 2 && is_leap_year(year) {
            days = 29;
        }
        days_since_epoch += days as i64;
    }

    // Add remaining days
    days_since_epoch += day as i64 - 1;

    // Convert to seconds
    Some(days_since_epoch * 86400)
}

/// Checks if a year is a leap year
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Generates XML for lifecycle configuration
pub fn lifecycle_configuration_xml(config: &LifecycleConfiguration) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#,
    );

    for rule in &config.rules {
        xml.push_str("\n    <Rule>");

        // ID
        if let Some(id) = &rule.id {
            xml.push_str(&format!("\n        <ID>{}</ID>", xml_escape(id)));
        }

        // Status
        let status_str = match rule.status {
            LifecycleRuleStatus::Enabled => "Enabled",
            LifecycleRuleStatus::Disabled => "Disabled",
        };
        xml.push_str(&format!("\n        <Status>{}</Status>", status_str));

        // Filter
        if let Some(filter) = &rule.filter {
            xml.push_str("\n        <Filter>");
            if let Some(prefix) = &filter.prefix {
                xml.push_str(&format!(
                    "\n            <Prefix>{}</Prefix>",
                    xml_escape(prefix)
                ));
            }
            xml.push_str("\n        </Filter>");
        }

        // Expiration
        if let Some(exp) = &rule.expiration {
            xml.push_str("\n        <Expiration>");
            if let Some(days) = exp.days {
                xml.push_str(&format!("\n            <Days>{}</Days>", days));
            }
            if let Some(date) = &exp.date {
                xml.push_str(&format!("\n            <Date>{}</Date>", xml_escape(date)));
            }
            if let Some(expired_marker) = exp.expired_object_delete_marker {
                xml.push_str(&format!(
                    "\n            <ExpiredObjectDeleteMarker>{}</ExpiredObjectDeleteMarker>",
                    expired_marker
                ));
            }
            xml.push_str("\n        </Expiration>");
        }

        // NoncurrentVersionExpiration
        if let Some(nve) = &rule.noncurrent_version_expiration {
            xml.push_str(&format!(
                "\n        <NoncurrentVersionExpiration>\n            <NoncurrentDays>{}</NoncurrentDays>\n        </NoncurrentVersionExpiration>",
                nve.noncurrent_days
            ));
        }

        // AbortIncompleteMultipartUpload
        if let Some(aim) = &rule.abort_incomplete_multipart_upload {
            xml.push_str(&format!(
                "\n        <AbortIncompleteMultipartUpload>\n            <DaysAfterInitiation>{}</DaysAfterInitiation>\n        </AbortIncompleteMultipartUpload>",
                aim.days_after_initiation
            ));
        }

        xml.push_str("\n    </Rule>");
    }

    xml.push_str("\n</LifecycleConfiguration>");
    xml
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

        // Verify XML declaration
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));

        // Verify root element with namespace
        assert!(xml.contains(
            "<ListAllMyBucketsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">"
        ));

        // Verify owner information
        assert!(xml.contains("<Owner>"));
        assert!(xml.contains("<ID>anode-owner</ID>"));
        assert!(xml.contains("<DisplayName>anode</DisplayName>"));
        assert!(xml.contains("</Owner>"));

        // Verify buckets container
        assert!(xml.contains("<Buckets>"));
        assert!(xml.contains("</Buckets>"));

        // Verify bucket1 information
        assert!(xml.contains("<Bucket>"));
        assert!(xml.contains("<Name>bucket1</Name>"));
        assert!(xml.contains("<CreationDate>2023-01-01T00:00:00.000Z</CreationDate>"));

        // Verify bucket2 information
        assert!(xml.contains("<Name>bucket2</Name>"));
        assert!(xml.contains("<CreationDate>2023-01-02T00:00:00.000Z</CreationDate>"));

        // Verify closing tag
        assert!(xml.ends_with("</ListAllMyBucketsResult>"));
    }

    #[test]
    fn test_list_buckets_xml_empty() {
        let buckets = vec![];
        let xml = list_buckets_xml(&buckets);

        // Verify structure is still valid with no buckets
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains(
            "<ListAllMyBucketsResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">"
        ));
        assert!(xml.contains("<Owner>"));
        assert!(xml.contains("<Buckets>"));
        assert!(xml.contains("</Buckets>"));
        assert!(xml.contains("</ListAllMyBucketsResult>"));

        // Ensure no <Bucket> elements (only <Buckets> container)
        assert!(!xml.contains("<Bucket>"));
    }

    #[test]
    fn test_list_buckets_xml_special_characters() {
        let buckets = vec![
            BucketInfo {
                name: "bucket-with-&-ampersand".to_string(),
                creation_date: "2023-01-01T00:00:00.000Z".to_string(),
            },
            BucketInfo {
                name: "bucket<with>brackets".to_string(),
                creation_date: "2023-01-02T00:00:00.000Z".to_string(),
            },
            BucketInfo {
                name: "bucket\"with'quotes".to_string(),
                creation_date: "2023-01-03T00:00:00.000Z".to_string(),
            },
        ];

        let xml = list_buckets_xml(&buckets);

        // Verify special characters are properly escaped
        assert!(xml.contains("<Name>bucket-with-&amp;-ampersand</Name>"));
        assert!(xml.contains("<Name>bucket&lt;with&gt;brackets</Name>"));
        assert!(xml.contains("<Name>bucket&quot;with&apos;quotes</Name>"));

        // Verify creation dates are also escaped (though they shouldn't have special chars normally)
        assert!(xml.contains("<CreationDate>2023-01-01T00:00:00.000Z</CreationDate>"));
        assert!(xml.contains("<CreationDate>2023-01-02T00:00:00.000Z</CreationDate>"));
        assert!(xml.contains("<CreationDate>2023-01-03T00:00:00.000Z</CreationDate>"));
    }

    #[test]
    fn test_list_buckets_xml_single_bucket() {
        let buckets = vec![BucketInfo {
            name: "my-only-bucket".to_string(),
            creation_date: "2023-12-25T10:30:00.000Z".to_string(),
        }];

        let xml = list_buckets_xml(&buckets);

        // Verify complete structure with single bucket
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<ListAllMyBucketsResult"));
        assert!(xml.contains("<Owner>"));
        assert!(xml.contains("<Buckets>"));
        assert!(xml.contains("<Bucket>"));
        assert!(xml.contains("<Name>my-only-bucket</Name>"));
        assert!(xml.contains("<CreationDate>2023-12-25T10:30:00.000Z</CreationDate>"));
        assert!(xml.contains("</Bucket>"));
        assert!(xml.contains("</Buckets>"));
        assert!(xml.contains("</ListAllMyBucketsResult>"));
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
    fn test_list_objects_v2_xml_multiple_objects() {
        let result = ListObjectsResult {
            name: "my-bucket".to_string(),
            prefix: None,
            max_keys: 1000,
            is_truncated: false,
            contents: vec![
                ObjectInfo {
                    key: "file1.txt".to_string(),
                    size: 100,
                    last_modified: "2023-01-01T00:00:00.000Z".to_string(),
                    etag: "\"etag1\"".to_string(),
                    storage_class: "STANDARD".to_string(),
                },
                ObjectInfo {
                    key: "file2.txt".to_string(),
                    size: 200,
                    last_modified: "2023-01-02T00:00:00.000Z".to_string(),
                    etag: "\"etag2\"".to_string(),
                    storage_class: "STANDARD".to_string(),
                },
                ObjectInfo {
                    key: "file3.txt".to_string(),
                    size: 300,
                    last_modified: "2023-01-03T00:00:00.000Z".to_string(),
                    etag: "\"etag3\"".to_string(),
                    storage_class: "STANDARD".to_string(),
                },
            ],
            key_count: 3,
            continuation_token: None,
            next_continuation_token: None,
        };

        let xml = list_objects_v2_xml(&result);

        // Verify XML declaration and root element
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(
            xml.contains("<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">")
        );

        // Verify bucket metadata
        assert!(xml.contains("<Name>my-bucket</Name>"));
        assert!(xml.contains("<KeyCount>3</KeyCount>"));
        assert!(xml.contains("<MaxKeys>1000</MaxKeys>"));
        assert!(xml.contains("<IsTruncated>false</IsTruncated>"));

        // Verify all objects are present
        assert!(xml.contains("<Key>file1.txt</Key>"));
        assert!(xml.contains("<Size>100</Size>"));
        assert!(xml.contains("<Key>file2.txt</Key>"));
        assert!(xml.contains("<Size>200</Size>"));
        assert!(xml.contains("<Key>file3.txt</Key>"));
        assert!(xml.contains("<Size>300</Size>"));

        // Verify all objects have required fields
        for i in 1..=3 {
            assert!(xml.contains(&format!("<ETag>&quot;etag{}&quot;</ETag>", i)));
            assert!(xml.contains("<StorageClass>STANDARD</StorageClass>"));
        }

        // Verify closing tag
        assert!(xml.ends_with("</ListBucketResult>"));
    }

    #[test]
    fn test_list_objects_v2_xml_empty() {
        let result = ListObjectsResult {
            name: "empty-bucket".to_string(),
            prefix: None,
            max_keys: 1000,
            is_truncated: false,
            contents: vec![],
            key_count: 0,
            continuation_token: None,
            next_continuation_token: None,
        };

        let xml = list_objects_v2_xml(&result);

        // Verify structure for empty bucket
        assert!(xml.contains("<Name>empty-bucket</Name>"));
        assert!(xml.contains("<KeyCount>0</KeyCount>"));
        assert!(xml.contains("<IsTruncated>false</IsTruncated>"));
        assert!(xml.contains("</ListBucketResult>"));

        // Ensure no <Contents> elements
        assert!(!xml.contains("<Contents>"));
    }

    #[test]
    fn test_list_objects_v2_xml_with_continuation_tokens() {
        let result = ListObjectsResult {
            name: "paginated-bucket".to_string(),
            prefix: Some("docs/".to_string()),
            max_keys: 10,
            is_truncated: true,
            contents: vec![ObjectInfo {
                key: "docs/file.txt".to_string(),
                size: 512,
                last_modified: "2023-01-01T00:00:00.000Z".to_string(),
                etag: "\"abc\"".to_string(),
                storage_class: "STANDARD".to_string(),
            }],
            key_count: 1,
            continuation_token: Some("token123".to_string()),
            next_continuation_token: Some("token456".to_string()),
        };

        let xml = list_objects_v2_xml(&result);

        // Verify pagination elements
        assert!(xml.contains("<IsTruncated>true</IsTruncated>"));
        assert!(xml.contains("<Prefix>docs/</Prefix>"));
        assert!(xml.contains("<ContinuationToken>token123</ContinuationToken>"));
        assert!(xml.contains("<NextContinuationToken>token456</NextContinuationToken>"));
        assert!(xml.contains("<MaxKeys>10</MaxKeys>"));
    }

    #[test]
    fn test_list_objects_v2_xml_escapes_special_characters() {
        let result = ListObjectsResult {
            name: "test-bucket".to_string(),
            prefix: Some("path/with/&<>\"'".to_string()),
            max_keys: 1000,
            is_truncated: false,
            contents: vec![ObjectInfo {
                key: "file&name<test>.txt".to_string(),
                size: 1024,
                last_modified: "2023-01-01T00:00:00.000Z".to_string(),
                etag: "\"etag&<>\"".to_string(),
                storage_class: "STANDARD".to_string(),
            }],
            key_count: 1,
            continuation_token: None,
            next_continuation_token: None,
        };

        let xml = list_objects_v2_xml(&result);

        // Verify special characters are properly escaped
        assert!(xml.contains("<Prefix>path/with/&amp;&lt;&gt;&quot;&apos;</Prefix>"));
        assert!(xml.contains("<Key>file&amp;name&lt;test&gt;.txt</Key>"));
        assert!(xml.contains("<ETag>&quot;etag&amp;&lt;&gt;&quot;</ETag>"));
    }

    #[test]
    fn test_error_xml() {
        let xml = error_xml("NoSuchBucket", "The specified bucket does not exist");
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Message>The specified bucket does not exist</Message>"));
    }

    #[test]
    fn test_error_xml_structure() {
        let xml = error_xml("NoSuchKey", "The specified key does not exist");

        // Verify XML declaration
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));

        // Verify root element
        assert!(xml.contains("<Error>"));
        assert!(xml.ends_with("</Error>"));

        // Verify Code element
        assert!(xml.contains("<Code>NoSuchKey</Code>"));

        // Verify Message element
        assert!(xml.contains("<Message>The specified key does not exist</Message>"));

        // Verify complete structure
        assert!(xml.contains(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchKey</Code>
    <Message>The specified key does not exist</Message>
</Error>"#
        ));
    }

    #[test]
    fn test_error_xml_escapes_special_characters() {
        let xml = error_xml(
            "InvalidRequest",
            "Bucket name contains invalid characters: <>&\"'",
        );

        // Verify special characters are properly escaped
        assert!(xml.contains("<Code>InvalidRequest</Code>"));
        assert!(xml.contains(
            "<Message>Bucket name contains invalid characters: &lt;&gt;&amp;&quot;&apos;</Message>"
        ));

        // Verify the raw characters are not present in the output
        assert!(!xml.contains("characters: <>&"));
    }

    #[test]
    fn test_error_xml_various_error_codes() {
        // Test AccessDenied
        let xml = error_xml("AccessDenied", "Access Denied");
        assert!(xml.contains("<Code>AccessDenied</Code>"));
        assert!(xml.contains("<Message>Access Denied</Message>"));

        // Test InvalidAccessKeyId
        let xml = error_xml(
            "InvalidAccessKeyId",
            "The AWS Access Key Id you provided does not exist in our records",
        );
        assert!(xml.contains("<Code>InvalidAccessKeyId</Code>"));
        assert!(xml.contains(
            "<Message>The AWS Access Key Id you provided does not exist in our records</Message>"
        ));

        // Test InternalError
        let xml = error_xml(
            "InternalError",
            "We encountered an internal error. Please try again.",
        );
        assert!(xml.contains("<Code>InternalError</Code>"));
        assert!(
            xml.contains("<Message>We encountered an internal error. Please try again.</Message>")
        );

        // Test BucketAlreadyExists
        let xml = error_xml(
            "BucketAlreadyExists",
            "The requested bucket name is not available",
        );
        assert!(xml.contains("<Code>BucketAlreadyExists</Code>"));
        assert!(xml.contains("<Message>The requested bucket name is not available</Message>"));
    }

    #[test]
    fn test_error_xml_empty_message() {
        let xml = error_xml("UnknownError", "");
        assert!(xml.contains("<Code>UnknownError</Code>"));
        assert!(xml.contains("<Message></Message>"));
    }

    #[test]
    fn test_error_xml_long_message() {
        let long_message = "This is a very long error message that contains a lot of details about what went wrong. It might include stack traces, file paths, and other debugging information that could be useful for diagnosing issues.";
        let xml = error_xml("InvalidRequest", long_message);

        assert!(xml.contains("<Code>InvalidRequest</Code>"));
        assert!(xml.contains(&format!("<Message>{}</Message>", long_message)));
    }

    #[test]
    fn test_initiate_multipart_xml() {
        let xml = initiate_multipart_xml("my-bucket", "my-key", "upload-123");
        assert!(xml.contains("<Bucket>my-bucket</Bucket>"));
        assert!(xml.contains("<Key>my-key</Key>"));
        assert!(xml.contains("<UploadId>upload-123</UploadId>"));
    }

    #[test]
    fn test_initiate_multipart_xml_structure() {
        let xml = initiate_multipart_xml("test-bucket", "test-key.txt", "abc123-upload-id");

        // Verify XML declaration
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));

        // Verify root element with namespace
        assert!(xml.contains(
            "<InitiateMultipartUploadResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">"
        ));

        // Verify Bucket element
        assert!(xml.contains("<Bucket>test-bucket</Bucket>"));

        // Verify Key element
        assert!(xml.contains("<Key>test-key.txt</Key>"));

        // Verify UploadId element
        assert!(xml.contains("<UploadId>abc123-upload-id</UploadId>"));

        // Verify closing tag
        assert!(xml.ends_with("</InitiateMultipartUploadResult>"));
    }

    #[test]
    fn test_initiate_multipart_xml_escapes_special_characters() {
        let xml = initiate_multipart_xml(
            "bucket-with-&-ampersand",
            "key/with/<special>&\"characters\"",
            "upload<id>&123",
        );

        // Verify special characters are properly escaped in Bucket
        assert!(xml.contains("<Bucket>bucket-with-&amp;-ampersand</Bucket>"));

        // Verify special characters are properly escaped in Key
        assert!(xml.contains("<Key>key/with/&lt;special&gt;&amp;&quot;characters&quot;</Key>"));

        // Verify special characters are properly escaped in UploadId
        assert!(xml.contains("<UploadId>upload&lt;id&gt;&amp;123</UploadId>"));

        // Verify the raw special characters are not present (they should be escaped)
        let bucket_section = xml
            .split("<Bucket>")
            .nth(1)
            .unwrap()
            .split("</Bucket>")
            .next()
            .unwrap();
        assert!(!bucket_section.contains('&') || bucket_section.contains("&amp;"));
    }

    #[test]
    fn test_initiate_multipart_xml_empty_strings() {
        let xml = initiate_multipart_xml("", "", "");

        // Verify structure is still valid with empty values
        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<InitiateMultipartUploadResult"));
        assert!(xml.contains("<Bucket></Bucket>"));
        assert!(xml.contains("<Key></Key>"));
        assert!(xml.contains("<UploadId></UploadId>"));
        assert!(xml.ends_with("</InitiateMultipartUploadResult>"));
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

    #[test]
    fn test_parse_delete_objects_request_empty() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 0);
        assert!(!request.quiet);
    }

    #[test]
    fn test_parse_delete_objects_request_multiple_objects() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Object>
        <Key>file1.txt</Key>
    </Object>
    <Object>
        <Key>file2.txt</Key>
    </Object>
    <Object>
        <Key>file3.txt</Key>
    </Object>
    <Object>
        <Key>path/to/file4.txt</Key>
        <VersionId>version123</VersionId>
    </Object>
    <Quiet>false</Quiet>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 4);
        assert_eq!(request.objects[0].key, "file1.txt");
        assert!(request.objects[0].version_id.is_none());
        assert_eq!(request.objects[1].key, "file2.txt");
        assert!(request.objects[1].version_id.is_none());
        assert_eq!(request.objects[2].key, "file3.txt");
        assert!(request.objects[2].version_id.is_none());
        assert_eq!(request.objects[3].key, "path/to/file4.txt");
        assert_eq!(
            request.objects[3].version_id,
            Some("version123".to_string())
        );
        assert!(!request.quiet);
    }

    #[test]
    fn test_parse_delete_objects_request_special_characters_in_keys() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Object>
        <Key>file with spaces.txt</Key>
    </Object>
    <Object>
        <Key>path/to/file-with-dashes_and_underscores.txt</Key>
    </Object>
    <Object>
        <Key>file@special#chars$.txt</Key>
    </Object>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 3);
        assert_eq!(request.objects[0].key, "file with spaces.txt");
        assert_eq!(
            request.objects[1].key,
            "path/to/file-with-dashes_and_underscores.txt"
        );
        assert_eq!(request.objects[2].key, "file@special#chars$.txt");
    }

    #[test]
    fn test_parse_delete_objects_request_quiet_mode() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Quiet>true</Quiet>
    <Object>
        <Key>quiet-delete.txt</Key>
    </Object>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 1);
        assert_eq!(request.objects[0].key, "quiet-delete.txt");
        assert!(request.quiet);
    }

    #[test]
    fn test_parse_delete_objects_request_with_version_ids() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Object>
        <Key>versioned-file1.txt</Key>
        <VersionId>v1.0</VersionId>
    </Object>
    <Object>
        <Key>versioned-file2.txt</Key>
        <VersionId>v2.5</VersionId>
    </Object>
    <Object>
        <Key>unversioned-file.txt</Key>
    </Object>
</Delete>"#;

        let request = parse_delete_objects_request(xml).unwrap();
        assert_eq!(request.objects.len(), 3);
        assert_eq!(request.objects[0].key, "versioned-file1.txt");
        assert_eq!(request.objects[0].version_id, Some("v1.0".to_string()));
        assert_eq!(request.objects[1].key, "versioned-file2.txt");
        assert_eq!(request.objects[1].version_id, Some("v2.5".to_string()));
        assert_eq!(request.objects[2].key, "unversioned-file.txt");
        assert!(request.objects[2].version_id.is_none());
    }

    #[test]
    fn test_parse_complete_multipart_upload() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"abc123"</ETag>
    </Part>
    <Part>
        <PartNumber>2</PartNumber>
        <ETag>"def456"</ETag>
    </Part>
    <Part>
        <PartNumber>3</PartNumber>
        <ETag>"ghi789"</ETag>
    </Part>
</CompleteMultipartUpload>"#;

        let body = Bytes::from(xml);
        let parts = parse_complete_multipart_upload(&body).unwrap();

        assert_eq!(parts.len(), 3);

        assert_eq!(parts[0].part_number, 1);
        assert_eq!(parts[0].etag, "abc123");

        assert_eq!(parts[1].part_number, 2);
        assert_eq!(parts[1].etag, "def456");

        assert_eq!(parts[2].part_number, 3);
        assert_eq!(parts[2].etag, "ghi789");
    }

    #[test]
    fn test_parse_complete_multipart_upload_single_part() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"single-part-etag"</ETag>
    </Part>
</CompleteMultipartUpload>"#;

        let body = Bytes::from(xml);
        let parts = parse_complete_multipart_upload(&body).unwrap();

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].part_number, 1);
        assert_eq!(parts[0].etag, "single-part-etag");
    }

    #[test]
    fn test_parse_complete_multipart_upload_etag_without_quotes() {
        // S3 clients might send ETags with or without quotes
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>no-quotes-etag</ETag>
    </Part>
</CompleteMultipartUpload>"#;

        let body = Bytes::from(xml);
        let parts = parse_complete_multipart_upload(&body).unwrap();

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].part_number, 1);
        assert_eq!(parts[0].etag, "no-quotes-etag");
    }

    #[test]
    fn test_parse_complete_multipart_upload_empty() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
</CompleteMultipartUpload>"#;

        let body = Bytes::from(xml);
        let result = parse_complete_multipart_upload(&body);

        // Should return an error because at least one part is required
        assert!(result.is_err());
        if let Err(S3Error::InvalidRequest(msg)) = result {
            assert!(msg.contains("at least one part"));
        } else {
            panic!("Expected InvalidRequest error");
        }
    }

    #[test]
    fn test_parse_complete_multipart_upload_invalid_utf8() {
        // Invalid UTF-8 bytes
        let invalid_bytes = vec![0xFF, 0xFE, 0xFD];
        let body = Bytes::from(invalid_bytes);
        let result = parse_complete_multipart_upload(&body);

        // Should return an error for invalid UTF-8
        assert!(result.is_err());
        if let Err(S3Error::InvalidRequest(msg)) = result {
            assert!(msg.contains("Invalid UTF-8"));
        } else {
            panic!("Expected InvalidRequest error for invalid UTF-8");
        }
    }

    #[test]
    fn test_parse_complete_multipart_upload_malformed_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"abc123"
    </Part>"#; // Missing closing tags

        let body = Bytes::from(xml);
        let result = parse_complete_multipart_upload(&body);

        // Should return an error for malformed XML
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_lifecycle_configuration_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>rule-1</ID>
        <Status>Enabled</Status>
        <Filter>
            <Prefix>logs/</Prefix>
        </Filter>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>"#;

        let config = parse_lifecycle_configuration(xml).unwrap();
        assert_eq!(config.rules.len(), 1);

        let rule = &config.rules[0];
        assert_eq!(rule.id.as_deref(), Some("rule-1"));
        assert_eq!(rule.status, LifecycleRuleStatus::Enabled);
        assert_eq!(
            rule.filter.as_ref().unwrap().prefix.as_deref(),
            Some("logs/")
        );
        assert_eq!(rule.expiration.as_ref().unwrap().days, Some(30));
    }

    #[test]
    fn test_parse_lifecycle_configuration_multiple_rules() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>rule-1</ID>
        <Status>Enabled</Status>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
    <Rule>
        <ID>rule-2</ID>
        <Status>Disabled</Status>
        <NoncurrentVersionExpiration>
            <NoncurrentDays>90</NoncurrentDays>
        </NoncurrentVersionExpiration>
    </Rule>
</LifecycleConfiguration>"#;

        let config = parse_lifecycle_configuration(xml).unwrap();
        assert_eq!(config.rules.len(), 2);

        assert_eq!(config.rules[0].id.as_deref(), Some("rule-1"));
        assert_eq!(config.rules[0].status, LifecycleRuleStatus::Enabled);
        assert_eq!(config.rules[0].expiration.as_ref().unwrap().days, Some(30));

        assert_eq!(config.rules[1].id.as_deref(), Some("rule-2"));
        assert_eq!(config.rules[1].status, LifecycleRuleStatus::Disabled);
        assert_eq!(
            config.rules[1]
                .noncurrent_version_expiration
                .as_ref()
                .unwrap()
                .noncurrent_days,
            90
        );
    }

    #[test]
    fn test_parse_lifecycle_abort_incomplete_multipart() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>abort-rule</ID>
        <Status>Enabled</Status>
        <AbortIncompleteMultipartUpload>
            <DaysAfterInitiation>7</DaysAfterInitiation>
        </AbortIncompleteMultipartUpload>
    </Rule>
</LifecycleConfiguration>"#;

        let config = parse_lifecycle_configuration(xml).unwrap();
        assert_eq!(config.rules.len(), 1);

        let rule = &config.rules[0];
        assert_eq!(
            rule.abort_incomplete_multipart_upload
                .as_ref()
                .unwrap()
                .days_after_initiation,
            7
        );
    }

    #[test]
    fn test_lifecycle_configuration_xml_generation() {
        let config = LifecycleConfiguration {
            rules: vec![LifecycleRule {
                id: Some("rule-1".to_string()),
                status: LifecycleRuleStatus::Enabled,
                filter: Some(LifecycleFilter {
                    prefix: Some("logs/".to_string()),
                }),
                expiration: Some(LifecycleExpiration {
                    days: Some(30),
                    date: None,
                    expired_object_delete_marker: None,
                }),
                noncurrent_version_expiration: None,
                abort_incomplete_multipart_upload: None,
            }],
        };

        let xml = lifecycle_configuration_xml(&config);

        assert!(xml.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<LifecycleConfiguration"));
        assert!(xml.contains("<ID>rule-1</ID>"));
        assert!(xml.contains("<Status>Enabled</Status>"));
        assert!(xml.contains("<Prefix>logs/</Prefix>"));
        assert!(xml.contains("<Days>30</Days>"));
    }

    #[test]
    fn test_lifecycle_validation_no_rules() {
        let config = LifecycleConfiguration { rules: vec![] };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must have at least one rule"));
    }

    #[test]
    fn test_lifecycle_validation_duplicate_ids() {
        let config = LifecycleConfiguration {
            rules: vec![
                LifecycleRule {
                    id: Some("rule-1".to_string()),
                    status: LifecycleRuleStatus::Enabled,
                    filter: None,
                    expiration: Some(LifecycleExpiration {
                        days: Some(30),
                        date: None,
                        expired_object_delete_marker: None,
                    }),
                    noncurrent_version_expiration: None,
                    abort_incomplete_multipart_upload: None,
                },
                LifecycleRule {
                    id: Some("rule-1".to_string()),
                    status: LifecycleRuleStatus::Enabled,
                    filter: None,
                    expiration: Some(LifecycleExpiration {
                        days: Some(60),
                        date: None,
                        expired_object_delete_marker: None,
                    }),
                    noncurrent_version_expiration: None,
                    abort_incomplete_multipart_upload: None,
                },
            ],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate rule ID"));
    }

    #[test]
    fn test_lifecycle_rule_validation_no_action() {
        let rule = LifecycleRule {
            id: Some("rule-1".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None,
            expiration: None,
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        };

        let result = rule.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least one action"));
    }

    #[test]
    fn test_lifecycle_expiration_validation_zero_days() {
        let exp = LifecycleExpiration {
            days: Some(0),
            date: None,
            expired_object_delete_marker: None,
        };

        let result = exp.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("greater than 0"));
    }

    #[test]
    fn test_lifecycle_expiration_validation_both_days_and_date() {
        let exp = LifecycleExpiration {
            days: Some(30),
            date: Some("2024-12-31".to_string()),
            expired_object_delete_marker: None,
        };

        let result = exp.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot have both"));
    }

    #[test]
    fn test_parse_complete_multipart_upload_large_part_numbers() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"part1"</ETag>
    </Part>
    <Part>
        <PartNumber>5000</PartNumber>
        <ETag>"part5000"</ETag>
    </Part>
    <Part>
        <PartNumber>10000</PartNumber>
        <ETag>"part10000"</ETag>
    </Part>
</CompleteMultipartUpload>"#;

        let body = Bytes::from(xml);
        let parts = parse_complete_multipart_upload(&body).unwrap();

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].part_number, 1);
        assert_eq!(parts[1].part_number, 5000);
        assert_eq!(parts[2].part_number, 10000);
    }

    // ========================================================================
    // Object Expiration Tests
    // ========================================================================

    #[test]
    fn test_should_expire_object_by_days() {
        let rules = vec![LifecycleRule {
            id: Some("expire-old-logs".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: Some(LifecycleFilter {
                prefix: Some("logs/".to_string()),
            }),
            expiration: Some(LifecycleExpiration {
                days: Some(30),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        // Object modified 31 days ago (should expire)
        let object_modified = 1000000;
        let current_time = object_modified + (31 * 86400);
        assert!(should_expire_object(
            "logs/2023/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Object modified 29 days ago (should not expire)
        let current_time = object_modified + (29 * 86400);
        assert!(!should_expire_object(
            "logs/2023/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Object modified exactly 30 days ago (should expire)
        let current_time = object_modified + (30 * 86400);
        assert!(should_expire_object(
            "logs/2023/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_object_prefix_filter() {
        let rules = vec![LifecycleRule {
            id: Some("expire-logs".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: Some(LifecycleFilter {
                prefix: Some("logs/".to_string()),
            }),
            expiration: Some(LifecycleExpiration {
                days: Some(30),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified + (31 * 86400);

        // Object with matching prefix (should expire)
        assert!(should_expire_object(
            "logs/2023/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Object without matching prefix (should not expire)
        assert!(!should_expire_object(
            "data/2023/data.csv",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Object with prefix as substring but not at start (should not expire)
        assert!(!should_expire_object(
            "my-logs/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_object_disabled_rule() {
        let rules = vec![LifecycleRule {
            id: Some("disabled-rule".to_string()),
            status: LifecycleRuleStatus::Disabled,
            filter: None,
            expiration: Some(LifecycleExpiration {
                days: Some(1),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified + (100 * 86400); // 100 days old

        // Should not expire because rule is disabled
        assert!(!should_expire_object(
            "any-object.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_object_by_date() {
        let rules = vec![LifecycleRule {
            id: Some("expire-by-date".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None,
            expiration: Some(LifecycleExpiration {
                days: None,
                date: Some("2024-01-01".to_string()),
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;

        // Current time is after expiration date (should expire)
        // 2024-01-02 = approx 1704153600 seconds since epoch
        let current_time = 1704153600 + 86400; // Jan 2, 2024
        assert!(should_expire_object(
            "test.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Current time is before expiration date (should not expire)
        // 2023-12-31 = approx 1704067200 seconds since epoch
        let current_time = 1703980800; // Dec 31, 2023
        assert!(!should_expire_object(
            "test.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_noncurrent_version() {
        let rules = vec![LifecycleRule {
            id: Some("expire-old-versions".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None,
            expiration: None,
            noncurrent_version_expiration: Some(NoncurrentVersionExpiration {
                noncurrent_days: 90,
            }),
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;

        // Noncurrent version 91 days old (should expire)
        let current_time = object_modified + (91 * 86400);
        assert!(should_expire_object(
            "test.txt",
            object_modified,
            false, // noncurrent version
            &rules,
            current_time
        ));

        // Noncurrent version 89 days old (should not expire)
        let current_time = object_modified + (89 * 86400);
        assert!(!should_expire_object(
            "test.txt",
            object_modified,
            false,
            &rules,
            current_time
        ));

        // Current version should not expire with noncurrent rule
        let current_time = object_modified + (100 * 86400);
        assert!(!should_expire_object(
            "test.txt",
            object_modified,
            true, // current version
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_multiple_rules() {
        let rules = vec![
            LifecycleRule {
                id: Some("expire-logs".to_string()),
                status: LifecycleRuleStatus::Enabled,
                filter: Some(LifecycleFilter {
                    prefix: Some("logs/".to_string()),
                }),
                expiration: Some(LifecycleExpiration {
                    days: Some(7),
                    date: None,
                    expired_object_delete_marker: None,
                }),
                noncurrent_version_expiration: None,
                abort_incomplete_multipart_upload: None,
            },
            LifecycleRule {
                id: Some("expire-temp".to_string()),
                status: LifecycleRuleStatus::Enabled,
                filter: Some(LifecycleFilter {
                    prefix: Some("tmp/".to_string()),
                }),
                expiration: Some(LifecycleExpiration {
                    days: Some(1),
                    date: None,
                    expired_object_delete_marker: None,
                }),
                noncurrent_version_expiration: None,
                abort_incomplete_multipart_upload: None,
            },
        ];

        let object_modified = 1000000;

        // Logs object 8 days old (matches first rule)
        let current_time = object_modified + (8 * 86400);
        assert!(should_expire_object(
            "logs/app.log",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Tmp object 2 days old (matches second rule)
        let current_time = object_modified + (2 * 86400);
        assert!(should_expire_object(
            "tmp/cache.dat",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Data object 100 days old (matches no rule)
        let current_time = object_modified + (100 * 86400);
        assert!(!should_expire_object(
            "data/important.csv",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_no_filter() {
        let rules = vec![LifecycleRule {
            id: Some("expire-all".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None, // No filter = applies to all objects
            expiration: Some(LifecycleExpiration {
                days: Some(365),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified + (366 * 86400);

        // All objects should match when there's no filter
        assert!(should_expire_object(
            "any/path/file.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
        assert!(should_expire_object(
            "file.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
        assert!(should_expire_object(
            "deeply/nested/path/to/file.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_should_expire_empty_prefix() {
        let rules = vec![LifecycleRule {
            id: Some("expire-all-with-empty-prefix".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: Some(LifecycleFilter {
                prefix: Some("".to_string()),
            }),
            expiration: Some(LifecycleExpiration {
                days: Some(30),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified + (31 * 86400);

        // Empty prefix should match all objects
        assert!(should_expire_object(
            "any-file.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_parse_iso8601_date() {
        // Test basic date format
        let timestamp = parse_iso8601_date("2024-01-01").unwrap();
        let expected = 1704067200; // 2024-01-01 00:00:00 UTC
        assert_eq!(timestamp, expected);

        // Test with time component
        let timestamp = parse_iso8601_date("2024-01-01T00:00:00.000Z").unwrap();
        assert_eq!(timestamp, expected);

        // Test leap year date
        let timestamp = parse_iso8601_date("2024-02-29").unwrap();
        let expected = 1709164800; // 2024-02-29 00:00:00 UTC
        assert_eq!(timestamp, expected);

        // Test invalid date
        assert!(parse_iso8601_date("invalid").is_none());
        assert!(parse_iso8601_date("2024-13-01").is_none()); // Invalid month
        assert!(parse_iso8601_date("2024-01-32").is_none()); // Invalid day
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000)); // Divisible by 400
        assert!(is_leap_year(2004)); // Divisible by 4
        assert!(is_leap_year(2024)); // Divisible by 4
        assert!(!is_leap_year(1900)); // Divisible by 100 but not 400
        assert!(!is_leap_year(2001)); // Not divisible by 4
        assert!(!is_leap_year(2100)); // Divisible by 100 but not 400
    }

    #[test]
    fn test_expiration_edge_cases() {
        // Test with current time exactly at object modified time
        let rules = vec![LifecycleRule {
            id: Some("test".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None,
            expiration: Some(LifecycleExpiration {
                days: Some(1),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: None,
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified; // Same time

        // Should not expire (0 days old)
        assert!(!should_expire_object(
            "test.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_noncurrent_version_with_prefix() {
        let rules = vec![LifecycleRule {
            id: Some("expire-old-temp-versions".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: Some(LifecycleFilter {
                prefix: Some("tmp/".to_string()),
            }),
            expiration: None,
            noncurrent_version_expiration: Some(NoncurrentVersionExpiration {
                noncurrent_days: 30,
            }),
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;
        let current_time = object_modified + (31 * 86400);

        // Noncurrent version with matching prefix (should expire)
        assert!(should_expire_object(
            "tmp/cache.dat",
            object_modified,
            false,
            &rules,
            current_time
        ));

        // Noncurrent version without matching prefix (should not expire)
        assert!(!should_expire_object(
            "data/file.dat",
            object_modified,
            false,
            &rules,
            current_time
        ));
    }

    #[test]
    fn test_mixed_current_and_noncurrent_rules() {
        let rules = vec![LifecycleRule {
            id: Some("mixed-rule".to_string()),
            status: LifecycleRuleStatus::Enabled,
            filter: None,
            expiration: Some(LifecycleExpiration {
                days: Some(30),
                date: None,
                expired_object_delete_marker: None,
            }),
            noncurrent_version_expiration: Some(NoncurrentVersionExpiration { noncurrent_days: 7 }),
            abort_incomplete_multipart_upload: None,
        }];

        let object_modified = 1000000;

        // Current version 31 days old (should expire by current rule)
        let current_time = object_modified + (31 * 86400);
        assert!(should_expire_object(
            "test.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));

        // Noncurrent version 8 days old (should expire by noncurrent rule)
        let current_time = object_modified + (8 * 86400);
        assert!(should_expire_object(
            "test.txt",
            object_modified,
            false,
            &rules,
            current_time
        ));

        // Current version 8 days old (should not expire - needs 30 days)
        assert!(!should_expire_object(
            "test.txt",
            object_modified,
            true,
            &rules,
            current_time
        ));
    }
}
