use std::time::Duration;

use rand::rngs::ThreadRng;
use re_auth::{Error, RedapProvider, SecretKey, VerificationOptions};

const KEY: &str = "CKxq6b4Hy3xdjDOWwaShOJab+eu6jcsFso4rbLjJuZ8=";

#[test]
fn key_base64_round_trip() {
    let expected = SecretKey::generate(ThreadRng::default());
    let base64 = expected.to_base64();
    let actual = SecretKey::from_base64(&base64).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn generate_read_only_token_with_duration() {
    let provider = RedapProvider::from_secret_key_base64(KEY).unwrap();

    let token = provider
        .token(
            Duration::from_secs(2 * 60 * 60),
            "re_auth_test",
            "test@rerun.io",
            re_auth::Permission::ReadWrite,
            None,
        )
        .unwrap();

    let user = provider
        .verify(&token, VerificationOptions::default())
        .unwrap();

    assert_eq!(user.sub(), "test@rerun.io");
    assert_eq!(user.iss(), "re_auth_test");
}

#[test]
fn expired_token() {
    let provider = RedapProvider::from_secret_key_base64(KEY).unwrap();

    let duration = Duration::from_secs(1);

    let token = provider
        .token(
            Duration::from_secs(1),
            "re_auth_test",
            "test@rerun.io",
            re_auth::Permission::ReadWrite,
            None,
        )
        .unwrap();

    std::thread::sleep(duration * 2);

    let user = provider.verify(&token, VerificationOptions::default().without_leeway());

    assert!(
        matches!(user, Err(Error::Jwt(_))),
        "Expected an expired token error"
    );
}
