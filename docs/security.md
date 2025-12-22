# Security Guide

Security best practices and guidelines for Anode deployments.

## Security Model

Anode implements multiple layers of security:

1. **Authentication** - AWS Signature V4
2. **Authorization** - Bucket and object permissions
3. **Network Security** - TLS encryption, network isolation
4. **Data Security** - At-rest encryption (optional)
5. **Audit Logging** - Request logging and audit trails

## Authentication

### AWS Signature Version 4

Anode uses AWS Signature V4 for request authentication, compatible with AWS SDKs and tools.

**How it works:**
1. Client creates canonical request
2. Client creates string to sign
3. Client derives signing key
4. Client calculates signature
5. Server verifies signature

**Required headers:**
```
Authorization: AWS4-HMAC-SHA256 Credential=<access_key>/...
x-amz-date: 20231219T103000Z
x-amz-content-sha256: <SHA256 hash>
```

### Credential Management

#### Creating Credentials

```bash
# Generate access key and secret key
curl -X POST http://node1:8081/admin/credentials \
    -H "Content-Type: application/json" \
    -d '{
        "user": "admin",
        "permissions": ["read", "write"]
    }'
```

**Response:**
```json
{
    "access_key": "AKIAIOSFODNN7EXAMPLE",
    "secret_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
}
```

**Important:** Secret key is shown only once. Store it securely.

#### Rotating Credentials

```bash
# Create new credentials
NEW_CREDS=$(curl -X POST http://node1:8081/admin/credentials -d '{"user": "admin"}')

# Update applications to use new credentials
# ... deploy applications ...

# Revoke old credentials
curl -X DELETE http://node1:8081/admin/credentials/OLDACCESSKEY
```

**Best practices:**
- Rotate credentials every 90 days
- Use different credentials for each application
- Never commit credentials to version control
- Use environment variables or secret management tools

### Credential Storage

Credentials are stored securely:
- Access keys: Plaintext (used for lookup)
- Secret keys: Hashed with bcrypt (never stored plaintext)
- Metadata: Encrypted at rest (if encryption enabled)

**Storage location:**
```
/var/lib/anode/data/metadata/credentials.db
```

**Permissions:**
```bash
chmod 600 /var/lib/anode/data/metadata/credentials.db
chown anode:anode /var/lib/anode/data/metadata/credentials.db
```

## Authorization

### Bucket Permissions

Basic bucket-level permissions:

**Permissions:**
- `read` - Can list and read objects
- `write` - Can create and delete objects
- `read_acp` - Can read bucket permissions
- `write_acp` - Can modify bucket permissions

**Set bucket permissions:**
```bash
curl -X PUT http://node1:8081/admin/buckets/my-bucket/permissions \
    -H "Content-Type: application/json" \
    -d '{
        "user:alice": ["read"],
        "user:bob": ["read", "write"],
        "user:admin": ["read", "write", "read_acp", "write_acp"]
    }'
```

### Object-Level ACLs

**ACL values:**
- `private` - Only owner can access
- `public-read` - Anyone can read
- `public-read-write` - Anyone can read and write
- `authenticated-read` - Any authenticated user can read

**Set object ACL:**
```bash
aws --endpoint-url http://node1:8080 s3api put-object-acl \
    --bucket my-bucket \
    --key my-object \
    --acl public-read
```

### Bucket Policies (Future)

Planned support for IAM-like bucket policies:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Principal": {"AWS": "user:alice"},
            "Action": ["s3:GetObject"],
            "Resource": "arn:aws:s3:::my-bucket/*"
        },
        {
            "Effect": "Deny",
            "Principal": "*",
            "Action": ["s3:DeleteBucket"],
            "Resource": "arn:aws:s3:::my-bucket"
        }
    ]
}
```

## Network Security

### TLS/SSL Encryption

#### S3 API (HTTPS)

Use reverse proxy (nginx, HAProxy) for TLS termination:

**nginx configuration:**
```nginx
server {
    listen 443 ssl http2;
    server_name s3.example.com;

    ssl_certificate /etc/ssl/certs/anode.crt;
    ssl_certificate_key /etc/ssl/private/anode.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Increase timeout for large uploads
        proxy_connect_timeout 600s;
        proxy_send_timeout 600s;
        proxy_read_timeout 600s;
    }
}
```

**Let's Encrypt:**
```bash
# Install certbot
apt-get install certbot python3-certbot-nginx

