use std::collections::HashMap;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use crate::Jwt;

const ISSUER_URL_BASE: &str = "https://api.workos.com/user_management";
const JWKS_URL_BASE: &str = "https://api.workos.com/sso/jwks";

// TODO: This is the client ID for the WorkOS public staging environment
//       should be replaced by our actual client ID at some point.
//       When doing so, don't forget to replace it everywhere. :)
const WORKOS_CLIENT_ID: &str = match option_env!("WORKOS_CLIENT_ID") {
    Some(v) => v,
    None => "client_01JZ3JVQW6JNVXME6HV9G4VR0H",
};

fn issuer(client_id: &str) -> String {
    format!("{ISSUER_URL_BASE}/{client_id}")
}

#[derive(Deserialize)]
pub struct Claims {
    iss: String,
    sub: String,
    act: Option<Act>,
    org_id: Option<String>,
    role: Option<String>,
    permissions: Option<Vec<String>>,
    entitlements: Option<Vec<String>>,
    sid: String,
    jti: String,
    exp: usize,
    iat: usize,
}

#[derive(Deserialize)]
pub struct Act {
    sub: String,
}

/// Json web key
#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: String,
    e: String,
    alg: String,
}

/// Json web key set
#[derive(Deserialize)]
pub struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum JwksDecodeError {
    #[error("failed to decode JWKS: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

impl JwksResponse {
    pub fn decode(self) -> Result<Jwks, JwksDecodeError> {
        let mut keys = HashMap::new();
        for j in self.keys {
            if j.kty != "RSA" {
                continue;
            }

            let key = DecodingKey::from_rsa_components(&j.n, &j.e)?;
            keys.insert(j.kid.clone(), key);
        }

        Ok(Jwks { keys })
    }
}

/// Json web key set.
///
/// To produce this:
/// 1. Fetch [`jwks_url`]
/// 2. Decode JSON response into [`JwksResponse`]
/// 3. Call [`JwksResponse::decode`]
pub struct Jwks {
    keys: HashMap<String, DecodingKey>,
}

impl Jwks {
    pub fn get(&self, kid: &str) -> Option<&DecodingKey> {
        self.keys.get(kid)
    }
}

pub fn jwks_url() -> String {
    format!("{JWKS_URL_BASE}/{WORKOS_CLIENT_ID}")
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum VerifyError {
    #[error("invalid jwt: {0}")]
    InvalidJwt(#[from] jsonwebtoken::errors::Error),

    #[error("missing `kid` in JWT")]
    MissingKeyId,

    #[error("key with id {id:?} was not found in JWKS")]
    KeyNotFound { id: String },
}

pub enum Status {
    Valid,
    NeedsRefresh,
}

pub fn verify_token(jwt: Jwt, jwks: &Jwks) -> Result<Status, VerifyError> {
    // 1. Decode header to get `kid`
    let header = decode_header(jwt.as_str())?;
    let kid = header
        .kid
        .as_ref()
        .ok_or_else(|| VerifyError::MissingKeyId)?;
    let key = jwks
        .get(kid)
        .ok_or_else(|| VerifyError::KeyNotFound { id: kid.clone() })?;

    // 2. Verify token
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer(WORKOS_CLIENT_ID)]);
    validation.validate_exp = true;
    let _token_data = match decode::<Claims>(&jwt.as_str(), key, &validation) {
        Ok(v) => v,
        Err(err) => match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => return Ok(Status::NeedsRefresh),
            _ => return Err(err.into()),
        },
    };

    Ok(Status::Valid)
}

// TODO: refresh
// https://workos.com/docs/reference/user-management/session-tokens/refresh-token
