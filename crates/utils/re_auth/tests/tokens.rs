use rand::rngs::ThreadRng;
use std::time::Duration;

use re_auth::{Error, RedapProvider, VerificationOptions};

const KEY: &str = "CKxq6b4Hy3xdjDOWwaShOJab+eu6jcsFso4rbLjJuZ8=";

#[test]
fn key_base64_round_trip() {
    let expected = RedapProvider::generate(ThreadRng::default());
    let base64 = expected.to_base64();
    let actual = RedapProvider::from_base64(&base64).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn generate_read_only_token_with_duration() {
    let key = RedapProvider::from_base64(KEY).unwrap();

    let token = key
        .token(
            Duration::from_secs(2 * 60 * 60),
            "re_auth_test",
            "test@rerun.io",
        )
        .unwrap();

    let user = key.verify(&token, VerificationOptions::default()).unwrap();

    assert_eq!(&user.sub, "test@rerun.io");
    assert_eq!(&user.iss, "re_auth_test");
}

#[test]
fn expired_token() {
    let key = RedapProvider::from_base64(KEY).unwrap();

    let duration = Duration::from_secs(1);

    let token = key
        .token(Duration::from_secs(1), "re_auth_test", "test@rerun.io")
        .unwrap();

    std::thread::sleep(duration * 2);

    let user = key.verify(&token, VerificationOptions::default().without_leeway());

    assert!(
        matches!(user, Err(Error::Jwt(_))),
        "Expected an expired token error"
    );
}
