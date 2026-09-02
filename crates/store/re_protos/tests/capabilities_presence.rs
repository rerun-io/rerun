//! A server that advertises no capability must be told apart from a server built before
//! capabilities existed, which leaves the field unset.

#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

use prost::Message as _;

use re_protos::capabilities::{ServerCapabilities, catalog_write_register};
use re_protos::cloud::v1alpha1::WhoAmIResponse;

fn response(capabilities: Option<Vec<String>>) -> WhoAmIResponse {
    WhoAmIResponse {
        user_id: None,
        can_read: true,
        can_write: true,
        capabilities: capabilities
            .map(|capabilities| re_protos::cloud::v1alpha1::ServerCapabilities { capabilities }),
    }
}

fn round_trip(response: &WhoAmIResponse) -> ServerCapabilities {
    WhoAmIResponse::decode(response.encode_to_vec().as_slice())
        .unwrap()
        .capabilities
        .map_or_else(ServerCapabilities::unknown, Into::into)
}

/// A response that leaves the field unset decodes to a set that is not known, and one that carries
/// no capability decodes to a known empty set.
#[test]
fn an_empty_capability_set_decodes_apart_from_an_unset_one() {
    let unset = round_trip(&response(None));
    let nothing = round_trip(&response(Some(vec![])));

    assert!(!unset.is_known());
    assert!(nothing.is_known());
    assert_eq!(nothing.advertised(), Some(&[][..]));
}

/// A set that is not known and a known empty set answer every capability query the same way, so
/// only the presence of the field tells them apart.
#[test]
fn an_unknown_and_an_empty_set_answer_every_query_the_same_way() {
    for capabilities in [
        round_trip(&response(None)),
        round_trip(&response(Some(vec![]))),
    ] {
        assert!(!capabilities.has(&catalog_write_register("s3")));
        assert!(capabilities.register_schemes().is_empty());
    }
}

/// A server that advertises no capability writes the field anyway, so its response differs from
/// the response of a server that leaves the field unset.
#[test]
fn advertising_nothing_writes_the_field_anyway() {
    let unset = response(None).encode_to_vec();
    let nothing = response(Some(vec![])).encode_to_vec();

    // Field 4, wire type 2, length 0.
    assert_eq!(nothing, [unset.as_slice(), &[34, 0]].concat());
}
