# S3 API Reference

Anode implements a subset of the Amazon S3 API, providing compatibility with existing S3 tools and libraries.

## API Compatibility

Anode supports the following S3 operations:

### Bucket Operations

| Operation | Support | Notes |
|-----------|---------|-------|
| CreateBucket | ✅ | Creates bucket with default configuration |
| DeleteBucket | ✅ | Bucket must be empty |
| ListBuckets | ✅ | Lists all buckets |
| HeadBucket | ✅ | Checks if bucket exists |
| GetBucketLocation | ✅ | Always returns configured region |
| PutBucketTagging | ⚠️ | Partial support |
| GetBucketTagging | ⚠️ | Partial support |
| DeleteBucketTagging | ⚠️ | Partial support |
| PutBucketVersioning | ❌ | Not yet implemented |
| GetBucketVersioning | ❌ | Not yet implemented |
| PutBucketPolicy | ❌ | Planned |
| GetBucketPolicy | ❌ | Planned |

### Object Operations

| Operation | Support | Notes |
|-----------|---------|-------|
| PutObject | ✅ | Single PUT up to 5GB |
| GetObject | ✅ | Supports range requests |
| DeleteObject | ✅ | Soft delete with eventual GC |
| DeleteObjects | ✅ | Batch delete (max 1000) |
| HeadObject | ✅ | Returns metadata without body |
| CopyObject | ✅ | Server-side copy |
| ListObjectsV2 | ✅ | Preferred list method |
| ListObjects | ✅ | Legacy list method |
| ListObjectVersions | ❌ | Requires versioning support |
| GetObjectAttributes | ⚠️ | Partial support |
| PutObjectTagging | ✅ | Object-level tags |
| GetObjectTagging | ✅ | Retrieves tags |
| DeleteObjectTagging | ✅ | Removes all tags |
| PutObjectAcl | ❌ | Planned |
| GetObjectAcl | ❌ | Planned |
| RestoreObject | ❌ | Not applicable |

### Multipart Upload

| Operation | Support | Notes |
|-----------|---------|-------|
| CreateMultipartUpload | ✅ | Initiates multipart upload |
| UploadPart | ✅ | Upload individual parts |
| UploadPartCopy | ⚠️ | Partial support |
| CompleteMultipartUpload | ✅ | Finalizes upload |
| AbortMultipartUpload | ✅ | Cancels upload |
| ListMultipartUploads | ✅ | Lists in-progress uploads |
| ListParts | ✅ | Lists parts of upload |

### Presigned URLs

| Operation | Support | Notes |
|-----------|---------|-------|
| Presigned GET | ✅ | Temporary download URLs |
| Presigned PUT | ✅ | Temporary upload URLs |
| Presigned DELETE | ✅ | Temporary delete URLs |
| Presigned POST | ⚠️ | Partial support |

## Authentication

Anode supports AWS Signature Version 4 (SigV4) authentication.

### Request Signing

All requests must include an Authorization header:

```
Authorization: AWS4-HMAC-SHA256
Credential=AKIAIOSFODNN7EXAMPLE/20231219/us-east-1/s3/aws4_request,
SignedHeaders=host;x-amz-content-sha256;x-amz-date,
Signature=<signature>
```

### Required Headers

```
x-amz-date: 20231219T103000Z
x-amz-content-sha256: <SHA256 hash of body>
Host: localhost:8080
```

### Generating Signatures

Use AWS SDKs which automatically handle signing:

**AWS CLI:**
```bash
aws configure set aws_access_key_id YOUR_ACCESS_KEY
aws configure set aws_secret_access_key YOUR_SECRET_KEY
aws --endpoint-url http://localhost:8080 s3 ls
```

**Python (boto3):**
```python
import boto3

s3 = boto3.client(
    's3',
    endpoint_url='http://localhost:8080',
    aws_access_key_id='YOUR_ACCESS_KEY',
    aws_secret_access_key='YOUR_SECRET_KEY'
)
```

**Rust:**
```rust
use aws_sdk_s3::Client;
use aws_config::BehaviorVersion;

let config = aws_config::defaults(BehaviorVersion::latest())
    .endpoint_url("http://localhost:8080")
    .load()
    .await;

let client = Client::new(&config);
```

