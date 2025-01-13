use std::time::Duration;

use re_auth::{JwtClaims, RedapProvider};

#[test]
fn key_base64_round_trip() {
    let expected = RedapProvider::generate();
    let base64 = expected.to_base64();
    let actual = RedapProvider::from_base64(&base64).unwrap();
    assert_eq!(expected.to_bytes(), actual.to_bytes());
}

#[test]
fn generate_read_only_token_with_duration() {
    let key = RedapProvider::from_base64("CKxq6b4Hy3xdjDOWwaShOJab+eu6jcsFso4rbLjJuZ8=").unwrap();

    let token = key
        .token(
            Duration::from_secs(2 * 60 * 60),
            "re_auth_test",
            "test@rerun.io",
        )
        .unwrap();

    let user: JwtClaims = key.verify(&token).unwrap();

    assert_eq!(user.subject, Some("test@rerun.io".to_owned()));
}
