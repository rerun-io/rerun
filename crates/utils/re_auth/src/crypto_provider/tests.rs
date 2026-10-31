use super::*;

#[cfg(all(
    feature = "oauth",
    not(any(feature = "crypto-ring", feature = "crypto-aws-lc-rs"))
))]
#[test]
fn rsa_requires_a_selected_backend() {
    let result = verifier_factory(&Algorithm::RS256, &DecodingKey::from_rsa_der(b"unused"));
    assert!(matches!(
        result.err().unwrap().kind(),
        ErrorKind::InvalidAlgorithm
    ));
}

#[test]
fn hmac_verifies_only_the_original_message_and_key() {
    let signer =
        signer_factory(&Algorithm::HS256, &EncodingKey::from_secret(b"test secret")).unwrap();
    let verifier =
        verifier_factory(&Algorithm::HS256, &DecodingKey::from_secret(b"test secret")).unwrap();
    let signature = signer.try_sign(b"original message").unwrap();

    assert!(verifier.verify(b"original message", &signature).is_ok());
    assert!(verifier.verify(b"different message", &signature).is_err());
    let other_key = verifier_factory(
        &Algorithm::HS256,
        &DecodingKey::from_secret(b"other secret"),
    )
    .unwrap();
    assert!(other_key.verify(b"original message", &signature).is_err());
}

#[cfg(all(
    feature = "oauth",
    any(feature = "crypto-ring", feature = "crypto-aws-lc-rs")
))]
mod rsa {
    use base64::Engine as _;
    use base64::prelude::BASE64_STANDARD;

    use super::*;

    // A fixed RSA-2048/SHA-256/PKCS#1 v1.5 vector generated independently with .NET RSA.
    // Retain only public material so this fixture can also test alternative backends.
    const MESSAGE: &[u8] = b"rerun crypto provider regression";
    const DER: &str = "MIIBCgKCAQEAqNGSp/UOLBbRYAaVqzjMmNGdMHjMMHifcpSZA90qogyk+SWgHSx5MUVlymAx2334rWRWD14wtithdlMSq4tziod2J9XjjS14ioWJLwxN084yoYaMnZlZq8AH+kVYwYaLgNd22+1cN+PdFMmCyFVKghKvUebk/yE1iMi/LyoB2V3QtGMP3sg1PTISO+pXlOf70tzuXKBjhdE717l3+3OeDoudvPJWNhWd6V4Be2hzC3Wx26nKMftHFDe8tA2moOPdLpSeN24XlzmFC6Au6jR5YTmyhf3ipg6SWfOLsZtxFX1hde/ZsJ1gyA24sWxSZjB2L+pOcbTu/iobhTUro5VoWQIDAQAB";
    const MODULUS: &str = "qNGSp_UOLBbRYAaVqzjMmNGdMHjMMHifcpSZA90qogyk-SWgHSx5MUVlymAx2334rWRWD14wtithdlMSq4tziod2J9XjjS14ioWJLwxN084yoYaMnZlZq8AH-kVYwYaLgNd22-1cN-PdFMmCyFVKghKvUebk_yE1iMi_LyoB2V3QtGMP3sg1PTISO-pXlOf70tzuXKBjhdE717l3-3OeDoudvPJWNhWd6V4Be2hzC3Wx26nKMftHFDe8tA2moOPdLpSeN24XlzmFC6Au6jR5YTmyhf3ipg6SWfOLsZtxFX1hde_ZsJ1gyA24sWxSZjB2L-pOcbTu_iobhTUro5VoWQ";
    const SIGNATURE: &str = "bPDG20AfZy4HjLtCtgyh3zwMc+SXxIuF+0lMGXdGs+dH2m1sE2X5zmF7yPDlbDXJS+8Q93LmAy0HRw+e8SyixwaAV+kW/jdEsZNuCHXeioalzUpzZpZj0L0hiQs2A5LfuPL63NTe6odXsJKIjG+XEZwv++elqA32iqie4taaYP/M5M9DdTWu8jBTDDm8R6h3Tav8iqxTdjiQNhlo4nB4qlsDMXC7Xf9yWInxjK2rOTkEgg4hNU6c+EOjuvFfdNXSmmuWHY2NVZMgqd+VJTPIXx0jV0RCYgK83+fA9VNnBExAVygDgDviQrnhPupTS9+Q/6KzbMIdmdxtmIP4nBxJLw==";

    fn keys() -> [DecodingKey; 2] {
        [
            DecodingKey::from_rsa_der(&BASE64_STANDARD.decode(DER).unwrap()),
            DecodingKey::from_rsa_components(MODULUS, "AQAB").unwrap(),
        ]
    }

    #[test]
    fn accepts_der_and_jwk_public_keys() {
        let signature = BASE64_STANDARD.decode(SIGNATURE).unwrap();
        for key in keys() {
            let verifier = verifier_factory(&Algorithm::RS256, &key).unwrap();
            assert_eq!(verifier.algorithm(), Algorithm::RS256);
            assert!(verifier.verify(MESSAGE, &signature).is_ok());
        }
    }

    #[test]
    fn rejects_modified_message_and_signature() {
        let signature = BASE64_STANDARD.decode(SIGNATURE).unwrap();
        let mut corrupted = signature.clone();
        corrupted[0] ^= 1;
        for key in keys() {
            let verifier = verifier_factory(&Algorithm::RS256, &key).unwrap();
            assert!(verifier.verify(b"modified message", &signature).is_err());
            assert!(verifier.verify(MESSAGE, &corrupted).is_err());
            assert!(verifier.verify(MESSAGE, &Vec::new()).is_err());
        }
    }

    #[test]
    fn rejects_malformed_public_keys() {
        let signature = BASE64_STANDARD.decode(SIGNATURE).unwrap();
        for key in [
            DecodingKey::from_rsa_der(b"not a DER key"),
            DecodingKey::from_rsa_components("AQ", "AQAB").unwrap(),
        ] {
            let verifier = verifier_factory(&Algorithm::RS256, &key).unwrap();
            assert!(verifier.verify(MESSAGE, &signature).is_err());
        }
    }
}