## Bucket Operations

### CreateBucket

Creates a new bucket.

**Request:**
```http
PUT /{bucket} HTTP/1.1
Host: localhost:8080
x-amz-date: 20231219T103000Z
```

**Response:**
```http
HTTP/1.1 200 OK
Location: /{bucket}
```

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket
```

### DeleteBucket

Deletes an empty bucket.

**Request:**
```http
DELETE /{bucket} HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 204 No Content
```

**Errors:**
- `BucketNotEmpty` (409) - Bucket contains objects
- `NoSuchBucket` (404) - Bucket doesn't exist

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 rb s3://my-bucket
```

### ListBuckets

Lists all buckets.

**Request:**
```http
GET / HTTP/1.1
Host: localhost:8080
```

**Response:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListAllMyBucketsResult>
  <Owner>
    <ID>user123</ID>
    <DisplayName>user123</DisplayName>
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

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 ls
```

### HeadBucket

Checks if bucket exists.

**Request:**
```http
HEAD /{bucket} HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 200 OK
```

**Errors:**
- `NoSuchBucket` (404) - Bucket doesn't exist

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3api head-bucket --bucket my-bucket
```

## Object Operations

### PutObject

Uploads an object.

**Request:**
```http
PUT /{bucket}/{key} HTTP/1.1
Host: localhost:8080
Content-Length: 1024
Content-Type: text/plain
x-amz-metadata-key: value

<object data>
```

**Response:**
```http
HTTP/1.1 200 OK
ETag: "abc123def456"
```

**Optional Headers:**
- `Content-Type` - MIME type
- `Content-Encoding` - Encoding (gzip, etc.)
- `Content-Disposition` - Download filename
- `x-amz-metadata-*` - Custom metadata
- `x-amz-tagging` - Object tags

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://my-bucket/
```

### GetObject

Downloads an object.

**Request:**
```http
GET /{bucket}/{key} HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Length: 1024
ETag: "abc123def456"
Last-Modified: Tue, 19 Dec 2023 10:30:00 GMT

<object data>
```

**Range Requests:**
```http
GET /{bucket}/{key} HTTP/1.1
Range: bytes=0-1023
```

**Response:**
```http
HTTP/1.1 206 Partial Content
Content-Range: bytes 0-1023/10240
Content-Length: 1024

<partial object data>
```

**Conditional Requests:**
- `If-Match: "etag"` - Return only if ETag matches
- `If-None-Match: "etag"` - Return only if ETag doesn't match
- `If-Modified-Since: date` - Return only if modified since date
- `If-Unmodified-Since: date` - Return only if not modified since date

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/file.txt .
```

### DeleteObject

Deletes an object.

**Request:**
```http
DELETE /{bucket}/{key} HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 204 No Content
```

**Note:** Deletion is eventually consistent. Garbage collection runs periodically.

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 rm s3://my-bucket/file.txt
```

### HeadObject

Retrieves object metadata without body.

**Request:**
```http
HEAD /{bucket}/{key} HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Length: 1024
ETag: "abc123def456"
Last-Modified: Tue, 19 Dec 2023 10:30:00 GMT
x-amz-metadata-key: value
```

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3api head-object \
    --bucket my-bucket \
    --key file.txt
```

### CopyObject

Server-side copy of an object.

**Request:**
```http
PUT /{dest-bucket}/{dest-key} HTTP/1.1
Host: localhost:8080
x-amz-copy-source: /{source-bucket}/{source-key}
```

**Response:**
```http
HTTP/1.1 200 OK

<CopyObjectResult>
  <ETag>"abc123def456"</ETag>
  <LastModified>2023-12-19T10:30:00.000Z</LastModified>
</CopyObjectResult>
```

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 cp \
    s3://my-bucket/file.txt \
    s3://my-bucket/file-copy.txt
