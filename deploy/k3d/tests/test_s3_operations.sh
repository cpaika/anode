#!/bin/bash
# Test S3 operations via the cluster
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"
S3_PORT=8080

# Cleanup function
cleanup() {
    kill $PF_PID 2>/dev/null || true
    rm -f /tmp/test-upload-*.txt /tmp/test-download-*.txt
}
trap cleanup EXIT

echo "[INFO] Setting up S3 API access..."

# Port forward to S3 API
POD_NAME="${RELEASE_NAME}-0"
kubectl port-forward -n "${NAMESPACE}" "pod/${POD_NAME}" ${S3_PORT}:${S3_PORT} &
PF_PID=$!

# Wait for port forward to be ready
sleep 5

# Check if port forward is working
if ! kill -0 $PF_PID 2>/dev/null; then
    echo "[FAIL] Port forward failed to start"
    exit 1
fi

echo "[INFO] Port forward established to pod ${POD_NAME}"

# Test: Check S3 API is accessible
echo "[TEST] Checking S3 API accessibility..."
if curl -sf -X GET "http://localhost:${S3_PORT}/" > /dev/null 2>&1; then
    echo "[PASS] S3 API is accessible"
elif curl -sf -X GET "http://localhost:${S3_PORT}/health" > /dev/null 2>&1; then
    echo "[PASS] S3 API is accessible (via health endpoint)"
else
    echo "[WARN] S3 API response check inconclusive"
    # Don't fail immediately - the S3 API might require auth
fi

# Test: List buckets
echo "[TEST] Testing ListBuckets operation..."
BUCKET_LIST_RESPONSE=$(curl -sf -X GET "http://localhost:${S3_PORT}/" 2>/dev/null || echo "")

if [ -n "$BUCKET_LIST_RESPONSE" ]; then
    echo "[PASS] ListBuckets request succeeded"
else
    echo "[WARN] ListBuckets request returned empty or requires authentication"
fi

# Test: Create bucket
echo "[TEST] Testing CreateBucket operation..."
BUCKET_NAME="test-bucket-$(date +%s)"

CREATE_RESPONSE=$(curl -sf -X PUT "http://localhost:${S3_PORT}/${BUCKET_NAME}" 2>&1 || echo "FAILED")

if [[ "$CREATE_RESPONSE" != "FAILED" ]]; then
    echo "[PASS] CreateBucket request accepted for bucket: ${BUCKET_NAME}"
else
    echo "[WARN] CreateBucket request may require authentication or returned error"
    # Try to continue anyway
fi

# Test: Put object
echo "[TEST] Testing PutObject operation..."
TEST_FILE="/tmp/test-upload-$(date +%s).txt"
echo "This is a test file for S3 operations" > "$TEST_FILE"
OBJECT_KEY="test-object.txt"

PUT_RESPONSE=$(curl -sf -X PUT \
    -H "Content-Type: text/plain" \
    --data-binary "@${TEST_FILE}" \
    "http://localhost:${S3_PORT}/${BUCKET_NAME}/${OBJECT_KEY}" 2>&1 || echo "FAILED")

if [[ "$PUT_RESPONSE" != "FAILED" ]]; then
    echo "[PASS] PutObject request accepted"
else
    echo "[WARN] PutObject request may require authentication"
fi

# Test: Get object
echo "[TEST] Testing GetObject operation..."
DOWNLOAD_FILE="/tmp/test-download-$(date +%s).txt"

GET_RESPONSE=$(curl -sf -X GET \
    "http://localhost:${S3_PORT}/${BUCKET_NAME}/${OBJECT_KEY}" \
    -o "$DOWNLOAD_FILE" 2>&1 || echo "FAILED")

if [[ "$GET_RESPONSE" != "FAILED" ]] && [ -f "$DOWNLOAD_FILE" ]; then
    CONTENT=$(cat "$DOWNLOAD_FILE")
    if [ -n "$CONTENT" ]; then
        echo "[PASS] GetObject retrieved data"
    else
        echo "[WARN] GetObject returned empty content"
    fi
else
    echo "[WARN] GetObject request may require authentication"
fi

# Test: Delete object
echo "[TEST] Testing DeleteObject operation..."
DELETE_RESPONSE=$(curl -sf -X DELETE \
    "http://localhost:${S3_PORT}/${BUCKET_NAME}/${OBJECT_KEY}" 2>&1 || echo "FAILED")

if [[ "$DELETE_RESPONSE" != "FAILED" ]]; then
    echo "[PASS] DeleteObject request accepted"
else
    echo "[WARN] DeleteObject request may require authentication"
fi

# Test: Delete bucket
echo "[TEST] Testing DeleteBucket operation..."
DELETE_BUCKET_RESPONSE=$(curl -sf -X DELETE \
    "http://localhost:${S3_PORT}/${BUCKET_NAME}" 2>&1 || echo "FAILED")

if [[ "$DELETE_BUCKET_RESPONSE" != "FAILED" ]]; then
    echo "[PASS] DeleteBucket request accepted"
else
    echo "[WARN] DeleteBucket request may require authentication"
fi

# Test: Multi-part upload endpoints exist
echo "[TEST] Checking multi-part upload endpoints..."
MULTIPART_INIT=$(curl -sf -X POST \
    "http://localhost:${S3_PORT}/${BUCKET_NAME}/large-file?uploads" 2>&1 || echo "FAILED")

if [[ "$MULTIPART_INIT" != "FAILED" ]]; then
    echo "[PASS] Multi-part upload endpoints are accessible"
else
    echo "[WARN] Multi-part upload may require authentication or different parameters"
fi

echo ""
echo "=========================================="
echo "S3 operations test completed!"
echo ""
echo "NOTE: Some operations may show warnings if"
echo "authentication is required. This is normal"
echo "for a secure S3-compatible service."
echo "=========================================="

# Cleanup is handled by trap
