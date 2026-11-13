//! Protocol levels: what a build of a client or of a server understands and can handle.
//!
//! A protocol level is fixed by the build, ordered, and shared by clients and servers: see
//! `ClientMeta` in `cloud.proto`. It counts revisions within one protobuf package version.
//!
//! What one server is configured to offer is a server capability instead: see
//! [`crate::capabilities`].

/// A protocol level: a protocol revision shared by clients and servers.
///
/// A level implies every level below it. A side claims a level only once it implements its part
/// of that level and of every level below.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolLevel(u32);

impl ProtocolLevel {
    /// Level 0, `baseline`: a client or a server from before protocol levels existed.
    pub const BASELINE: Self = Self(0);

    /// Level 1, `manifest_key_store_id`.
    ///
    /// * Client: picks the manifest out of a fetched `RrdFooter` by `RrdManifestKey.store_id`
    ///   rather than by segment id.
    /// * Server: hands out manifest keys for segments whose segment id is not their store's
    ///   recording id only to clients at this level or above.
    pub const MANIFEST_KEY_STORE_ID: Self = Self(1);

    /// The level this build implements, sent in `ClientMeta.protocol_level`.
    pub const CURRENT: Self = Self::MANIFEST_KEY_STORE_ID;

    /// The level of the client that sent a request. A request without `ClientMeta` comes from a
    /// client at [`Self::BASELINE`].
    pub fn of_client(client_meta: Option<crate::cloud::v1alpha1::ClientMeta>) -> Self {
        client_meta.map_or(Self::BASELINE, |meta| Self(meta.protocol_level))
    }

    /// The level as it goes on the wire.
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// The first Rerun release whose SDK and Viewer implement this level.
    ///
    /// `None` for [`Self::BASELINE`], and for a level this build does not know.
    /// Every level up to [`Self::CURRENT`] must name one: it is what a refused user is told to
    /// upgrade to.
    pub fn first_client_release(self) -> Option<&'static str> {
        match self.0 {
            1 => Some("0.44"),
            _ => None,
        }
    }

    /// Checks a request against `self`, the level required to serve it.
    ///
    /// `subject` names what requires the level, such as `"this dataset"`: it starts the message
    /// of the refusal.
    ///
    /// Fails with `FailedPrecondition` when the client is below `self`.
    /// Always succeeds for [`ClientLevelCheck::Exempt`].
    pub fn ensure_met_by(self, check: ClientLevelCheck, subject: &str) -> tonic::Result<()> {
        let client = match check {
            ClientLevelCheck::Enforce(client_meta) => Self::of_client(client_meta),
            ClientLevelCheck::Exempt => return Ok(()),
        };
        if client >= self {
            return Ok(());
        }

        let requirement = match self.first_client_release() {
            Some(release) => format!("Rerun {release} or newer"),
            None => "a newer version of Rerun".to_owned(),
        };
        Err(tonic::Status::failed_precondition(format!(
            "{subject} requires {requirement}: upgrade the Rerun SDK or Viewer (required protocol \
             level: {self}, client protocol level: {client})"
        )))
    }
}

/// How a request is checked against the protocol level required to serve it.
///
/// Every RPC that is subject to a required level picks one.
#[derive(Debug, Clone, Copy)]
pub enum ClientLevelCheck {
    /// What the RPC serves is misread by a client below the required level: the client that sent
    /// this `ClientMeta` must be at that level.
    Enforce(Option<crate::cloud::v1alpha1::ClientMeta>),

    /// The RPC serves nothing that a client below the required level misreads.
    Exempt,
}

impl From<u32> for ProtocolLevel {
    fn from(level: u32) -> Self {
        Self(level)
    }
}

impl std::fmt::Display for ProtocolLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client meets a required level at or above it, a request without `ClientMeta` counts as
    /// level 0, and an exempt request always passes.
    #[test]
    fn a_required_level_is_met_at_or_above_it() {
        use crate::cloud::v1alpha1::ClientMeta;

        let client = |level: Option<u32>| {
            ClientLevelCheck::Enforce(level.map(|protocol_level| ClientMeta { protocol_level }))
        };
        let required = ProtocolLevel::from(2);

        let baseline = ProtocolLevel::BASELINE;
        assert!(baseline.ensure_met_by(client(None), "this").is_ok());
        assert!(required.ensure_met_by(client(Some(2)), "this").is_ok());
        assert!(required.ensure_met_by(client(Some(3)), "this").is_ok());
        assert!(
            required
                .ensure_met_by(ClientLevelCheck::Exempt, "this")
                .is_ok()
        );

        for below in [None, Some(0), Some(1)] {
            let status = required
                .ensure_met_by(client(below), "this")
                .expect_err("a client below the required level must be refused");
            assert_eq!(status.code(), tonic::Code::FailedPrecondition, "{below:?}");
        }
    }

    /// The refusal names what requires the level, the release to upgrade to, and both levels.
    #[test]
    fn a_refusal_names_the_release_to_upgrade_to() {
        let status = ProtocolLevel::MANIFEST_KEY_STORE_ID
            .ensure_met_by(ClientLevelCheck::Enforce(None), "this dataset")
            .expect_err("a client below the required level must be refused");

        assert_eq!(
            status.message(),
            "this dataset requires Rerun 0.44 or newer: upgrade the Rerun SDK or Viewer (required \
             protocol level: 1, client protocol level: 0)"
        );
    }

    /// A level this build does not know has no release to name, and the refusal still says to
    /// upgrade.
    #[test]
    fn a_refusal_for_an_unknown_level_asks_for_a_newer_version() {
        let unknown = ProtocolLevel::from(ProtocolLevel::CURRENT.as_u32() + 1);
        let status = unknown
            .ensure_met_by(ClientLevelCheck::Enforce(None), "this dataset")
            .expect_err("a client below the required level must be refused");

        assert_eq!(
            status.message(),
            format!(
                "this dataset requires a newer version of Rerun: upgrade the Rerun SDK or Viewer \
                 (required protocol level: {unknown}, client protocol level: 0)"
            )
        );
    }

    /// Every level this build implements names the release a refused user upgrades to.
    #[test]
    fn every_implemented_level_names_its_first_client_release() {
        for level in 1..=ProtocolLevel::CURRENT.as_u32() {
            assert!(
                ProtocolLevel::from(level).first_client_release().is_some(),
                "level {level} has no first client release"
            );
        }
    }
}