```

### ListObjectsV2

Lists objects in a bucket (recommended method).

**Request:**
```http
GET /{bucket}?list-type=2 HTTP/1.1
Host: localhost:8080
```

**Query Parameters:**
- `prefix` - Filter by prefix
- `delimiter` - Group by delimiter (for folder simulation)
- `max-keys` - Maximum objects to return (default 1000)
- `continuation-token` - Pagination token
- `start-after` - Start listing after this key

**Response:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult>
  <Name>my-bucket</Name>
  <Prefix></Prefix>
  <KeyCount>2</KeyCount>
  <MaxKeys>1000</MaxKeys>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>file1.txt</Key>
    <LastModified>2023-12-19T10:30:00.000Z</LastModified>
    <ETag>"abc123"</ETag>
    <Size>1024</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>file2.txt</Key>
    <LastModified>2023-12-19T11:00:00.000Z</LastModified>
    <ETag>"def456"</ETag>
    <Size>2048</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
</ListBucketResult>
```

**Example:**
```bash
# List all objects
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/

# List with prefix
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/logs/

# List recursively
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/ --recursive
```

## Multipart Upload

For objects larger than 100MB, use multipart upload for better performance and reliability.

### CreateMultipartUpload

Initiates a multipart upload.

**Request:**
```http
POST /{bucket}/{key}?uploads HTTP/1.1
Host: localhost:8080
```

**Response:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult>
  <Bucket>my-bucket</Bucket>
  <Key>large-file.bin</Key>
  <UploadId>upload123</UploadId>
</InitiateMultipartUploadResult>
```

**Example:**
```python
import boto3

s3 = boto3.client('s3', endpoint_url='http://localhost:8080')
response = s3.create_multipart_upload(
    Bucket='my-bucket',
    Key='large-file.bin'
)
upload_id = response['UploadId']
```

### UploadPart

Uploads a part of the object.

**Request:**
```http
PUT /{bucket}/{key}?partNumber=1&uploadId=upload123 HTTP/1.1
Host: localhost:8080
Content-Length: 5242880

<part data>
```

**Response:**
```http
HTTP/1.1 200 OK
ETag: "part1-etag"
```

**Requirements:**
- Part size: 5MB to 5GB (except last part)
- Part numbers: 1 to 10,000
- Maximum parts: 10,000
- Maximum object size: 5TB

**Example:**
```python
# Upload part
with open('large-file.bin', 'rb') as f:
    f.seek(0)  # First part
    data = f.read(5 * 1024 * 1024)  # 5MB
    response = s3.upload_part(
        Bucket='my-bucket',
        Key='large-file.bin',
        PartNumber=1,
        UploadId=upload_id,
        Body=data
    )
    part1_etag = response['ETag']
```

### CompleteMultipartUpload

Finalizes the multipart upload.

**Request:**
```http
POST /{bucket}/{key}?uploadId=upload123 HTTP/1.1
Host: localhost:8080

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
```xml
<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult>
  <Location>http://localhost:8080/my-bucket/large-file.bin</Location>
  <Bucket>my-bucket</Bucket>
  <Key>large-file.bin</Key>
  <ETag>"final-etag"</ETag>
</CompleteMultipartUploadResult>
```

**Example:**
```python
# Complete upload
s3.complete_multipart_upload(
    Bucket='my-bucket',
    Key='large-file.bin',
    UploadId=upload_id,
    MultipartUpload={
        'Parts': [
            {'ETag': part1_etag, 'PartNumber': 1},
            {'ETag': part2_etag, 'PartNumber': 2},
        ]
    }
)
```

### AbortMultipartUpload

Cancels a multipart upload.

**Request:**
```http
DELETE /{bucket}/{key}?uploadId=upload123 HTTP/1.1
Host: localhost:8080
```

**Response:**
```http
HTTP/1.1 204 No Content
```

**Example:**
```python
s3.abort_multipart_upload(
    Bucket='my-bucket',
    Key='large-file.bin',
    UploadId=upload_id
)
```

## Presigned URLs

Generate temporary URLs for accessing objects without credentials.

### Presigned GET URL

**Example:**
```bash
aws --endpoint-url http://localhost:8080 s3 presign \
    s3://my-bucket/file.txt \
    --expires-in 3600
```

**Output:**
```
http://localhost:8080/my-bucket/file.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=...&X-Amz-Date=...&X-Amz-Expires=3600&X-Amz-SignedHeaders=host&X-Amz-Signature=...
```

**Use the URL:**
```bash
curl "http://localhost:8080/my-bucket/file.txt?..." -o downloaded.txt
```

### Presigned PUT URL

