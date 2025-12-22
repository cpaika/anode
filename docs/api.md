# Anode S3 API Reference

Comprehensive reference documentation for Anode's S3-compatible API.

## Table of Contents

- [Authentication](#authentication)
- [Bucket Operations](#bucket-operations)
- [Object Operations](#object-operations)
- [Multipart Upload Operations](#multipart-upload-operations)
- [Batch Operations](#batch-operations)
- [Error Responses](#error-responses)

## Authentication

Anode supports AWS Signature Version 4 (SigV4) authentication for all API requests.

### AWS Signature V4

All authenticated requests must include an `Authorization` header with the following format:

```
Authorization: AWS4-HMAC-SHA256 Credential=<access_key>/<date>/<region>/s3/aws4_request, SignedHeaders=<signed_headers>, Signature=<signature>
```

### Required Headers

- `Authorization` - AWS SigV4 authorization header (if authentication is enabled)
- `x-amz-date` - ISO 8601 timestamp (format: `YYYYMMDDTHHMMSSZ`)
- `Host` - Target host

### Optional Headers

- `x-amz-content-sha256` - SHA256 hash of request payload (can be `UNSIGNED-PAYLOAD`)

### Configuration

Authentication can be disabled for testing/development:

```rust
// Disabled authentication (not recommended for production)
let auth_config = AuthConfig::disabled();

// Enabled authentication
let mut access_keys = HashMap::new();
access_keys.insert("AKIAIOSFODNN7EXAMPLE".to_string(), "secret_key".to_string());
let auth_config = AuthConfig::enabled(access_keys);
```

### Example with AWS CLI

```bash
# Configure credentials
aws configure set aws_access_key_id AKIAIOSFODNN7EXAMPLE
aws configure set aws_secret_access_key wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY

# Use with Anode
aws --endpoint-url http://localhost:8080 s3 ls
```

## Bucket Operations

### CreateBucket

Creates a new bucket.

**Method:** `PUT`
**Path:** `/{bucket}`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Length: 0`

**Errors:**
- `400 InvalidBucketName` - Bucket name doesn't meet requirements
- `409 BucketAlreadyExists` - Bucket with this name already exists

**Bucket Naming Rules:**
- 3-63 characters long
- Only lowercase letters, numbers, hyphens, and periods
- Cannot start or end with a hyphen
- No uppercase letters

**Example (curl):**
```bash
curl -X PUT http://localhost:8080/my-bucket \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Authorization: AWS4-HMAC-SHA256 Credential=..."
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket
```

---

### DeleteBucket

Deletes an empty bucket.

**Method:** `DELETE`
**Path:** `/{bucket}`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `204 No Content`

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist
- `409 BucketNotEmpty` - Bucket contains objects

**Example (curl):**
```bash
curl -X DELETE http://localhost:8080/my-bucket \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Authorization: AWS4-HMAC-SHA256 Credential=..."
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 rb s3://my-bucket
```

---

### HeadBucket

Checks if a bucket exists and you have permission to access it.

**Method:** `HEAD`
**Path:** `/{bucket}`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Example (curl):**
```bash
curl -I -X HEAD http://localhost:8080/my-bucket \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Authorization: AWS4-HMAC-SHA256 Credential=..."
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3api head-bucket --bucket my-bucket
```

---

### ListBuckets

Lists all buckets.

**Method:** `GET`
**Path:** `/`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Owner>
        <ID>anode-owner</ID>
        <DisplayName>anode</DisplayName>
    </Owner>
    <Buckets>
        <Bucket>
            <Name>bucket1</Name>
            <CreationDate>2023-12-19T10:30:00.000Z</CreationDate>
        </Bucket>
        <Bucket>
            <Name>bucket2</Name>
            <CreationDate>2023-12-19T11:00:00.000Z</CreationDate>
        </Bucket>
    </Buckets>
</ListAllMyBucketsResult>
```

**Example (curl):**
```bash
curl http://localhost:8080/ \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Authorization: AWS4-HMAC-SHA256 Credential=..."
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 ls
```

## Object Operations

### PutObject

Uploads an object to a bucket.

**Method:** `PUT`
**Path:** `/{bucket}/{key}`

**Request Headers:**
- Standard authentication headers
- `Content-Length` - Size of the object (required)
- `Content-Type` - MIME type (optional, stored as metadata)
- `x-amz-meta-*` - Custom metadata (optional, any number of headers)

**Request Body:** Object data (binary)

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `ETag: "sha256_hash"` - SHA256 hash of the object (quoted, hex-encoded)
  - `Content-Length: 0`

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Custom Metadata:**
All headers with prefix `x-amz-meta-` are stored as object metadata. The prefix is stripped when storing, and restored when retrieving.

**Example (curl):**
```bash
# Simple upload
curl -X PUT http://localhost:8080/my-bucket/file.txt \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Content-Type: text/plain" \
  -H "x-amz-meta-author: John Doe" \
  -H "x-amz-meta-version: 1.0" \
  --data-binary @file.txt

# Upload with metadata
echo "Hello World" | curl -X PUT http://localhost:8080/my-bucket/hello.txt \
  -H "Content-Type: text/plain" \
  -H "x-amz-meta-description: A greeting" \
  --data-binary @-
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://my-bucket/file.txt \
  --metadata author="John Doe",version="1.0"
```

---

### GetObject

Downloads an object from a bucket.

**Method:** `GET`
**Path:** `/{bucket}/{key}`

**Request Headers:**
- Standard authentication headers
- `Range` - Byte range to retrieve (optional)

**Request Body:** None

**Response:**
- **Status:** `200 OK` (or `206 Partial Content` for range requests)
- **Headers:**
  - `Content-Type` - Stored content type (default: `application/octet-stream`)
  - `Content-Length` - Size of object data
  - `x-amz-meta-*` - Custom metadata headers (if any were set during upload)
- **Body:** Object data (binary)

**Range Requests:**
The `Range` header is currently parsed but not fully implemented. Future support will include:
- `bytes=0-100` - Bytes 0 through 100 (inclusive)
- `bytes=100-` - Byte 100 to end of file
- `bytes=-100` - Last 100 bytes

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist
- `404 NoSuchKey` - Object doesn't exist

**Example (curl):**
```bash
# Download entire object
curl http://localhost:8080/my-bucket/file.txt \
  -H "x-amz-date: 20231219T120000Z" \
  -o file.txt

# Range request (when implemented)
curl http://localhost:8080/my-bucket/file.txt \
  -H "Range: bytes=0-1023" \
  -o partial.txt
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/file.txt ./file.txt
```

---

### HeadObject

Retrieves metadata about an object without returning the object data.

**Method:** `HEAD`
**Path:** `/{bucket}/{key}`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type` - Stored content type
  - `Content-Length` - Size of object
  - `ETag: "sha256_hash"` - SHA256 hash of the object (quoted)
  - `x-amz-meta-*` - Custom metadata headers

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist
- `404 NoSuchKey` - Object doesn't exist

**Example (curl):**
```bash
curl -I http://localhost:8080/my-bucket/file.txt \
  -H "x-amz-date: 20231219T120000Z"
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3api head-object \
  --bucket my-bucket \
  --key file.txt
```

---

### DeleteObject

Deletes an object from a bucket.

**Method:** `DELETE`
**Path:** `/{bucket}/{key}`

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `204 No Content`

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Note:** S3 compatibility - DeleteObject returns success even if the object doesn't exist.

**Example (curl):**
```bash
curl -X DELETE http://localhost:8080/my-bucket/file.txt \
  -H "x-amz-date: 20231219T120000Z"
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 rm s3://my-bucket/file.txt
```

---

### CopyObject

Copies an object from one location to another within Anode (server-side copy).

**Method:** `PUT`
**Path:** `/{dest-bucket}/{dest-key}`

**Request Headers:**
- Standard authentication headers
- `x-amz-copy-source: /{source-bucket}/{source-key}` - Source object (required)

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<CopyObjectResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <LastModified>2023-12-19T10:30:00.000Z</LastModified>
    <ETag>"sha256_hash"</ETag>
</CopyObjectResult>
```

**Errors:**
- `400 InvalidRequest` - Missing or invalid `x-amz-copy-source` header
- `404 NoSuchBucket` - Source or destination bucket doesn't exist
- `404 NoSuchKey` - Source object doesn't exist

**Example (curl):**
```bash
curl -X PUT http://localhost:8080/my-bucket/copy.txt \
  -H "x-amz-date: 20231219T120000Z" \
  -H "x-amz-copy-source: /my-bucket/original.txt"
```

**Example (AWS CLI):**
```bash
aws --endpoint-url http://localhost:8080 s3 cp \
  s3://my-bucket/original.txt \
  s3://my-bucket/copy.txt
```

---

### ListObjectsV2

Lists objects in a bucket with pagination support.

**Method:** `GET`
**Path:** `/{bucket}`

**Query Parameters:**
- `list-type=2` - Required to use ListObjectsV2 (otherwise uses ListObjects v1)
- `prefix` - Limits response to keys beginning with prefix (optional)
- `delimiter` - Character used to group keys (optional, typically `/`)
- `max-keys` - Maximum number of keys to return (optional, default: 1000, max: 1000)
- `continuation-token` - Token from previous response for pagination (optional)
- `start-after` - Start listing after this key (optional)

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Name>my-bucket</Name>
    <Prefix>logs/</Prefix>
    <KeyCount>2</KeyCount>
    <MaxKeys>1000</MaxKeys>
    <IsTruncated>false</IsTruncated>
    <Contents>
        <Key>logs/2023-12-19.log</Key>
        <LastModified>2023-12-19T10:30:00.000Z</LastModified>
        <ETag>"abc123"</ETag>
        <Size>1024</Size>
        <StorageClass>STANDARD</StorageClass>
    </Contents>
    <Contents>
        <Key>logs/2023-12-20.log</Key>
        <LastModified>2023-12-20T10:30:00.000Z</LastModified>
        <ETag>"def456"</ETag>
        <Size>2048</Size>
        <StorageClass>STANDARD</StorageClass>
    </Contents>
</ListBucketResult>
```

**Pagination:**
If `IsTruncated` is `true`, use `NextContinuationToken` for the next request.

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Example (curl):**
```bash
# List all objects
curl "http://localhost:8080/my-bucket?list-type=2" \
  -H "x-amz-date: 20231219T120000Z"

# List with prefix
curl "http://localhost:8080/my-bucket?list-type=2&prefix=logs/" \
  -H "x-amz-date: 20231219T120000Z"

# List with pagination
curl "http://localhost:8080/my-bucket?list-type=2&max-keys=10" \
  -H "x-amz-date: 20231219T120000Z"
```

**Example (AWS CLI):**
```bash
# List all objects
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/

# List with prefix
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/logs/

# Recursive list
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/ --recursive
```

## Multipart Upload Operations

For large objects (typically >100MB), multipart upload provides better performance and reliability.

### Multipart Upload Limits

- **Minimum part size:** 5 MB (except last part, which can be smaller)
- **Maximum part size:** 5 GB
- **Maximum parts:** 10,000 per upload
- **Maximum object size:** 5 TB
- **Part numbers:** 1-10,000

### CreateMultipartUpload

Initiates a multipart upload and returns an upload ID.

**Method:** `POST`
**Path:** `/{bucket}/{key}?uploads`

**Query Parameters:**
- `uploads` - Required flag to initiate multipart upload

**Request Headers:**
- Standard authentication headers
- `x-amz-meta-*` - Custom metadata (optional, applied to final object)

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>my-bucket</Bucket>
    <Key>large-file.bin</Key>
    <UploadId>uuid-upload-id-12345</UploadId>
</InitiateMultipartUploadResult>
```

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Example (curl):**
```bash
curl -X POST "http://localhost:8080/my-bucket/large-file.bin?uploads" \
  -H "x-amz-date: 20231219T120000Z" \
  -H "x-amz-meta-description: Large file upload"
```

---

### UploadPart

Uploads a single part of a multipart upload.

**Method:** `PUT`
**Path:** `/{bucket}/{key}?partNumber={part_number}&uploadId={upload_id}`

**Query Parameters:**
- `partNumber` - Part number (1-10,000, required)
- `uploadId` - Upload ID from CreateMultipartUpload (required)

**Request Headers:**
- Standard authentication headers
- `Content-Length` - Size of part data (required)

**Request Body:** Part data (binary)

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `ETag: "sha256_hash"` - Hash of this part (quoted, save for CompleteMultipartUpload)

**Errors:**
- `400 InvalidArgument` - Invalid part number or missing uploadId
- `400 EntityTooLarge` - Part exceeds 5GB
- `404 NoSuchUpload` - Upload ID doesn't exist or expired

**Example (curl):**
```bash
# Upload part 1 (5MB)
dd if=/dev/zero bs=1M count=5 | curl -X PUT \
  "http://localhost:8080/my-bucket/large-file.bin?partNumber=1&uploadId=uuid-12345" \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Content-Length: 5242880" \
  --data-binary @-
```

---

### CompleteMultipartUpload

Completes a multipart upload by assembling previously uploaded parts.

**Method:** `POST`
**Path:** `/{bucket}/{key}?uploadId={upload_id}`

**Query Parameters:**
- `uploadId` - Upload ID from CreateMultipartUpload (required)

**Request Headers:**
- Standard authentication headers

**Request Body:** XML list of parts

**Request Format:**
```xml
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"part1-etag"</ETag>
    </Part>
    <Part>
        <PartNumber>2</PartNumber>
        <ETag>"part2-etag"</ETag>
    </Part>
</CompleteMultipartUpload>
```

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
  - `ETag: "final-etag"` - Final object ETag
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Location>/my-bucket/large-file.bin</Location>
    <Bucket>my-bucket</Bucket>
    <Key>large-file.bin</Key>
    <ETag>"final-sha256-hash"</ETag>
</CompleteMultipartUploadResult>
```

**Validation:**
- Parts must be in ascending order by part number
- All parts must exist and ETags must match
- All parts except the last must be >= 5MB

**Errors:**
- `400 InvalidRequest` - Missing uploadId or empty parts list
- `400 InvalidPart` - Part doesn't exist or ETag mismatch
- `400 InvalidPartOrder` - Parts not in ascending order
- `400 EntityTooSmall` - Non-final part is < 5MB
- `404 NoSuchUpload` - Upload ID doesn't exist

**Example (curl):**
```bash
curl -X POST "http://localhost:8080/my-bucket/large-file.bin?uploadId=uuid-12345" \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Content-Type: application/xml" \
  -d '<CompleteMultipartUpload>
    <Part>
      <PartNumber>1</PartNumber>
      <ETag>"etag1"</ETag>
    </Part>
    <Part>
      <PartNumber>2</PartNumber>
      <ETag>"etag2"</ETag>
    </Part>
  </CompleteMultipartUpload>'
```

---

### AbortMultipartUpload

Aborts a multipart upload and frees storage used by uploaded parts.

**Method:** `DELETE`
**Path:** `/{bucket}/{key}?uploadId={upload_id}`

**Query Parameters:**
- `uploadId` - Upload ID from CreateMultipartUpload (required)

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `204 No Content`

**Errors:**
- `400 InvalidRequest` - Missing uploadId
- `404 NoSuchUpload` - Upload ID doesn't exist

**Example (curl):**
```bash
curl -X DELETE \
  "http://localhost:8080/my-bucket/large-file.bin?uploadId=uuid-12345" \
  -H "x-amz-date: 20231219T120000Z"
```

---

### ListParts

Lists the parts that have been uploaded for a specific multipart upload.

**Method:** `GET`
**Path:** `/{bucket}/{key}?uploadId={upload_id}`

**Query Parameters:**
- `uploadId` - Upload ID from CreateMultipartUpload (required)
- `max-parts` - Maximum parts to return (optional, default: 1000, max: 1000)
- `part-number-marker` - List parts after this part number (optional, for pagination)

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListPartsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>my-bucket</Bucket>
    <Key>large-file.bin</Key>
    <UploadId>uuid-12345</UploadId>
    <StorageClass>STANDARD</StorageClass>
    <PartNumberMarker>0</PartNumberMarker>
    <MaxParts>1000</MaxParts>
    <IsTruncated>false</IsTruncated>
    <Part>
        <PartNumber>1</PartNumber>
        <LastModified>2023-12-19T10:30:00.000Z</LastModified>
        <ETag>"etag1"</ETag>
        <Size>5242880</Size>
    </Part>
    <Part>
        <PartNumber>2</PartNumber>
        <LastModified>2023-12-19T10:35:00.000Z</LastModified>
        <ETag>"etag2"</ETag>
        <Size>5242880</Size>
    </Part>
</ListPartsResult>
```

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist
- `404 NoSuchUpload` - Upload ID doesn't exist

**Example (curl):**
```bash
curl "http://localhost:8080/my-bucket/large-file.bin?uploadId=uuid-12345" \
  -H "x-amz-date: 20231219T120000Z"
```

---

### ListMultipartUploads

Lists in-progress multipart uploads for a bucket.

**Method:** `GET`
**Path:** `/{bucket}?uploads`

**Query Parameters:**
- `uploads` - Required flag to list multipart uploads
- `prefix` - Filter uploads by key prefix (optional)
- `max-uploads` - Maximum uploads to return (optional, default: 1000, max: 1000)
- `key-marker` - List uploads after this key (optional, for pagination)
- `upload-id-marker` - List uploads after this upload ID (optional, for pagination)

**Request Headers:**
- Standard authentication headers

**Request Body:** None

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListMultipartUploadsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Bucket>my-bucket</Bucket>
    <KeyMarker></KeyMarker>
    <UploadIdMarker></UploadIdMarker>
    <MaxUploads>1000</MaxUploads>
    <IsTruncated>false</IsTruncated>
    <Upload>
        <Key>large-file-1.bin</Key>
        <UploadId>uuid-12345</UploadId>
        <Initiated>2023-12-19T10:00:00.000Z</Initiated>
        <StorageClass>STANDARD</StorageClass>
    </Upload>
    <Upload>
        <Key>large-file-2.bin</Key>
        <UploadId>uuid-67890</UploadId>
        <Initiated>2023-12-19T11:00:00.000Z</Initiated>
        <StorageClass>STANDARD</StorageClass>
    </Upload>
</ListMultipartUploadsResult>
```

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist

**Example (curl):**
```bash
# List all multipart uploads
curl "http://localhost:8080/my-bucket?uploads" \
  -H "x-amz-date: 20231219T120000Z"

# List with prefix
curl "http://localhost:8080/my-bucket?uploads&prefix=backups/" \
  -H "x-amz-date: 20231219T120000Z"
```

## Batch Operations

### DeleteObjects

Deletes multiple objects in a single request (batch delete).

**Method:** `POST`
**Path:** `/{bucket}?delete`

**Query Parameters:**
- `delete` - Required flag for batch delete

**Request Headers:**
- Standard authentication headers
- `Content-Type: application/xml`

**Request Body:** XML list of objects to delete

**Request Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Quiet>false</Quiet>
    <Object>
        <Key>file1.txt</Key>
    </Object>
    <Object>
        <Key>file2.txt</Key>
    </Object>
</Delete>
```

**Request Options:**
- `Quiet` - If `true`, only returns errors (not successful deletions)

**Response:**
- **Status:** `200 OK`
- **Headers:**
  - `Content-Type: application/xml`
- **Body:** XML response

**Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Deleted>
        <Key>file1.txt</Key>
    </Deleted>
    <Deleted>
        <Key>file2.txt</Key>
    </Deleted>
</DeleteResult>
```

**Error Response Format:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Deleted>
        <Key>file1.txt</Key>
    </Deleted>
    <Error>
        <Key>file2.txt</Key>
        <Code>AccessDenied</Code>
        <Message>Access Denied</Message>
    </Error>
</DeleteResult>
```

**Limits:**
- Maximum 1000 objects per request

**Errors:**
- `404 NoSuchBucket` - Bucket doesn't exist
- `400 InvalidRequest` - Invalid XML format

**Example (curl):**
```bash
curl -X POST "http://localhost:8080/my-bucket?delete" \
  -H "x-amz-date: 20231219T120000Z" \
  -H "Content-Type: application/xml" \
  -d '<?xml version="1.0" encoding="UTF-8"?>
<Delete>
    <Quiet>false</Quiet>
    <Object>
        <Key>file1.txt</Key>
    </Object>
    <Object>
        <Key>file2.txt</Key>
    </Object>
</Delete>'
```

**Example (AWS CLI):**
```bash
# Delete multiple files
aws --endpoint-url http://localhost:8080 s3 rm s3://my-bucket/ \
  --recursive --exclude "*" --include "*.log"
```

## Error Responses

All errors return an XML response with details about the error.

### Error Response Format

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>ErrorCode</Code>
    <Message>Human-readable error message</Message>
</Error>
```

### Error Codes

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| `AccessDenied` | 403 | Access to the resource is denied |
| `BucketAlreadyExists` | 409 | Bucket with this name already exists |
| `BucketNotEmpty` | 409 | Bucket must be empty to delete |
| `NoSuchBucket` | 404 | The specified bucket does not exist |
| `NoSuchKey` | 404 | The specified object key does not exist |
| `InvalidBucketName` | 400 | Bucket name doesn't meet naming requirements |
| `InvalidArgument` | 400 | Invalid request argument or parameter |
| `InvalidRequest` | 400 | The request is malformed or invalid |
| `MissingContentLength` | 411 | Content-Length header is required but missing |
| `InvalidRange` | 416 | The requested byte range is invalid |
| `PreconditionFailed` | 412 | Conditional request failed |
| `InvalidPart` | 400 | Invalid multipart upload part |
| `NoSuchUpload` | 404 | The specified multipart upload does not exist |
| `InvalidPartOrder` | 400 | Parts must be in ascending order |
| `EntityTooSmall` | 400 | Part size is too small (minimum 5MB except last) |
| `EntityTooLarge` | 400 | Part or object size exceeds maximum |
| `InternalError` | 500 | Server encountered an internal error |
| `MethodNotAllowed` | 405 | HTTP method not allowed for this resource |
| `InvalidAccessKeyId` | 403 | The access key ID does not exist |
| `SignatureDoesNotMatch` | 403 | Request signature does not match |

### Example Error Response

**Request:**
```bash
curl http://localhost:8080/nonexistent-bucket/file.txt
```

**Response:**
```http
HTTP/1.1 404 Not Found
Content-Type: application/xml

<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchBucket</Code>
    <Message>No such bucket: nonexistent-bucket</Message>
</Error>
```

## Compatibility Notes

### AWS S3 Compatibility

Anode implements a subset of the S3 API with high compatibility:

**Fully Compatible:**
- Bucket operations (Create, Delete, List, Head)
- Object operations (Put, Get, Delete, Head, Copy, List)
- Multipart uploads (all operations)
- Batch delete
- AWS Signature V4 authentication
- Custom metadata (`x-amz-meta-*`)

**Partially Compatible:**
- Range requests (parsed but not fully implemented yet)
- Conditional requests (headers accepted but not evaluated)

**Not Yet Implemented:**
- Object versioning
- Server-side encryption (SSE-S3, SSE-C, SSE-KMS)
- Object lifecycle policies
- Bucket policies and ACLs
- Object locking (WORM)
- S3 Select (query in place)
- Event notifications
- Cross-region replication
- Presigned URLs (signed URLs work, but no special presign endpoint)

### Testing Compatibility

You can test Anode with standard S3 tools:

**AWS CLI:**
```bash
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
aws --endpoint-url http://localhost:8080 s3 ls
```

**boto3 (Python):**
```python
import boto3

s3 = boto3.client(
    's3',
    endpoint_url='http://localhost:8080',
    aws_access_key_id='test',
    aws_secret_access_key='test'
)

s3.create_bucket(Bucket='test')
s3.put_object(Bucket='test', Key='file.txt', Body=b'Hello')
```

**AWS SDK for Rust:**
```rust
use aws_sdk_s3::Client;
use aws_config::BehaviorVersion;

let config = aws_config::defaults(BehaviorVersion::latest())
    .endpoint_url("http://localhost:8080")
    .load()
    .await;

let client = Client::new(&config);
```

### ETag Calculation

Anode uses SHA-256 for ETag calculation instead of MD5:

- **Single-part uploads:** `ETag = "hex(sha256(object_data))"`
- **Multipart uploads:** `ETag = "hex(sha256(part1_data + part2_data + ...))"`

This differs from AWS S3 which uses:
- **Single-part:** MD5
- **Multipart:** MD5 of concatenated part MD5s with `-N` suffix

Most S3 clients treat ETags as opaque identifiers, so this difference is usually not an issue.

### Bucket Naming

Anode enforces S3 bucket naming rules:
- 3-63 characters
- Lowercase letters, numbers, hyphens, periods only
- Cannot start or end with hyphen
- No IP address format (e.g., 192.168.1.1)

### Cleanup and Expiration

- **Multipart uploads:** Automatically cleaned up after 7 days (configurable)
- **Deleted objects:** Immediately removed (no versioning/undelete support yet)

## Performance Considerations

### Chunking

Anode chunks all objects into 4MB chunks internally for deduplication and efficient storage:

- Small objects (< 4MB): Stored as single chunk
- Large objects: Split into multiple 4MB chunks
- Last chunk: May be smaller than 4MB

### Multipart Upload Best Practices

1. **Part size:** Use 5MB-100MB parts for best performance
2. **Parallel uploads:** Upload parts concurrently for faster transfers
3. **Abort incomplete uploads:** Clean up failed uploads to free storage
4. **Monitor expiration:** Uploads auto-expire after 7 days

### Request Rate Limits

Anode does not currently enforce rate limits, but consider:

- Limiting concurrent connections from clients
- Implementing client-side backoff/retry logic
- Monitoring server load and adjusting client behavior

### Large Object Uploads

For objects > 100MB:
- Always use multipart upload
- Upload parts in parallel
- Implement retry logic for failed parts
- Consider compressing data before upload

## Configuration

### Disabling Authentication

For development/testing, authentication can be disabled:

```rust
let auth = AuthConfig::disabled();
let router = S3Router::new(storage, Some(auth));
```

**Warning:** Never disable authentication in production!

### Multipart Configuration

```rust
use std::time::Duration;

let config = MultipartConfig {
    min_part_size: 5 * 1024 * 1024,          // 5MB
    max_part_size: 5 * 1024 * 1024 * 1024,   // 5GB
    max_parts: 10000,
    upload_expiry: Duration::from_secs(7 * 24 * 3600),  // 7 days
    cleanup_interval: Duration::from_secs(3600),         // 1 hour
};
```

## See Also

- [S3 API Overview](s3-api.md) - High-level overview and compatibility matrix
- [Configuration Guide](configuration.md) - Server configuration options
- [Security Guide](security.md) - Authentication and security best practices
- [Performance Guide](performance.md) - Performance tuning and optimization
