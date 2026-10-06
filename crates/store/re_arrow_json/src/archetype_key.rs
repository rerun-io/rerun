//! How an archetype is keyed in the JSON.
//!
//! Embedders can register archetypes at runtime, so short names are not guaranteed to be unique
//! in a [`Reflection`]. An archetype is keyed by its short name, e.g. `Points3D`, when no other
//! archetype shares it, and by its full name otherwise.

use re_sdk_types::ArchetypeName;
use re_sdk_types::reflection::{ArchetypeReflection, Reflection};

use crate::ValueFromJsonError;

/// The key `name` is written under.
pub fn json_key_of_archetype(reflection: &Reflection, name: ArchetypeName) -> &'static str {
    let short_name = name.short_name();
    let shared = reflection
        .archetypes
        .keys()
        .filter(|other| other.short_name() == short_name)
        .nth(1)
        .is_some();
    if shared { name.full_name() } else { short_name }
}

/// The archetype keyed by `key`: an exact full name first, else a short name no other archetype
/// shares.
pub fn archetype_by_json_key<'r>(
    reflection: &'r Reflection,
    key: &str,
) -> Result<(ArchetypeName, &'r ArchetypeReflection), ValueFromJsonError> {
    if let Some((name, archetype)) = reflection
        .archetypes
        .iter()
        .find(|(name, _)| name.full_name() == key)
    {
        return Ok((*name, archetype));
    }

    let mut matches = reflection
        .archetypes
        .iter()
        .filter(|(name, _)| name.short_name() == key);
    match (matches.next(), matches.next()) {
        (Some((name, archetype)), None) => Ok((*name, archetype)),
        (None, _) => Err(ValueFromJsonError::UnknownArchetype {
            name: key.to_owned(),
            expected: json_keys_of_archetypes(reflection),
        }),
        (Some(_), Some(_)) => {
            let mut candidates: Vec<_> = reflection
                .archetypes
                .keys()
                .filter(|name| name.short_name() == key)
                .map(|name| name.full_name())
                .collect();
            candidates.sort_unstable();
            Err(ValueFromJsonError::AmbiguousArchetype {
                name: key.to_owned(),
                candidates: candidates.join(", "),
            })
        }
    }
}

/// Every known archetype's key, sorted, for an error listing them.
fn json_keys_of_archetypes(reflection: &Reflection) -> String {
    let keys: std::collections::BTreeSet<&str> = reflection
        .archetypes
        .keys()
        .map(|name| json_key_of_archetype(reflection, *name))
        .collect();
    keys.into_iter().collect::<Vec<_>>().join(", ")
}
