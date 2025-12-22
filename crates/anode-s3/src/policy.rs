//! S3 bucket policy support.
//!
//! This module provides data structures and validation for S3 bucket policies.
//! Policies follow the AWS S3 bucket policy JSON format with Version, Id, and Statement.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// S3 bucket policy document.
///
/// Represents a complete bucket policy in AWS S3 format.
/// See: https://docs.aws.amazon.com/AmazonS3/latest/userguide/bucket-policies.html
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BucketPolicy {
    /// Policy language version (typically "2012-10-17")
    #[serde(rename = "Version")]
    pub version: String,

    /// Optional policy identifier
    #[serde(rename = "Id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// List of policy statements
    #[serde(rename = "Statement")]
    pub statement: Vec<PolicyStatement>,
}

/// A single statement within a bucket policy.
///
/// Each statement grants or denies specific permissions to principals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyStatement {
    /// Optional statement identifier
    #[serde(rename = "Sid", skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,

    /// Effect: "Allow" or "Deny"
    #[serde(rename = "Effect")]
    pub effect: Effect,

    /// Principal (user, account, service, etc.)
    #[serde(rename = "Principal")]
    pub principal: Principal,

    /// Action or list of actions
    #[serde(rename = "Action")]
    pub action: ActionList,

    /// Resource or list of resources
    #[serde(rename = "Resource")]
    pub resource: ResourceList,

    /// Optional conditions for when the policy applies
    #[serde(rename = "Condition", skip_serializing_if = "Option::is_none")]
    pub condition: Option<HashMap<String, HashMap<String, serde_json::Value>>>,
}

/// Effect of a policy statement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Effect {
    Allow,
    Deny,
}

/// Principal in a policy statement.
///
/// Can be a wildcard, specific AWS account, service, or user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Principal {
    /// Wildcard principal (everyone)
    Wildcard(String),
    /// Structured principal with AWS accounts, services, etc.
    Structured(HashMap<String, PrincipalValue>),
}

/// Value for a principal (can be string or array of strings).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PrincipalValue {
    Single(String),
    Multiple(Vec<String>),
}

/// Action or list of actions in a policy statement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ActionList {
    Single(String),
    Multiple(Vec<String>),
}

/// Resource or list of resources in a policy statement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ResourceList {
    Single(String),
    Multiple(Vec<String>),
}

impl BucketPolicy {
    /// Validates the policy structure.
    ///
    /// Checks that:
    /// - Version is present and valid
    /// - Statement array is not empty
    /// - Each statement has required fields
    ///
    /// Returns an error message if validation fails, or Ok(()) if valid.
    pub fn validate(&self) -> Result<(), String> {
        // Validate version
        if self.version.is_empty() {
            return Err("Policy version is required".to_string());
        }

        // AWS typically uses "2012-10-17" but we'll accept any non-empty version
        if self.version != "2012-10-17" && self.version != "2008-10-17" {
            return Err(format!(
                "Unsupported policy version: {}. Expected '2012-10-17' or '2008-10-17'",
                self.version
            ));
        }

        // Validate statements
        if self.statement.is_empty() {
            return Err("Policy must contain at least one statement".to_string());
        }

        // Validate each statement
        for (idx, stmt) in self.statement.iter().enumerate() {
            stmt.validate()
                .map_err(|e| format!("Statement {}: {}", idx, e))?;
        }

        Ok(())
    }

    /// Parses a bucket policy from JSON string.
    ///
    /// Returns the parsed policy or an error message.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))
    }

    /// Serializes the policy to a JSON string.
    ///
    /// Returns the JSON representation or an error message.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("Serialization error: {}", e))
    }
}

