//! Capability names, describing what a server implements and supports.
//!
//! A capability does not depend on the caller. Whether the caller is allowed to
//! use one is a separate question. The user wants to know both, to tell apart
//! "this server does not support that" and "you do not have permission for that".
//!
//! Naming rules:
//!
//! * Lowercase segments separated by `:`, most general segment first, so that
//!   related capabilities share a prefix that [`ServerCapabilities::has_any_under`]
//!   can query.
//! * A server advertises full names only. A shorter prefix such as
//!   `catalog:write:register` is for querying a group.
//! * An advertised name never contains a `*`. Permissions use wildcards, and
//!   the server expands them against the capability names it knows.
//! * Names are permanent, because permissions refer to them.
//! * The last segment of a group can come from the server's own config, such as
//!   the URL schemes it reads data sources from. Build those names with the
//!   functions in this module, so a server and a client spell them the same way.

/// Separates the segments of a capability name.
const SEPARATOR: char = ':';

/// Prefix for all catalog registering capabilities.
pub const CATALOG_WRITE_REGISTER: &str = "catalog:write:register";

/// The capability a server advertises to register data sources with this URL scheme.
///
/// A server builds one name per scheme it reads, so its config decides which names it advertises.
pub fn catalog_write_register(scheme: &str) -> String {
    format!("{CATALOG_WRITE_REGISTER}{SEPARATOR}{scheme}")
}

/// The capabilities server advertised.
///
/// A capability name is added together with the code that implements it, so a
/// server that registers a data source URL also advertises the matching name. An
/// absent name means the server does not support it.
///
/// A server from before capabilities existed advertises nothing at all, which is
/// [`Self::unknown`] rather than an empty set.
///
/// The lookups therefore answer for what was advertised and nothing more, and
/// the caller picks the fallback.
#[derive(Debug, Clone, PartialEq, Eq, re_byte_size::SizeBytes)]
pub struct ServerCapabilities {
    /// Sorted and deduplicated. `None` when the server advertised nothing.
    advertised: Option<Vec<String>>,
}

impl ServerCapabilities {
    /// The server did not advertise capabilities.
    pub fn unknown() -> Self {
        Self { advertised: None }
    }

    /// The capability names a server advertised, in any order.
    ///
    /// Names this build does not know are kept, so that a query about a group
    /// still finds a newer server's capabilities.
    pub fn from_advertised(names: impl IntoIterator<Item = String>) -> Self {
        let mut advertised: Vec<String> = names.into_iter().collect();
        advertised.sort_unstable();
        advertised.dedup();

        Self {
            advertised: Some(advertised),
        }
    }

    /// Whether the server advertised its capabilities at all.
    pub fn is_known(&self) -> bool {
        self.advertised.is_some()
    }

    /// Every name the server advertised, for logging and for display.
    pub fn advertised(&self) -> Option<&[String]> {
        self.advertised.as_deref()
    }

    /// Whether the server advertised this exact capability.
    pub fn has(&self, capability: &str) -> bool {
        self.advertised
            .as_ref()
            .is_some_and(|advertised| advertised.iter().any(|name| name == capability))
    }

    /// The URL schemes the server advertised it registers data sources from. Empty both for a
    /// server that registers none and for one that advertised nothing, which [`Self::is_known`]
    /// tells apart.
    pub fn register_schemes(&self) -> Vec<&str> {
        let group = format!("{CATALOG_WRITE_REGISTER}{SEPARATOR}");

        self.advertised
            .iter()
            .flatten()
            .filter_map(|name| name.strip_prefix(&group))
            .filter(|scheme| !scheme.contains(SEPARATOR))
            .collect()
    }

    /// Whether the server advertised any capability at or below `prefix`.
    ///
    /// Use it to check whether a group of capabilities is supported at all,
    /// without naming every capability in the group.
    pub fn has_any_under(&self, prefix: &str) -> bool {
        self.advertised.as_ref().is_some_and(|advertised| {
            advertised.iter().any(|name| {
                name == prefix
                    || name
                        .strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with(SEPARATOR))
            })
        })
    }
}

