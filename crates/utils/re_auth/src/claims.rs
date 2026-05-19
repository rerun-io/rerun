use crate::Permission;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RedapClaims {
    /// The issuer of the token.
    ///
    /// Could be an identity provider or the storage node directly.
    pub iss: String,

    /// The subject (user) of the token.
    pub sub: String,

    /// The `aud` claim, identifying the intended consumer of the token.
    ///
    /// Typically set to `"redap"` for Rerun storage-node tokens.
    /// Per RFC 7519, this can be either a single string or an array of strings.
    #[serde(
        deserialize_with = "deser_string_or_vec",
        serialize_with = "ser_string_or_vec"
    )]
    pub aud: Vec<String>,

    /// Expiry time of the token.
    pub exp: u64,

    /// Issued at time of the token.
    pub iat: u64,

    #[serde(default)]
    pub permissions: Vec<Permission>,

    /// Host patterns this token is allowed to be sent to.
    ///
    /// Uses the same domain-matching semantics as [`crate::host_matches_pattern`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Claims {
    #[cfg(feature = "oauth")]
    RerunCloud(crate::oauth::RerunCloudClaims),

    Redap(RedapClaims),
}

impl Claims {
    /// Subject. An email if available, otherwise it's usually the user ID.
    pub fn sub(&self) -> &str {
        match self {
            #[cfg(feature = "oauth")]
            Self::RerunCloud(claims) => claims.email.as_deref().unwrap_or(claims.sub.as_str()),
            Self::Redap(claims) => claims.sub.as_str(),
        }
    }

    /// Issuer
    pub fn iss(&self) -> &str {
        match self {
            #[cfg(feature = "oauth")]
            Self::RerunCloud(claims) => claims.iss.as_str(),
            Self::Redap(claims) => claims.iss.as_str(),
        }
    }

    pub fn permissions(&self) -> &[Permission] {
        match self {
            #[cfg(feature = "oauth")]
            Self::RerunCloud(claims) => &claims.permissions[..],
            Self::Redap(claims) => &claims.permissions[..],
        }
    }

    pub fn has_read_permission(&self) -> bool {
        self.permissions().iter().any(|p| p == &Permission::Read)
    }

    pub fn has_write_permission(&self) -> bool {
        self.permissions()
            .iter()
            .any(|p| p == &Permission::ReadWrite)
    }
}

/// Deserializes either a string of an array of strings into an array of strings.
fn deser_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        One(String),
        Many(Vec<String>),
    }

    use serde::Deserialize as _;
    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::One(s) => Ok(vec![s]),
        StringOrVec::Many(v) => Ok(v),
    }
}

/// Serializes an array of strings into either a single string if unary, or into an array of strings otherwise.
fn ser_string_or_vec<S>(value: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize as _;
    if value.len() == 1 {
        serializer.serialize_str(&value[0])
    } else {
        value.serialize(serializer)
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aud_deserialize_single_string() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": "redap",
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, vec!["redap"]);
        assert!(claims.allowed_hosts.is_empty());
    }

    #[test]
    fn test_aud_deserialize_array() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": ["redap", "other-service"],
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, vec!["redap", "other-service"]);
    }

    #[test]
    fn test_aud_deserialize_empty_array() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": [],
            "exp": 1234567890,
            "iat": 1234567890
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, Vec::<String>::new());
    }

    #[test]
    fn test_allowed_hosts_deserialize() {
        let json = r#"{
            "iss": "test",
            "sub": "user123",
            "aud": "redap",
            "exp": 1234567890,
            "iat": 1234567890,
            "allowed_hosts": ["api.acme.cloud.rerun.io"]
        }"#;

        let claims: RedapClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.aud, vec!["redap"]);
        assert_eq!(claims.allowed_hosts, vec!["api.acme.cloud.rerun.io"]);
    }

    #[test]
    fn test_aud_serialize_single() {
        let claims = RedapClaims {
            iss: "test".to_owned(),
            sub: "user123".to_owned(),
            aud: vec!["redap".to_owned()],
            exp: 1234567890,
            iat: 1234567890,
            permissions: vec![],
            allowed_hosts: vec![],
        };

        let json = serde_json::to_value(&claims).unwrap();
        // When there's exactly one aud value, it should serialize as a string
        assert_eq!(json["aud"], serde_json::json!("redap"));
        // Empty allowed_hosts should not appear in JSON
        assert!(json.get("allowed_hosts").is_none());
    }

    #[test]
    fn test_aud_serialize_multiple() {
        let claims = RedapClaims {
            iss: "test".to_owned(),
            sub: "user123".to_owned(),
            aud: vec!["redap".to_owned(), "other".to_owned()],
            exp: 1234567890,
            iat: 1234567890,
            permissions: vec![],
            allowed_hosts: vec![],
        };

        let json = serde_json::to_value(&claims).unwrap();
        // When there are multiple aud values, it should serialize as an array
        assert_eq!(json["aud"], serde_json::json!(["redap", "other"]));
    }

    #[test]
    fn test_allowed_hosts_serialize() {
        let claims = RedapClaims {
            iss: "test".to_owned(),
            sub: "user123".to_owned(),
            aud: vec!["redap".to_owned()],
            exp: 1234567890,
            iat: 1234567890,
            permissions: vec![],
            allowed_hosts: vec!["api.acme.cloud.rerun.io".to_owned()],
        };

        let json = serde_json::to_value(&claims).unwrap();
        assert_eq!(
            json["allowed_hosts"],
            serde_json::json!(["api.acme.cloud.rerun.io"])
        );
    }
}