impl PolicyStatement {
    /// Validates a single policy statement.
    fn validate(&self) -> Result<(), String> {
        // Validate effect (already constrained by enum)

        // Validate actions
        match &self.action {
            ActionList::Single(action) => {
                if action.is_empty() {
                    return Err("Action cannot be empty".to_string());
                }
            },
            ActionList::Multiple(actions) => {
                if actions.is_empty() {
                    return Err("Action list cannot be empty".to_string());
                }
                for action in actions {
                    if action.is_empty() {
                        return Err("Action in list cannot be empty".to_string());
                    }
                }
            },
        }

        // Validate resources
        match &self.resource {
            ResourceList::Single(resource) => {
                if resource.is_empty() {
                    return Err("Resource cannot be empty".to_string());
                }
            },
            ResourceList::Multiple(resources) => {
                if resources.is_empty() {
                    return Err("Resource list cannot be empty".to_string());
                }
                for resource in resources {
                    if resource.is_empty() {
                        return Err("Resource in list cannot be empty".to_string());
                    }
                }
            },
        }

        Ok(())
    }

    /// Checks if this statement matches the given request context.
    ///
    /// Returns true if the statement applies to the request.
    fn matches(&self, ctx: &EvaluationContext) -> bool {
        self.matches_principal(&ctx.principal)
            && self.matches_action(&ctx.action)
            && self.matches_resource(&ctx.resource)
    }

    /// Checks if the principal matches.
    fn matches_principal(&self, principal: &str) -> bool {
        match &self.principal {
            Principal::Wildcard(w) => w == "*",
            Principal::Structured(map) => {
                // Check if any principal type matches
                for values in map.values() {
                    match values {
                        PrincipalValue::Single(p) => {
                            if matches_pattern(p, principal) {
                                return true;
                            }
                        },
                        PrincipalValue::Multiple(list) => {
                            for p in list {
                                if matches_pattern(p, principal) {
                                    return true;
                                }
                            }
                        },
                    }
                }
                false
            },
        }
    }

    /// Checks if the action matches.
    fn matches_action(&self, action: &str) -> bool {
        match &self.action {
            ActionList::Single(a) => matches_pattern(a, action),
            ActionList::Multiple(list) => list.iter().any(|a| matches_pattern(a, action)),
        }
    }

    /// Checks if the resource matches.
    fn matches_resource(&self, resource: &str) -> bool {
        match &self.resource {
            ResourceList::Single(r) => matches_pattern(r, resource),
            ResourceList::Multiple(list) => list.iter().any(|r| matches_pattern(r, resource)),
        }
    }
}

/// Context for policy evaluation.
///
/// Contains information about the request being evaluated.
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// The principal making the request (user, account, etc.)
    pub principal: String,
    /// The action being performed (e.g., "s3:GetObject")
    pub action: String,
    /// The resource being accessed (e.g., "arn:aws:s3:::bucket/key")
    pub resource: String,
}

/// Result of policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Request is explicitly allowed
    Allow,
    /// Request is explicitly denied
    Deny,
    /// No policy statement matched (defaults to deny)
    DefaultDeny,
}

impl BucketPolicy {
    /// Evaluates the policy against a request context.
    ///
    /// Returns a PolicyDecision indicating whether the request should be allowed or denied.
    ///
    /// Policy evaluation follows AWS S3 semantics:
    /// 1. If any Deny statement matches, deny the request
    /// 2. If any Allow statement matches, allow the request
    /// 3. Otherwise, deny by default
    pub fn evaluate(&self, ctx: &EvaluationContext) -> PolicyDecision {
        // First check for explicit denies
        for statement in &self.statement {
            if statement.effect == Effect::Deny && statement.matches(ctx) {
                return PolicyDecision::Deny;
            }
        }

        // Then check for explicit allows
        for statement in &self.statement {
            if statement.effect == Effect::Allow && statement.matches(ctx) {
                return PolicyDecision::Allow;
            }
        }

        // Default deny if no statements matched
        PolicyDecision::DefaultDeny
    }
}

