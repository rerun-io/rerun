//! Minimal [`CryptoProvider`] for `jsonwebtoken` that supports HS256 and RS256.

use hmac::{Hmac, Mac as _};
use jsonwebtoken::crypto::{CryptoProvider, JwkUtils, JwtSigner, JwtVerifier};
use jsonwebtoken::errors::{Error, ErrorKind};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey};
use sha2::Sha256;
use signature::{Signer, Verifier};

type HmacSha256 = Hmac<Sha256>;

// --- HS256 ---

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

// --- RS256 ---

#[cfg(all(
    feature = "oauth",
    any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
))]
struct Rs256Verifier(DecodingKey);

#[cfg(all(
    feature = "oauth",
    any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
))]
impl Rs256Verifier {
    fn new(key: &DecodingKey) -> Self {
        Self(key.clone())
    }
}

#[cfg(all(
    feature = "oauth",
    any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
))]
impl Verifier<Vec<u8>> for Rs256Verifier {
    fn verify(&self, msg: &[u8], sig: &Vec<u8>) -> Result<(), signature::Error> {
        #[cfg(feature = "crypto-aws-lc-rs")]
        use aws_lc_rs::signature as rsa;
        use jsonwebtoken::DecodingKeyKind;
        #[cfg(not(feature = "crypto-aws-lc-rs"))]
        use ring::signature as rsa;

        match self.0.kind() {
            DecodingKeyKind::SecretOrDer(bytes) => {
                let public_key =
                    rsa::UnparsedPublicKey::new(&rsa::RSA_PKCS1_2048_8192_SHA256, bytes);
                public_key.verify(msg, sig)
            }
            DecodingKeyKind::RsaModulusExponent { n, e } => {
                let components = rsa::RsaPublicKeyComponents { n, e };
                components.verify(&rsa::RSA_PKCS1_2048_8192_SHA256, msg, sig)
            }
        }
        .map_err(|_err| signature::Error::from_source("RSA signature verification failed"))
    }
}

#[cfg(all(
    feature = "oauth",
    any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
))]
impl JwtVerifier for Rs256Verifier {
    fn algorithm(&self) -> Algorithm {
        Algorithm::RS256
    }
}

// ---

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
        #[cfg(all(
            feature = "oauth",
            any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
        ))]
        Algorithm::RS256 => Ok(Box::new(Rs256Verifier::new(key))),
        #[cfg(all(
            feature = "oauth",
            not(any(feature = "crypto-ring", feature = "crypto-aws-lc-rs"))
        ))]
        Algorithm::RS256 => Err(ErrorKind::InvalidAlgorithm.into()),
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

#[cfg(test)]
mod tests;