# Get certificate
certbot --nginx -d s3.example.com

# Auto-renewal
certbot renew --dry-run
```

#### gRPC (mTLS)

Enable mutual TLS for inter-node communication:

**Configuration:**
```toml
[cluster]
enable_tls = true
tls_cert = "/etc/anode/certs/node.crt"
tls_key = "/etc/anode/certs/node.key"
tls_ca = "/etc/anode/certs/ca.crt"
```

**Generate certificates:**
```bash
# Create CA
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 3650 -key ca.key -out ca.crt \
    -subj "/CN=Anode CA"

# Create node certificate
openssl genrsa -out node1.key 2048
openssl req -new -key node1.key -out node1.csr \
    -subj "/CN=node1.cluster.local"
openssl x509 -req -in node1.csr -CA ca.crt -CAkey ca.key \
    -CAcreateserial -out node1.crt -days 365
```

### Network Isolation

**Best practices:**
- Place gRPC endpoints on private network
- Expose S3 API through load balancer
- Use firewall rules to restrict access

**Firewall rules (iptables):**
```bash
# Allow S3 API from anywhere
iptables -A INPUT -p tcp --dport 8080 -j ACCEPT

# Allow gRPC only from cluster nodes
iptables -A INPUT -p tcp --dport 9090 -s 10.0.1.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 9090 -j DROP

# Allow admin API only from management network
iptables -A INPUT -p tcp --dport 8081 -s 10.0.0.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 8081 -j DROP
```

**Kubernetes NetworkPolicy:**
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: anode-network-policy
spec:
  podSelector:
    matchLabels:
      app: anode
  policyTypes:
    - Ingress
    - Egress
  ingress:
    # Allow S3 API from anywhere
    - ports:
      - protocol: TCP
        port: 8080
    # Allow gRPC only from anode pods
    - from:
      - podSelector:
          matchLabels:
            app: anode
      ports:
      - protocol: TCP
        port: 9090
    # Allow admin API only from monitoring
    - from:
      - namespaceSelector:
          matchLabels:
            name: monitoring
      ports:
      - protocol: TCP
        port: 8081
  egress:
    # Allow all egress (for now)
    - {}
```

## Data Security

### At-Rest Encryption

#### Filesystem-Level Encryption

Use encrypted filesystem (LUKS, dm-crypt):

```bash
# Create encrypted volume
cryptsetup luksFormat /dev/sdb
cryptsetup luksOpen /dev/sdb anode-data

# Create filesystem
mkfs.ext4 /dev/mapper/anode-data

# Mount
mount /dev/mapper/anode-data /var/lib/anode/data

# Auto-mount with key file
echo "passphrase" > /root/anode-key
chmod 600 /root/anode-key
echo "anode-data /dev/sdb /root/anode-key luks" >> /etc/crypttab
echo "/dev/mapper/anode-data /var/lib/anode/data ext4 defaults 0 2" >> /etc/fstab
```

#### Chunk-Level Encryption (Future)

Planned support for transparent chunk encryption:

```toml
[storage]
encryption = true
encryption_key = "/etc/anode/encryption.key"  # Or KMS integration
encryption_algorithm = "AES-256-GCM"
```

### In-Transit Encryption

All data in transit should be encrypted:
- S3 API: HTTPS (via reverse proxy)
- gRPC: mTLS (native support)
- Metrics: HTTPS (via reverse proxy)

### Key Management

**Best practices:**
- Use hardware security module (HSM) for key storage
- Rotate encryption keys regularly
- Use key derivation for per-object keys
- Backup keys securely offline