/// Matches a pattern against a value, supporting wildcards.
///
/// Supports:
/// - Exact match: "s3:GetObject" matches "s3:GetObject"
/// - Wildcard: "s3:*" matches "s3:GetObject", "s3:PutObject", etc.
/// - Prefix wildcard: "arn:aws:s3:::bucket/*" matches "arn:aws:s3:::bucket/key"
fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern == value {
        return true;
    }

    // Handle wildcard patterns
    if pattern.contains('*') {
        // Simple wildcard matching: split on * and check parts
        let parts: Vec<&str> = pattern.split('*').collect();

        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];

            if suffix.is_empty() {
                // Pattern like "s3:Get*"
                return value.starts_with(prefix);
            } else if prefix.is_empty() {
                // Pattern like "*Object"
                return value.ends_with(suffix);
            } else {
                // Pattern like "s3:*Object"
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        } else if parts.len() == 1 {
            // No wildcard, exact match
            return pattern == value;
        }
        // Multiple wildcards - for now, just check prefix
        return value.starts_with(parts[0]);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_policy() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        assert_eq!(policy.version, "2012-10-17");
        assert_eq!(policy.statement.len(), 1);
        assert_eq!(policy.statement[0].effect, Effect::Allow);
    }

    #[test]
    fn test_parse_policy_with_multiple_actions() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {"AWS": "arn:aws:iam::123456789012:root"},
                    "Action": ["s3:GetObject", "s3:PutObject"],
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        assert_eq!(policy.statement.len(), 1);
        match &policy.statement[0].action {
            ActionList::Multiple(actions) => {
                assert_eq!(actions.len(), 2);
                assert_eq!(actions[0], "s3:GetObject");
                assert_eq!(actions[1], "s3:PutObject");
            },
            _ => panic!("Expected multiple actions"),
        }
    }

    #[test]
    fn test_parse_policy_with_conditions() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::mybucket/*",
                    "Condition": {
                        "IpAddress": {
                            "aws:SourceIp": "192.168.1.0/24"
                        }
                    }
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        assert!(policy.statement[0].condition.is_some());
    }

    #[test]
    fn test_validate_valid_policy() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_version() {
        let policy = BucketPolicy {
            version: "".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_validate_empty_statements() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![],
        };

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_version() {
        let policy = BucketPolicy {
            version: "2020-01-01".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_roundtrip_serialization() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: Some("MyPolicy".to_string()),
            statement: vec![PolicyStatement {
                sid: Some("PublicRead".to_string()),
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Multiple(vec![
                    "s3:GetObject".to_string(),
                    "s3:GetObjectVersion".to_string(),
                ]),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let json = policy.to_json().unwrap();
        let parsed = BucketPolicy::from_json(&json).unwrap();
        assert_eq!(policy, parsed);
    }

    #[test]
    fn test_deny_effect() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Deny",
                    "Principal": "*",
                    "Action": "s3:DeleteObject",
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        assert_eq!(policy.statement[0].effect, Effect::Deny);
    }

    #[test]
    fn test_structured_principal() {
        let json = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "AWS": ["arn:aws:iam::123456789012:root", "arn:aws:iam::123456789012:user/alice"]
                    },
                    "Action": "s3:*",
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        match &policy.statement[0].principal {
            Principal::Structured(map) => {
                assert!(map.contains_key("AWS"));
            },
            _ => panic!("Expected structured principal"),
        }
    }

    #[test]
    fn test_policy_with_id() {
        let json = r#"{
            "Version": "2012-10-17",
            "Id": "MyBucketPolicy",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": "*",
                    "Action": "s3:GetObject",
                    "Resource": "arn:aws:s3:::mybucket/*"
                }
            ]
        }"#;

        let policy = BucketPolicy::from_json(json).unwrap();
        assert_eq!(policy.id, Some("MyBucketPolicy".to_string()));
    }

    #[test]
    fn test_validate_empty_action() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_validate_empty_resource() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("".to_string()),
                condition: None,
            }],
        };

        assert!(policy.validate().is_err());
    }

    // Policy enforcement tests
    #[test]
    fn test_policy_allows_action() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx), PolicyDecision::Allow);
    }

    #[test]
    fn test_policy_denies_action() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Deny,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:DeleteObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:DeleteObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx), PolicyDecision::Deny);
    }

    #[test]
    fn test_policy_default_deny_when_no_match() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_policy_deny_overrides_allow() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![
                PolicyStatement {
                    sid: Some("AllowAll".to_string()),
                    effect: Effect::Allow,
                    principal: Principal::Wildcard("*".to_string()),
                    action: ActionList::Single("s3:*".to_string()),
                    resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                    condition: None,
                },
                PolicyStatement {
                    sid: Some("DenyDelete".to_string()),
                    effect: Effect::Deny,
                    principal: Principal::Wildcard("*".to_string()),
                    action: ActionList::Single("s3:DeleteObject".to_string()),
                    resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                    condition: None,
                },
            ],
        };

        let ctx = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:DeleteObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        // Deny should take precedence
        assert_eq!(policy.evaluate(&ctx), PolicyDecision::Deny);
    }

    #[test]
    fn test_wildcard_action_matching() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:*".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx1 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx2 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx3 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:DeleteObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx1), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx2), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx3), PolicyDecision::Allow);
    }

    #[test]
    fn test_wildcard_principal_matching() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx1 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx2 = EvaluationContext {
            principal: "arn:aws:iam::999999999999:user/bob".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx1), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx2), PolicyDecision::Allow);
    }

    #[test]
    fn test_structured_principal_matching() {
        let mut principal_map = HashMap::new();
        principal_map.insert(
            "AWS".to_string(),
            PrincipalValue::Single("arn:aws:iam::123456789012:user/alice".to_string()),
        );

        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Structured(principal_map),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx_match = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_no_match = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/bob".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_match), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_no_match), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_multiple_principals() {
        let mut principal_map = HashMap::new();
        principal_map.insert(
            "AWS".to_string(),
            PrincipalValue::Multiple(vec![
                "arn:aws:iam::123456789012:user/alice".to_string(),
                "arn:aws:iam::123456789012:user/bob".to_string(),
            ]),
        );

        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Structured(principal_map),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx_alice = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_bob = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/bob".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_charlie = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/charlie".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_alice), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_bob), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_charlie), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_multiple_actions() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Multiple(vec![
                    "s3:GetObject".to_string(),
                    "s3:GetObjectVersion".to_string(),
                ]),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx_get = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_version = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObjectVersion".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_put = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_get), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_version), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_put), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_multiple_resources() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Multiple(vec![
                    "arn:aws:s3:::mybucket/public/*".to_string(),
                    "arn:aws:s3:::mybucket/shared/*".to_string(),
                ]),
                condition: None,
            }],
        };

        let ctx_public = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/public/file.txt".to_string(),
        };

        let ctx_shared = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/shared/doc.pdf".to_string(),
        };

        let ctx_private = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/private/secret.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_public), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_shared), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_private), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_action_prefix_wildcard() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:Get*".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx_get_object = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_get_version = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObjectVersion".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        let ctx_put = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_get_object), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_get_version), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_put), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_resource_wildcard_matching() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: None,
            statement: vec![PolicyStatement {
                sid: None,
                effect: Effect::Allow,
                principal: Principal::Wildcard("*".to_string()),
                action: ActionList::Single("s3:GetObject".to_string()),
                resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                condition: None,
            }],
        };

        let ctx_match = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/path/to/file.txt".to_string(),
        };

        let ctx_no_match = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/alice".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::otherbucket/file.txt".to_string(),
        };

        assert_eq!(policy.evaluate(&ctx_match), PolicyDecision::Allow);
        assert_eq!(policy.evaluate(&ctx_no_match), PolicyDecision::DefaultDeny);
    }

    #[test]
    fn test_pattern_matching_exact() {
        assert!(matches_pattern("s3:GetObject", "s3:GetObject"));
        assert!(!matches_pattern("s3:GetObject", "s3:PutObject"));
    }

    #[test]
    fn test_pattern_matching_wildcard_only() {
        assert!(matches_pattern("*", "s3:GetObject"));
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("*", ""));
    }

    #[test]
    fn test_pattern_matching_prefix_wildcard() {
        assert!(matches_pattern("s3:Get*", "s3:GetObject"));
        assert!(matches_pattern("s3:Get*", "s3:GetObjectVersion"));
        assert!(!matches_pattern("s3:Get*", "s3:PutObject"));
        assert!(!matches_pattern("s3:Get*", "ec2:GetInstance"));
    }

    #[test]
    fn test_pattern_matching_suffix_wildcard() {
        assert!(matches_pattern("*Object", "s3:GetObject"));
        assert!(matches_pattern("*Object", "s3:PutObject"));
        assert!(!matches_pattern("*Object", "s3:ListBucket"));
    }

    #[test]
    fn test_pattern_matching_middle_wildcard() {
        assert!(matches_pattern("s3:*Object", "s3:GetObject"));
        assert!(matches_pattern("s3:*Object", "s3:PutObject"));
        assert!(matches_pattern("s3:*Object", "s3:DeleteObject"));
        assert!(!matches_pattern("s3:*Object", "s3:ListBucket"));
        assert!(!matches_pattern("s3:*Object", "ec2:GetObject"));
    }

    #[test]
    fn test_complex_policy_scenario() {
        let policy = BucketPolicy {
            version: "2012-10-17".to_string(),
            id: Some("ComplexPolicy".to_string()),
            statement: vec![
                // Allow read access to public directory
                PolicyStatement {
                    sid: Some("PublicRead".to_string()),
                    effect: Effect::Allow,
                    principal: Principal::Wildcard("*".to_string()),
                    action: ActionList::Single("s3:GetObject".to_string()),
                    resource: ResourceList::Single("arn:aws:s3:::mybucket/public/*".to_string()),
                    condition: None,
                },
                // Allow specific user to write
                {
                    let mut principal_map = HashMap::new();
                    principal_map.insert(
                        "AWS".to_string(),
                        PrincipalValue::Single("arn:aws:iam::123456789012:user/admin".to_string()),
                    );
                    PolicyStatement {
                        sid: Some("AdminWrite".to_string()),
                        effect: Effect::Allow,
                        principal: Principal::Structured(principal_map),
                        action: ActionList::Multiple(vec![
                            "s3:PutObject".to_string(),
                            "s3:DeleteObject".to_string(),
                        ]),
                        resource: ResourceList::Single("arn:aws:s3:::mybucket/*".to_string()),
                        condition: None,
                    }
                },
                // Deny delete on critical files
                PolicyStatement {
                    sid: Some("ProtectCritical".to_string()),
                    effect: Effect::Deny,
                    principal: Principal::Wildcard("*".to_string()),
                    action: ActionList::Single("s3:DeleteObject".to_string()),
                    resource: ResourceList::Single("arn:aws:s3:::mybucket/critical/*".to_string()),
                    condition: None,
                },
            ],
        };

        // Public can read from /public
        let ctx1 = EvaluationContext {
            principal: "arn:aws:iam::999999999999:user/anyone".to_string(),
            action: "s3:GetObject".to_string(),
            resource: "arn:aws:s3:::mybucket/public/image.png".to_string(),
        };
        assert_eq!(policy.evaluate(&ctx1), PolicyDecision::Allow);

        // Admin can write
        let ctx2 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/admin".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/data/file.txt".to_string(),
        };
        assert_eq!(policy.evaluate(&ctx2), PolicyDecision::Allow);

        // Even admin cannot delete critical files (deny overrides allow)
        let ctx3 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/admin".to_string(),
            action: "s3:DeleteObject".to_string(),
            resource: "arn:aws:s3:::mybucket/critical/config.json".to_string(),
        };
        assert_eq!(policy.evaluate(&ctx3), PolicyDecision::Deny);

        // Admin can delete non-critical files
        let ctx4 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/admin".to_string(),
            action: "s3:DeleteObject".to_string(),
            resource: "arn:aws:s3:::mybucket/data/old.txt".to_string(),
        };
        assert_eq!(policy.evaluate(&ctx4), PolicyDecision::Allow);

        // Regular user cannot write
        let ctx5 = EvaluationContext {
            principal: "arn:aws:iam::123456789012:user/bob".to_string(),
            action: "s3:PutObject".to_string(),
            resource: "arn:aws:s3:::mybucket/data/file.txt".to_string(),
        };
        assert_eq!(policy.evaluate(&ctx5), PolicyDecision::DefaultDeny);
    }
}
