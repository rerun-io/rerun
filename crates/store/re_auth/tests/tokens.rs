use std::time::Duration;

use re_auth::{Permission, SecretKey};

#[test]
fn key_base64_round_trip() {
    let expected = SecretKey::generate();
    let base64 = expected.to_base64();
    let actual = SecretKey::from_base64(&base64).unwrap();
    assert_eq!(expected.to_bytes(), actual.to_bytes());
}

#[test]
fn generate_read_only_token_with_duration() {
    let key = SecretKey::from_base64("CKxq6b4Hy3xdjDOWwaShOJab+eu6jcsFso4rbLjJuZ8=").unwrap();

    let token = key
        .token(Duration::from_secs(2 * 60 * 60), Permission::read())
        .unwrap();

    assert!(
        key.verify(&token, Permission::read()).is_ok(),
        "token `{}` should be valid",
        token.as_ref()
    );
}