impl From<crate::cloud::v1alpha1::ServerCapabilities> for ServerCapabilities {
    fn from(value: crate::cloud::v1alpha1::ServerCapabilities) -> Self {
        Self::from_advertised(value.capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(names: &[&str]) -> ServerCapabilities {
        ServerCapabilities::from_advertised(names.iter().map(|name| (*name).to_owned()))
    }

    /// A registering capability names its scheme in the last segment, so it sits under the group
    /// prefix that queries the whole group.
    #[test]
    fn a_registering_capability_names_its_scheme_under_the_group() {
        let s3 = catalog_write_register("s3");

        assert_eq!(s3, "catalog:write:register:s3");
        assert!(caps(&[&s3]).has_any_under(CATALOG_WRITE_REGISTER));
    }

    /// A server lists the schemes it advertised a registering capability for, and nothing else. A
    /// name outside the group does not name a scheme.
    #[test]
    fn register_schemes_lists_the_advertised_ones() {
        let caps = caps(&[
            &catalog_write_register("s3"),
            &catalog_write_register("file"),
            "catalog:write:staging",
        ]);

        assert_eq!(caps.register_schemes(), vec!["file", "s3"]);
        assert!(ServerCapabilities::unknown().register_schemes().is_empty());
    }

    /// A name that continues past the scheme segment does not name a scheme, so it is left out.
    #[test]
    fn register_schemes_skips_a_name_with_a_deeper_segment() {
        let caps = caps(&["catalog:write:register:s3:eu"]);

        assert!(caps.register_schemes().is_empty());
    }

    /// An exact query matches only the same name, not another name in the same
    /// group and not the group prefix.
    #[test]
    fn has_matches_only_the_same_name() {
        let caps = caps(&[&catalog_write_register("s3")]);

        assert!(caps.has(&catalog_write_register("s3")));
        assert!(!caps.has(&catalog_write_register("file")));
        assert!(!caps.has(CATALOG_WRITE_REGISTER));
    }

    /// A group query matches a server advertising a name inside the group, and
    /// one advertising the group prefix itself.
    #[test]
    fn has_any_under_matches_names_in_the_group_and_the_prefix() {
        assert!(caps(&[&catalog_write_register("https")]).has_any_under(CATALOG_WRITE_REGISTER));
        assert!(caps(&[CATALOG_WRITE_REGISTER]).has_any_under(CATALOG_WRITE_REGISTER));
    }

    /// A group query compares whole segments, so a longer segment starting with
    /// the same text is a different group.
    #[test]
    fn has_any_under_does_not_match_a_longer_segment() {
        assert!(!caps(&["catalog:write:registerish:s3"]).has_any_under(CATALOG_WRITE_REGISTER));
    }

    /// A newer server advertises names this build does not know, and they do not
    /// affect queries for the names it does know.
    #[test]
    fn unknown_capability_names_are_ignored() {
        let caps = caps(&["catalog:write:staging", &catalog_write_register("s3")]);

        assert!(caps.has(&catalog_write_register("s3")));
        assert!(!caps.has(&catalog_write_register("file")));
        assert!(caps.has_any_under(CATALOG_WRITE_REGISTER));
    }

    /// A server that did not advertise capabilities and one that advertised none
    /// answer every query the same way, and only `is_known` tells them apart.
    #[test]
    fn an_absent_capability_set_is_told_apart_from_an_empty_one() {
        let unknown = ServerCapabilities::unknown();
        let nothing = caps(&[]);

        for caps in [&unknown, &nothing] {
            assert!(!caps.has(&catalog_write_register("s3")));
            assert!(!caps.has_any_under(CATALOG_WRITE_REGISTER));
        }

        assert!(!unknown.is_known());
        assert!(nothing.is_known());
    }

    /// A set that came over the wire is known, and answers for the names it
    /// carried.
    #[test]
    fn an_advertised_set_converts_from_the_wire() {
        let wire = crate::cloud::v1alpha1::ServerCapabilities {
            capabilities: vec![catalog_write_register("s3")],
        };

        let caps = ServerCapabilities::from(wire);

        assert!(caps.is_known());
        assert!(caps.has(&catalog_write_register("s3")));
        assert!(!caps.has(&catalog_write_register("file")));
    }

    /// A capability a server advertises more than once is kept once.
    #[test]
    fn repeated_names_are_kept_once() {
        let s3 = catalog_write_register("s3");
        let caps = caps(&[&s3, &s3]);

        assert_eq!(caps.advertised(), Some(&[s3][..]));
    }
}
