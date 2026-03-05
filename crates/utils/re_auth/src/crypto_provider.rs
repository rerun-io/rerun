//! Minimal [`CryptoProvider`] for `jsonwebtoken` that only supports HS256.

use hmac::{Hmac, Mac as _};
use jsonwebtoken::crypto::{CryptoProvider, JwkUtils, JwtSigner, JwtVerifier};
use jsonwebtoken::errors::{Error, ErrorKind};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use sha2::Sha256;
use signature::{Signer, Verifier};

type HmacSha256 = Hmac<Sha256>;

struct Hs256Signer(HmacSha256);

impl Hs256Signer {
    fn new(key: &EncodingKey) -> Result<Self, Error> {
        let inner = HmacSha256::new_from_slice(key.try_get_hmac_secret()?)
            .map_err(|_ignored| ErrorKind::InvalidKeyFormat)?;
        Ok(Self(inner))
    }
}

impl Signer<Vec<u8>> for Hs256Signer {
    fn try_sign(&self, msg: &[u8]) -> Result<Vec<u8>, signature::Error> {
        let mut mac = self.0.clone();
        mac.update(msg);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

impl JwtSigner for Hs256Signer {
    fn algorithm(&self) -> Algorithm {
        Algorithm::HS256
    }
}

struct Hs256Verifier(HmacSha256);

impl Hs256Verifier {
    fn new(key: &DecodingKey) -> Result<Self, Error> {
        let inner = HmacSha256::new_from_slice(key.try_get_hmac_secret()?)
            .map_err(|_ignored| ErrorKind::InvalidKeyFormat)?;
        Ok(Self(inner))
    }
}

impl Verifier<Vec<u8>> for Hs256Verifier {
    fn verify(&self, msg: &[u8], signature: &Vec<u8>) -> Result<(), signature::Error> {
        let mut mac = self.0.clone();
        mac.update(msg);
        mac.verify_slice(signature)
            .map_err(signature::Error::from_source)
    }
}

impl JwtVerifier for Hs256Verifier {
    fn algorithm(&self) -> Algorithm {
        Algorithm::HS256
    }
}

fn unsupported_algorithm(algo: &Algorithm) -> Error {
    re_log::debug_panic!("DEBUG PANIC: unsupported algorithm: {algo:?}");

    ErrorKind::InvalidAlgorithm.into()
}

fn signer_factory(algorithm: &Algorithm, key: &EncodingKey) -> Result<Box<dyn JwtSigner>, Error> {
    match algorithm {
        Algorithm::HS256 => Ok(Box::new(Hs256Signer::new(key)?)),
        other => Err(unsupported_algorithm(other)),
    }
}

fn verifier_factory(
    algorithm: &Algorithm,
    key: &DecodingKey,
) -> Result<Box<dyn JwtVerifier>, Error> {
    match algorithm {
        Algorithm::HS256 => Ok(Box::new(Hs256Verifier::new(key)?)),
        other => Err(unsupported_algorithm(other)),
    }
}

pub static PROVIDER: CryptoProvider = CryptoProvider {
    signer_factory,
    verifier_factory,
    jwk_utils: JwkUtils::new_unimplemented(),
};

/// Install our minimal [`CryptoProvider`]. Safe to call multiple times.
pub fn install() {
    PROVIDER.install_default().ok();
}