**Future: KMS Integration**
```toml
[security]
kms_provider = "aws"  # or "vault", "azure", etc.
kms_key_id = "arn:aws:kms:..."
```

## Audit Logging

### Access Logs

Enable S3 access logging:

```toml
[logging]
access_log = "/var/log/anode/access.log"
access_log_format = "json"
```

**Log format:**
```json
{
    "timestamp": "2023-12-19T10:30:00Z",
    "remote_addr": "192.168.1.100",
    "user": "alice",
    "method": "GET",
    "bucket": "my-bucket",
    "key": "my-object",
    "status": 200,
    "bytes_sent": 1024,
    "user_agent": "aws-cli/2.0",
    "duration_ms": 45
}
```

### Admin Audit Log

All admin operations are logged:

```bash
tail -f /var/log/anode/audit.log
```

**Events logged:**
- Credential creation/revocation
- Permission changes
- Cluster membership changes
- Configuration changes

### Centralized Logging

Send logs to centralized logging system:

**Fluentd:**
```yaml
<source>
  @type tail
  path /var/log/anode/*.log
  pos_file /var/log/td-agent/anode.pos
  tag anode
  <parse>
    @type json
  </parse>
</source>

<match anode>
  @type elasticsearch
  host elasticsearch.local
  port 9200
  index_name anode
  type_name log
</match>
```

**Prometheus metrics for security:**
```promql
# Authentication failures
rate(anode_auth_failures_total[5m])

# Permission denials
rate(anode_permission_denied_total[5m])

# Unusual access patterns
rate(anode_requests_total{status=~"4.."}[5m])
```

## Security Hardening

### Operating System

**Minimal install:**
- Install only required packages
- Disable unnecessary services
- Keep system updated

**User isolation:**
```bash
# Create anode user
useradd -r -s /bin/false anode

# Run as non-root
chown -R anode:anode /var/lib/anode
chmod -R 750 /var/lib/anode
```

**Systemd hardening:**
```ini
[Unit]
Description=Anode Object Storage
After=network.target

[Service]
Type=simple
User=anode
Group=anode
ExecStart=/usr/local/bin/anode --config /etc/anode/config.toml

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/anode
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
AmbientCapabilities=CAP_NET_BIND_SERVICE
SecureBits=keep-caps

[Install]
WantedBy=multi-user.target
```

### Container Security

**Docker:**
```yaml
services:
  anode:
    image: anode:latest
    security_opt:
      - no-new-privileges:true
    read_only: true
    tmpfs:
      - /tmp
    volumes:
      - data:/data:rw
      - config:/config:ro
    user: "1000:1000"
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
```

**Kubernetes:**
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: anode
spec:
  securityContext:
    runAsNonRoot: true
    runAsUser: 1000
    fsGroup: 1000
    seccompProfile:
      type: RuntimeDefault
  containers:
  - name: anode
    image: anode:latest
    securityContext:
      allowPrivilegeEscalation: false
      readOnlyRootFilesystem: true
      capabilities:
        drop:
          - ALL
        add:
          - NET_BIND_SERVICE
    volumeMounts:
    - name: data
      mountPath: /data
    - name: config
      mountPath: /config
      readOnly: true
```

## Compliance

### GDPR Compliance

**Data deletion:**
```bash
# Delete user data
aws --endpoint-url http://node1:8080 s3 rb s3://user-bucket --force

# Verify deletion (run GC)
curl -X POST http://node1:8081/admin/gc

# Confirm deletion
curl http://node1:8081/admin/gc/status
```

**Data export:**
```bash
# Export user data
aws --endpoint-url http://node1:8080 s3 sync \
    s3://user-bucket /export/user-data/
```

### SOC 2 Compliance

**Required controls:**
- Access logging (enabled)
- Encryption in transit (TLS)
- Encryption at rest (filesystem encryption)
- Access controls (credentials + permissions)
- Audit trails (audit log)
- Incident response procedures (documented)

## Vulnerability Management

### Dependency Scanning

```bash
# Install cargo-audit
cargo install cargo-audit

# Audit dependencies
cargo audit

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

### Container Scanning