**Python example:**
```python
url = s3.generate_presigned_url(
    'put_object',
    Params={
        'Bucket': 'my-bucket',
        'Key': 'upload.txt'
    },
    ExpiresIn=3600
)

# Upload using presigned URL
import requests
with open('file.txt', 'rb') as f:
    requests.put(url, data=f)
```

## Error Responses

Anode returns standard S3 error responses:

### Common Errors

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| NoSuchBucket | 404 | Bucket doesn't exist |
| NoSuchKey | 404 | Object doesn't exist |
| BucketAlreadyExists | 409 | Bucket already exists |
| BucketNotEmpty | 409 | Cannot delete non-empty bucket |
| InvalidBucketName | 400 | Invalid bucket name |
| KeyTooLong | 400 | Object key too long (>1024 chars) |
| EntityTooLarge | 400 | Object too large |
| InvalidArgument | 400 | Invalid request parameter |
| AccessDenied | 403 | Authentication failed |
| SignatureDoesNotMatch | 403 | Invalid signature |
| RequestTimeTooSkewed | 403 | Request time differs from server time |
| InternalError | 500 | Server error |
| ServiceUnavailable | 503 | Cluster not ready |

### Error Response Format

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
  <Code>NoSuchKey</Code>
  <Message>The specified key does not exist.</Message>
  <Key>file.txt</Key>
  <RequestId>request123</RequestId>
</Error>
```

## Limitations

Current limitations compared to AWS S3:

1. **No Versioning**: Object versioning not yet supported
2. **No Server-Side Encryption**: Must use client-side encryption or filesystem encryption
3. **No Object Lifecycle**: No automatic expiration or transition
4. **No Replication**: No cross-region replication
5. **No Object Lock**: No WORM support
6. **No Access Logs**: No S3 access logging
7. **No Event Notifications**: No SNS/SQS notifications
8. **Limited ACLs**: Basic permissions only

## Best Practices

1. **Use Multipart Upload for Large Files**
   - Files > 100MB should use multipart upload
   - Better performance and reliability
   - Ability to retry failed parts

2. **Set Appropriate Content-Type**
   - Helps with browser rendering
   - Some tools rely on Content-Type

3. **Use Presigned URLs for Temporary Access**
   - Avoid sharing credentials
   - Time-limited access
   - Revocable (by changing credentials)

4. **Implement Retry Logic**
   - Network failures happen
   - Exponential backoff recommended
   - AWS SDKs handle this automatically

5. **Monitor Request Rates**
   - Respect cluster capacity
   - Implement client-side rate limiting if needed
   - Use metrics to track usage

6. **Use Proper Key Naming**
   - Include prefixes for organization
   - Avoid sequential keys (e.g., timestamps)
   - Use random prefixes for better distribution

## Testing Compatibility

Test Anode S3 compatibility with your application:

```bash
# Using AWS CLI
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_ENDPOINT_URL=http://localhost:8080

aws s3 mb s3://test-bucket
aws s3 cp test.txt s3://test-bucket/
aws s3 ls s3://test-bucket/
aws s3 rm s3://test-bucket/test.txt
aws s3 rb s3://test-bucket
```

```python
# Using boto3
import boto3
from botocore.client import Config

s3 = boto3.client(
    's3',
    endpoint_url='http://localhost:8080',
    aws_access_key_id='test',
    aws_secret_access_key='test',
    config=Config(signature_version='s3v4')
)

# Test operations
s3.create_bucket(Bucket='test-bucket')
s3.put_object(Bucket='test-bucket', Key='test.txt', Body=b'Hello')
s3.get_object(Bucket='test-bucket', Key='test.txt')
s3.delete_object(Bucket='test-bucket', Key='test.txt')
s3.delete_bucket(Bucket='test-bucket')
```

## Future API Support

Planned features:

- ✅ Object versioning
- ✅ Server-side encryption (SSE-C, SSE-KMS)
- ✅ Object lifecycle policies
- ✅ Cross-region replication
- ✅ Object locking (WORM)
- ✅ S3 Select (query in place)
- ✅ Bucket policies
- ✅ Access control lists (ACLs)
- ✅ Event notifications

See [ROADMAP.md](../ROADMAP.md) for details and timelines.