```bash
# Scan Docker image
trivy image anode:latest

# Scan Kubernetes manifests
trivy config deploy/helm/
```

### Security Updates

Subscribe to security announcements:
- GitHub Security Advisories
- Rust Security Advisories
- Anode security mailing list

**Update process:**
1. Review security advisory
2. Test update in staging
3. Rolling update to production
4. Verify no vulnerabilities

## Incident Response

### Security Incident Checklist

1. **Detection**
   - [ ] Monitor alerts for anomalies
   - [ ] Review audit logs
   - [ ] Check for unauthorized access

2. **Containment**
   - [ ] Revoke compromised credentials
   - [ ] Isolate affected nodes
   - [ ] Block malicious IPs

3. **Investigation**
   - [ ] Collect logs and evidence
   - [ ] Determine scope of breach
   - [ ] Identify root cause

4. **Recovery**
   - [ ] Restore from backup if needed
   - [ ] Patch vulnerabilities
   - [ ] Rotate all credentials

5. **Post-Incident**
   - [ ] Document incident
   - [ ] Update procedures
   - [ ] Notify affected parties

### Emergency Procedures

**Revoke all credentials:**
```bash
curl -X POST http://node1:8081/admin/credentials/revoke-all
```

**Enable read-only mode:**
```bash
curl -X POST http://node1:8081/admin/readonly
```

**Block IP address:**
```bash
iptables -A INPUT -s 192.168.1.100 -j DROP
```

## Security Best Practices

1. **Principle of Least Privilege**
   - Grant minimum required permissions
   - Use separate credentials for each application
   - Regularly review and revoke unused credentials

2. **Defense in Depth**
   - Multiple layers of security
   - Network isolation
   - TLS encryption
   - Access controls
   - Audit logging

3. **Regular Security Audits**
   - Review access logs monthly
   - Audit permissions quarterly
   - Penetration testing annually
   - Dependency scanning weekly

4. **Secure Defaults**
   - Authentication required (no anonymous access)
   - Encryption in transit recommended
   - Private buckets by default
   - Audit logging enabled

5. **Monitoring and Alerting**
   - Alert on authentication failures
   - Alert on permission denials
   - Alert on unusual access patterns
   - Monitor for vulnerabilities

## Security Checklist

**Authentication:**
- [ ] Strong credentials generated
- [ ] Credentials rotated regularly
- [ ] No hardcoded credentials
- [ ] Credentials stored securely

**Authorization:**
- [ ] Bucket permissions configured
- [ ] Least privilege principle followed
- [ ] Regular permission audits
- [ ] No public buckets (unless intended)

**Network:**
- [ ] TLS enabled for S3 API
- [ ] mTLS enabled for gRPC
- [ ] Firewall rules configured
- [ ] Network isolation implemented

**Data:**
- [ ] Encryption at rest configured
- [ ] Encryption in transit enabled
- [ ] Secure key management
- [ ] Regular backups

**Monitoring:**
- [ ] Audit logging enabled
- [ ] Centralized logging configured
- [ ] Security alerts configured
- [ ] Regular log review

**Compliance:**
- [ ] Data deletion procedures
- [ ] Data export capabilities
- [ ] Incident response plan
- [ ] Security documentation

**Hardening:**
- [ ] Minimal OS install
- [ ] Non-root user
- [ ] Systemd hardening
- [ ] Container security

**Maintenance:**
- [ ] Regular security updates
- [ ] Vulnerability scanning
- [ ] Dependency audits
- [ ] Penetration testing

## Resources

- [AWS Signature V4](https://docs.aws.amazon.com/general/latest/gr/signature-version-4.html)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CIS Security Benchmarks](https://www.cisecurity.org/cis-benchmarks/)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)
- [Anode Security Policy](https://github.com/christopherpaika/anode/security/policy)

## Reporting Security Issues

**Do not open public issues for security vulnerabilities.**

Email: security@anode.io
PGP Key: https://anode.io/security.asc

We will respond within 24 hours and provide a fix within 7 days for critical issues.
