use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;

use crate::EntityPath;

/// Error returned by [`EntityPathFilter::resolve_strict`] and [`EntityPathFilter::parse_strict`].
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum EntityPathFilterError {
    #[error("Path parse error: {0}")]
    PathParseError(#[from] crate::PathParseError),

    #[error("Unresolved substitution: {0}")]
    UnresolvedSubstitution(String),
}

/// A set of substitutions for entity paths.
///
/// Important: the same substitutions must be used in every place we resolve [`EntityPathFilter`] to
/// [`ResolvedEntityPathFilter`].
#[derive(Debug)]
pub struct EntityPathSubs(HashMap<String, String>);

impl EntityPathSubs {
    /// Create a new set of substitutions from a single origin.
    pub fn new_with_origin(origin: &EntityPath) -> Self {
        Self(std::iter::once(("origin".to_owned(), origin.to_string())).collect())
    }

    /// No variable substitutions.
    pub fn empty() -> Self {
        Self(HashMap::default())
    }
}

/// A way to filter a set of `EntityPath`s.
///
/// This implements as simple set of include/exclude rules:
///
/// ```diff
/// + /world/**           # add everything…
/// - /world/roads/**     # …but remove all roads…
/// + /world/roads/main   # …but show main road
/// ```
///
/// If there is multiple matching rules, the most specific rule wins.
/// If there are multiple rules of the same specificity, the last one wins.
/// If no rules match, the path is excluded.
///
/// The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
/// (`/world/**` matches both `/world` and `/world/car/driver`).
/// Other uses of `*` are not (yet) supported.
///
/// Since variable substitution (and thus path parsing) hasn't been performed yet,
/// the rules can not be sorted yet from general to specific, instead they are stored
/// in alphabetical order.
/// To expand variables & evaluate the filter, use [`ResolvedEntityPathFilter`].
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct EntityPathFilter {
    rules: BTreeMap<EntityPathRule, RuleEffect>,
}

impl std::str::FromStr for EntityPathFilter {
    type Err = EntityPathFilterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_strict(value)
    }
}

impl std::fmt::Debug for EntityPathFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Keep it compact for the sake of snapshot tests:
        self.rules
            .iter()
            .map(|(rule, effect)| {
                let sign = match effect {
                    RuleEffect::Include => '+',
                    RuleEffect::Exclude => '-',
                };
                format!("{sign} {rule:?}")
            })
            .collect_vec()
            .fmt(f)
    }
}

/// An [`EntityPathFilter`] with all variables Resolved.
///
/// [`ResolvedEntityPathFilter`] sorts the rule by specificity of the entity path,
/// with recursive coming before non-recursive.
/// This means the last matching rule is also the most specific one.
/// For instance:
///
/// ```diff
/// + /world/**
/// - /world
/// - /world/car/**
/// + /world/car/driver
/// ```
///
/// The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
/// The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
/// The last rule matching `/world` is `- /world`, so it is excluded.
/// The last rule matching `/world/house` is `+ /world/**`, so it is included.

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ResolvedEntityPathFilter {
    rules: BTreeMap<ResolvedEntityPathRule, RuleEffect>,
}

impl ResolvedEntityPathFilter {
    /// Creates an filter that matches [`EntityPath::properties`].
    pub fn properties() -> Self {
        // TODO(grtlr): Consider using `OnceLock` here to cache this.
        Self {
            rules: std::iter::once((
                ResolvedEntityPathRule::including_subtree(&EntityPath::properties()),
                RuleEffect::Include,
            ))
            .collect(),
        }
    }
}

impl std::fmt::Debug for ResolvedEntityPathFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ResolvedEntityPathFilter({:?})", self.formatted())
    }
}

/// A single entity path rule.
///
/// This is a raw, whitespace trimmed path expression without any variable substitutions.
/// See [`EntityPathFilter`] for more information.
///
/// Note that ordering of unresolved entity path rules is simply alphabetical.
/// In contrast, [`ResolvedEntityPathRule`] are ordered by entity path from least specific to most specific.
#[derive(Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct EntityPathRule(String);

impl From<EntityPath> for EntityPathRule {
    #[inline]
    fn from(entity_path: EntityPath) -> Self {
        Self::exact_entity(&entity_path)
    }
}

impl std::ops::Deref for EntityPathRule {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for EntityPathRule {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A path rule with all variables resolved to entity paths.
#[derive(Clone, Debug)]
pub struct ResolvedEntityPathRule {
    /// The original rule, with unresolved variables.
    pub rule: EntityPathRule,

    /// The resolved path, with all variables Resolved.
    pub resolved_path: EntityPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleEffect {
    Include,
    Exclude,
}

impl std::ops::AddAssign for EntityPathFilter {
    /// The union of all rules
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.rules.extend(rhs.rules);
    }
}

impl std::iter::Sum for EntityPathFilter {
    /// The union of all rules
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = Self::default();
        for item in iter {
            sum += item;
        }
        sum
    }
}

/// Split a string into whitespace-separated tokens with extra logic.
///
/// Specifically, we allow for whitespace between `+`/`-` and the following token.
///
/// Additional rules:
///  - Escaped whitespace never results in a split.
///  - Otherwise always split on `\n` (even if it follows a `+` or `-` character).
///  - Only consider `+` and `-` characters as special if they are the first character of a token.
///  - Split on whitespace does not following a relevant `+` or `-` character.
fn split_whitespace_smart(path: &'_ str) -> Vec<&'_ str> {
    #![expect(clippy::unwrap_used)]

    // We parse on bytes, and take care to only split on either side of a one-byte ASCII,
    // making the `from_utf8(…)`s below safe to unwrap.
    let mut bytes = path.as_bytes();

    let mut tokens = vec![];

    // Start by ignoring any leading whitespace
    while !bytes.is_empty() {
        let mut i = 0;
        let mut is_in_escape = false;
        let mut is_include_exclude = false;
        let mut new_token = true;

        // Find the next unescaped whitespace character not following a '+' or '-' character
        while i < bytes.len() {
            let is_unescaped_whitespace = !is_in_escape && bytes[i].is_ascii_whitespace();

            let is_unescaped_newline = !is_in_escape && bytes[i] == b'\n';

            if is_unescaped_newline || (!is_include_exclude && is_unescaped_whitespace) {
                break;
            }

            is_in_escape = bytes[i] == b'\\';

            if bytes[i] == b'+' || bytes[i] == b'-' {
                is_include_exclude = new_token;
            } else if !is_unescaped_whitespace {
                is_include_exclude = false;
                new_token = false;
            }

            i += 1;
        }
        if i > 0 {
            tokens.push(&bytes[..i]);
        }

        // Continue skipping whitespace characters
        while i < bytes.len() {
            if is_in_escape || !bytes[i].is_ascii_whitespace() {
                break;
            }
            is_in_escape = bytes[i] == b'\\';
            i += 1;
        }

        bytes = &bytes[i..];
    }

    // unwrap: we split at proper character boundaries
    tokens
        .iter()
        .map(|token| std::str::from_utf8(token).unwrap())
        .collect()
}

impl EntityPathFilter {
    /// Iterates the expressions in alphabetical order.
    ///
    /// This is **not** the order the rules are evaluated in
    /// (use [`ResolvedEntityPathFilter::iter_unresolved_expressions`] for that instead).
    pub fn iter_expressions(&self) -> impl Iterator<Item = String> + '_ {
        self.rules.iter().map(|(filter, effect)| {
            let mut s = String::new();
            s.push_str(match effect {
                RuleEffect::Include => "+ ",
                RuleEffect::Exclude => "- ",
            });
            s.push_str(&filter.0);
            s
        })
    }

    /// Parse an entity path filter from a string, ignoring any parsing errors.
    ///
    /// Example of rules:
    ///
    /// ```diff
    /// + /world/**
    /// - /world/roads/**
    /// + /world/roads/main
    /// ```
    ///
    /// Each line is a rule.
    ///
    /// The first character should be `+` or `-`. If missing, `+` is assumed.
    /// The rest of the line is trimmed and treated as an entity path after variable substitution through [`Self::resolve_forgiving`]/[`Self::resolve_strict`].
    ///
    /// Conflicting rules are resolved by the last rule.
    pub fn parse_forgiving(rules: impl AsRef<str>) -> Self {
        let split_rules = split_whitespace_smart(rules.as_ref());
        Self::from_query_expressions(split_rules)
    }

    /// Parse an entity path filter from a string.
    ///
    /// Example of rules:
    ///
    /// ```diff
    /// + /world/**
    /// - /world/roads/**
    /// + /world/roads/main
    /// ```
    ///
    /// Each line is a rule.
    ///
    /// The first character should be `+` or `-`. If missing, `+` is assumed.
    /// The rest of the line is trimmed and treated as an entity path after variable substitution through [`Self::resolve_forgiving`]/[`Self::resolve_strict`].
    ///
    /// Conflicting rules are resolved by the last rule.
    #[expect(clippy::unnecessary_wraps)] // TODO(andreas): Do some error checking here?
    pub fn parse_strict(rules: impl AsRef<str>) -> Result<Self, EntityPathFilterError> {
        Ok(Self::parse_forgiving(rules))
    }

    /// Build a filter from a list of query expressions.
    ///
    /// Each item in the iterator should be a query expression.
    ///
    /// The first character should be `+` or `-`. If missing, `+` is assumed.
    /// The rest of the expression is trimmed and treated as an entity path.
    ///
    /// Conflicting rules are resolved by the last rule.
    pub fn from_query_expressions<'a>(rules: impl IntoIterator<Item = &'a str>) -> Self {
        let mut filter = Self::default();

        for line in rules
            .into_iter()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
        {
            let (effect, path_pattern) = match line.chars().next() {
                Some('+') => (RuleEffect::Include, &line[1..]),
                Some('-') => (RuleEffect::Exclude, &line[1..]),
                _ => (RuleEffect::Include, line),
            };

            let rule = EntityPathRule::new(path_pattern);

            filter.rules.insert(rule, effect);
        }

        filter
    }

    /// Adds a rule to this filter.
    ///
    /// If there's already an effect for the rule, it is overwritten with the new effect.
    pub fn insert_rule(&mut self, effect: RuleEffect, rule: EntityPathRule) {
        self.rules.insert(rule, effect);
    }

    /// Creates a filter that accepts everything.
    pub fn all() -> Self {
        Self {
            rules: std::iter::once((EntityPathRule::including_subtree(""), RuleEffect::Include))
                .collect(),
        }
    }

    /// Include this path or variable expression, but not the subtree.
    pub fn add_exact(&mut self, path_or_variable: &str) {
        self.rules
            .insert(EntityPathRule::exact(path_or_variable), RuleEffect::Include);
    }

    /// Include this entity, but not the subtree.
    pub fn add_exact_entity(&mut self, path: &EntityPath) {
        self.rules
            .insert(EntityPathRule::exact_entity(path), RuleEffect::Include);
    }

    /// Include this entity or variable expression with subtree.
    pub fn add_subtree(&mut self, path_or_variable: &str) {
        self.rules.insert(
            EntityPathRule::including_subtree(path_or_variable),
            RuleEffect::Include,
        );
    }

    /// Include this entity with subtree.
    pub fn add_entity_subtree(&mut self, entity_path: &EntityPath) {
        self.rules.insert(
            EntityPathRule::including_entity_subtree(entity_path),
            RuleEffect::Include,
        );
    }

    /// Creates a new entity path filter that includes only a single path or variable expression.
    pub fn single_filter(path_or_variable: &str) -> Self {
        let mut filter = Self::default();
        filter.add_exact(path_or_variable);
        filter
    }

    /// Creates a new entity path filter that includes only a single entity.
    pub fn single_entity_filter(entity_path: &EntityPath) -> Self {
        let mut filter = Self::default();
        filter.add_exact_entity(entity_path);
        filter
    }

    /// Creates a new entity path filter that includes a single subtree.
    pub fn subtree_filter(path_or_variable: &str) -> Self {
        let mut filter = Self::default();
        filter.add_subtree(path_or_variable);
        filter
    }

    /// Creates a new entity path filter that includes a single subtree.
    ///
    /// To use this with unsubsituted variables, use [`Self::subtree_filter`] instead.
    pub fn subtree_entity_filter(entity_path: &EntityPath) -> Self {
        let mut filter = Self::default();
        filter.add_entity_subtree(entity_path);
        filter
    }

    /// Resolve variables & parse paths, ignoring any errors.
    ///
    /// If there is no mention of [`EntityPath::properties`] in the filter, it will be added.
    pub fn resolve_forgiving(&self, subst_env: &EntityPathSubs) -> ResolvedEntityPathFilter {
        let mut seen_properties = false;

        let mut rules: BTreeMap<ResolvedEntityPathRule, RuleEffect> = self
            .rules
            .iter()
            .map(|(rule, effect)| {
                (
                    ResolvedEntityPathRule::parse_forgiving(rule, subst_env),
                    *effect,
                )
            })
            .inspect(|(ResolvedEntityPathRule { resolved_path, .. }, _)| {
                if resolved_path.starts_with(&EntityPath::properties()) {
                    seen_properties = true;
                }
            })
            .collect();

        if !seen_properties {
            rules.insert(
                ResolvedEntityPathRule::including_subtree(&EntityPath::properties()),
                RuleEffect::Exclude,
            );
        }

        ResolvedEntityPathFilter { rules }
    }

    /// Resolve variables & parse paths, returning an error if any rule cannot be parsed or variable substitution fails.
    pub fn resolve_strict(
        self,
        subst_env: &EntityPathSubs,
    ) -> Result<ResolvedEntityPathFilter, EntityPathFilterError> {
        let mut seen_properties = false;

        let mut rules = self
            .rules
            .into_iter()
            .map(|(rule, effect)| {
                ResolvedEntityPathRule::parse_strict(&rule, subst_env).map(|r| (r, effect))
            })
            .inspect(|maybe_rule| {
                if let Ok((ResolvedEntityPathRule { resolved_path, .. }, _)) = maybe_rule
                    && resolved_path.starts_with(&EntityPath::properties())
                {
                    seen_properties = true;
                }
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        if !seen_properties {
            rules.insert(
                ResolvedEntityPathRule::including_subtree(&EntityPath::properties()),
                RuleEffect::Exclude,
            );
        }

        Ok(ResolvedEntityPathFilter { rules })
    }

    /// Resolve variables & parse paths, without any substitutions.
    pub fn resolve_without_substitutions(self) -> ResolvedEntityPathFilter {
        self.resolve_forgiving(&EntityPathSubs::empty())
    }

    #[inline]
    /// Iterate over all rules in the filter.
    pub fn rules(&self) -> impl Iterator<Item = (&EntityPathRule, &RuleEffect)> {
        self.rules.iter()
    }
}

impl ResolvedEntityPathFilter {
    /// Turns the resolved filter back into an unresolved filter.
    ///
    /// The returned [`EntityPathFilter`] will _not_ contain the default exclusion of the recording properties.
    ///
    /// Warning: Iterating over the rules in the unresolved filter will yield a different order
    /// than the order of the rules in the resolved filter.
    /// To preserve the order, use [`Self::iter_unresolved_expressions`] instead.
    pub fn unresolved(&self) -> EntityPathFilter {
        EntityPathFilter {
            rules: self
                .rules
                .iter()
                .filter_map(|(rule, effect)| {
                    if rule == &ResolvedEntityPathRule::including_subtree(&EntityPath::properties())
                        && effect == &RuleEffect::Exclude
                    {
                        None
                    } else {
                        Some((rule.rule.clone(), *effect))
                    }
                })
                .collect(),
        }
    }

    fn iter_unresolved_expressions_impl(
        &self,
        with_properties: bool,
    ) -> impl Iterator<Item = String> + '_ {
        // Do **not** call `unresolved()` because this would yield a different order!

        self.rules.iter().filter_map(move |(filter, effect)| {
            if !with_properties
                && filter.rule
                    == EntityPathRule::including_entity_subtree(&EntityPath::properties())
                && effect == &RuleEffect::Exclude
            {
                return None;
            }

            let mut s = String::new();
            s.push_str(match effect {
                RuleEffect::Include => "+ ",
                RuleEffect::Exclude => "- ",
            });
            s.push_str(&filter.rule);
            Some(s)
        })
    }

    /// Iterate over the raw expressions of the rules, displaying the raw unresolved expressions.
    ///
    /// Note that they are iterated in the order of the resolved rules in contrast to [`EntityPathFilter::iter_expressions`].
    pub fn iter_unresolved_expressions(&self) -> impl Iterator<Item = String> + '_ {
        self.iter_unresolved_expressions_impl(true)
    }

    /// Iterate over the raw expressions of the rules, displaying the raw unresolved expressions.
    ///
    /// Note that they are iterated in the order of the resolved rules in contrast to [`EntityPathFilter::iter_expressions`].
    pub fn iter_unresolved_expressions_without_properties(
        &self,
    ) -> impl Iterator<Item = String> + '_ {
        self.iter_unresolved_expressions_impl(false)
    }

    pub fn formatted(&self) -> String {
        self.iter_unresolved_expressions().join("\n")
    }

    pub fn formatted_without_properties(&self) -> String {
        self.iter_unresolved_expressions_without_properties()
            .join("\n")
    }

    /// Find the most specific matching rule and return its effect.
    /// If no rule matches, return `None`.
    pub fn most_specific_match(&self, path: &EntityPath) -> Option<RuleEffect> {
        // We sort the rule by entity path, with recursive coming before non-recursive.
        // This means the last matching rule is also the most specific one.
        // We can definitely optimize this at some point, especially when matching
        // again an `EntityTree` where we could potentially cut out whole subtrees.
        for (rule, effect) in self.rules.iter().rev() {
            if rule.matches(path) {
                return Some(*effect);
            }
        }
        None
    }

    /// Does this filter include the given entity path?
    pub fn matches(&self, path: &EntityPath) -> bool {
        let effect = self
            .most_specific_match(path)
            .unwrap_or(RuleEffect::Exclude);
        match effect {
            RuleEffect::Include => true,
            RuleEffect::Exclude => false,
        }
    }

    /// Is there a rule for this exact entity path (ignoring subtree)?
    pub fn matches_exactly(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, effect)| {
            effect == &RuleEffect::Include
                && !rule.rule.include_subtree()
                && rule.resolved_path == *entity_path
        })
    }

    /// Adds a rule to this filter.
    ///
    /// If there's already an effect for the rule, it is overwritten with the new effect.
    pub fn add_rule(&mut self, effect: RuleEffect, rule: ResolvedEntityPathRule) {
        self.rules.insert(rule, effect);
    }

    /// Remove a subtree and any existing rules that it would match.
    ///
    /// Because most-specific matches win, if we only add a subtree exclusion
    /// it can still be overridden by existing inclusions. This method ensures
    /// that not only do we add a subtree exclusion, but clear out any existing
    /// inclusions or (now redundant) exclusions that would match the subtree.
    pub fn remove_subtree_and_matching_rules(&mut self, entity_path: EntityPath) {
        let new_exclusion = ResolvedEntityPathRule {
            rule: EntityPathRule::including_entity_subtree(&entity_path),
            resolved_path: entity_path,
        };

        // Remove any rule that is a subtree of the new exclusion.
        self.rules
            .retain(|rule, _| !new_exclusion.matches(&rule.resolved_path));

        self.rules.insert(new_exclusion, RuleEffect::Exclude);
    }

    /// Remove any rule for the given entity path (ignoring whether or not that rule includes the subtree).
    pub fn remove_rule_for(&mut self, entity_path: &EntityPath) {
        self.rules
            .retain(|rule, _| rule.resolved_path != *entity_path);
    }

    /// Is there any rule for this entity path?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn contains_rule_for_exactly(&self, entity_path: &EntityPath) -> bool {
        self.rules
            .iter()
            .any(|(rule, _)| rule.resolved_path == *entity_path)
    }

    /// Is this entity path explicitly included?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn is_explicitly_included(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, effect)| {
            rule.resolved_path == *entity_path && effect == &RuleEffect::Include
        })
    }

    /// Is this entity path explicitly excluded?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn is_explicitly_excluded(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, effect)| {
            rule.resolved_path == *entity_path && effect == &RuleEffect::Exclude
        })
    }

    /// Is anything under this path included (including self)?
    pub fn is_anything_in_subtree_included(&self, path: &EntityPath) -> bool {
        for (rule, effect) in &self.rules {
            if effect == &RuleEffect::Include && rule.resolved_path.starts_with(path) {
                return true; // something in this subtree is explicitly included
            }
        }

        // We sort the rule by entity path, with recursive coming before non-recursive.
        // This means the last matching rule is also the most specific one.
        for (rule, effect) in self.rules.iter().rev() {
            if rule.matches(path) {
                match effect {
                    RuleEffect::Include => {
                        return true; // the entity (with or without subtree) is explicitly included
                    }
                    RuleEffect::Exclude => {
                        if rule.rule.include_subtree() {
                            // the subtree is explicitly excluded,
                            // and we've already checked that nothing in the subtree was included.
                            return false;
                        } else {
                            // the entity is excluded, but (maybe!) not thee entire subtree.
                        }
                    }
                }
            }
        }

        false // no matching rule and we are exclude-by-default.
    }

    /// Checks whether this results of this filter "fully contain" the results of another filter.
    ///
    /// If this returns `true` there should not exist any [`EntityPath`] for which [`Self::matches`]
    /// would return `true` and the other filter would return `false` for this filter.
    ///
    /// This check operates purely on the rule expressions and not the actual entity tree,
    /// and will thus not reason about entities included in an actual recording:
    /// different queries that are *not* a superset of each other may still query the same entities
    /// in given recording.
    ///
    /// This is a conservative estimate, and may return `false` in situations where the
    /// query does in fact cover the other query. However, it should never return `true`
    /// in a case where the other query would not be fully covered.
    pub fn is_superset_of(&self, other: &Self) -> bool {
        // First check that we include everything included by other
        for (other_rule, other_effect) in &other.rules {
            match other_effect {
                RuleEffect::Include => {
                    if let Some((self_rule, self_effect)) = self
                        .rules
                        .iter()
                        .rev()
                        .find(|(r, _)| r.matches(&other_rule.resolved_path))
                    {
                        match self_effect {
                            RuleEffect::Include => {
                                // If the other rule includes the subtree, but the matching
                                // rule doesn't, then we don't fully contain the other rule.
                                if other_rule.rule.include_subtree()
                                    && !self_rule.rule.include_subtree()
                                {
                                    return false;
                                }
                            }
                            RuleEffect::Exclude => return false,
                        }
                    } else {
                        // No matching rule means this path isn't included
                        return false;
                    }
                }
                RuleEffect::Exclude => {}
            }
        }
        // Next check that the other rule hasn't included something that we've excluded
        for (self_rule, self_effect) in &self.rules {
            match self_effect {
                RuleEffect::Include => {}
                RuleEffect::Exclude => {
                    if let Some((_, other_effect)) = other
                        .rules
                        .iter()
                        .rev()
                        .find(|(r, _)| r.matches(&self_rule.resolved_path))
                    {
                        match other_effect {
                            RuleEffect::Include => {
                                return false;
                            }
                            RuleEffect::Exclude => {}
                        }
                    }
                }
            }
        }
        // If we got here, we checked every inclusion rule in `other` and they all had a more-inclusive
        // inclusion rule and didn't hit an exclusion rule.
        true
    }

    #[inline]
    /// Iterate over all rules in the filter.
    pub fn rules(&self) -> impl Iterator<Item = (&ResolvedEntityPathRule, &RuleEffect)> {
        self.rules.iter()
    }

    /// Evaluate how a path matches against this filter.
    ///
    /// This returns detailed information about:
    /// - Whether any part of the subtree rooted at this path should be included
    /// - Whether this specific path matches the filter
    /// - Whether this specific path is explicitly included (not just via subtree)
    pub fn evaluate(&self, path: &EntityPath) -> FilterEvaluation {
        let mut subtree_included = false;
        let mut matches_exactly = false;
        let mut last_match: Option<(RuleEffect, bool)> = None;
        let mut found_include_in_subtree = false;

        for (rule, effect) in self.rules() {
            if !found_include_in_subtree
                && *effect == RuleEffect::Include
                && rule.resolved_path.starts_with(path)
            {
                found_include_in_subtree = true;
                subtree_included = true;
            }

            if !matches_exactly
                && *effect == RuleEffect::Include
                && !rule.rule.include_subtree()
                && rule.resolved_path == *path
            {
                matches_exactly = true;
            }

            if rule.matches(path) {
                last_match = Some((*effect, rule.rule.include_subtree()));
            }
        }

        if let Some((effect, include_subtree)) = last_match {
            match effect {
                RuleEffect::Include => subtree_included = true,
                RuleEffect::Exclude => {
                    if include_subtree && !found_include_in_subtree {
                        // Entire subtree is excluded, and we've already checked that nothing
                        // in the subtree was explicitly included.
                        subtree_included = false;
                    }
                }
            }
        }

        let matches = last_match.is_some_and(|(effect, _)| effect == RuleEffect::Include);

        FilterEvaluation {
            subtree_included,
            matches,
            matches_exactly,
        }
    }
}

/// Result of evaluating a filter against an entity path.
///
/// This provides detailed information about how a path matches a filter,
/// which is useful for efficiently walking entity trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterEvaluation {
    /// Whether any part of the subtree rooted at this path should be included.
    ///
    /// If `false`, the entire subtree can be skipped during tree traversal.
    pub subtree_included: bool,

    /// Whether this specific path matches the filter.
    pub matches: bool,

    /// Whether this specific path is explicitly included (not just via a subtree rule).
    ///
    /// This is `true` when there's an exact (non-subtree) inclusion rule for this path.
    pub matches_exactly: bool,
}

impl EntityPathRule {
    /// Create a new [`EntityPathRule`] from a string.
    pub fn new(expression: &str) -> Self {
        Self(expression.trim().to_owned())
    }

    /// Whether this rule includes a subtree.
    #[inline]
    pub fn include_subtree(&self) -> bool {
        self.0.ends_with("/**")
    }

    /// Match this path or variable expression, but not children.
    #[inline]
    pub fn exact(path_or_variable: &str) -> Self {
        Self(path_or_variable.to_owned())
    }

    /// Match this path, but not children.
    #[inline]
    pub fn exact_entity(path: &EntityPath) -> Self {
        Self(path.to_string())
    }

    /// Match this path or variable expression and any entity in its subtree.
    #[inline]
    pub fn including_subtree(path_or_variable: &str) -> Self {
        Self(format!("{path_or_variable}/**"))
    }

    /// Match this path and any entity in its subtree.
    #[inline]
    pub fn including_entity_subtree(entity_path: &EntityPath) -> Self {
        Self::including_subtree(&entity_path.to_string())
    }
}

impl ResolvedEntityPathRule {
    /// Whether this rule matches the given path.
    #[inline]
    pub fn matches(&self, path: &EntityPath) -> bool {
        if self.rule.include_subtree() {
            path.starts_with(&self.resolved_path)
        } else {
            path == &self.resolved_path
        }
    }

    /// Match this path, but not children.
    #[inline]
    pub fn exact_entity(path: &EntityPath) -> Self {
        Self {
            rule: EntityPathRule::exact_entity(path),
            resolved_path: path.clone(),
        }
    }

    /// Match this path and any entity in its subtree.
    #[inline]
    pub fn including_subtree(entity_path: &EntityPath) -> Self {
        Self {
            rule: EntityPathRule::including_entity_subtree(entity_path),
            resolved_path: entity_path.clone(),
        }
    }

    fn substitute_variables(rule: &EntityPathRule, subst_env: &EntityPathSubs) -> String {
        // TODO(#5528): This is a very naive implementation of variable substitution.
        // unclear if we want to do this here, push this down into `EntityPath::parse`,
        // or even supported deferred evaluation on the `EntityPath` itself.
        let mut expression_sub = rule.0.clone();
        for (key, value) in &subst_env.0 {
            expression_sub = expression_sub.replace(format!("${key}").as_str(), value);
            expression_sub = expression_sub.replace(format!("${{{key}}}").as_str(), value);
        }
        expression_sub
    }

    pub fn parse_strict(
        expression: &str,
        subst_env: &EntityPathSubs,
    ) -> Result<Self, EntityPathFilterError> {
        let rule = EntityPathRule::new(expression);
        let expression_sub = Self::substitute_variables(&rule, subst_env);

        // Check for unresolved substitutions.
        if let Some(start) = expression_sub.find('$') {
            let rest = &expression_sub[start + 1..];
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            return Err(EntityPathFilterError::UnresolvedSubstitution(
                rest[..end].to_owned(),
            ));
        }

        if expression_sub == "/**" {
            Ok(Self {
                rule,
                resolved_path: EntityPath::root(),
            })
        } else if let Some(path) = expression_sub.strip_suffix("/**") {
            Ok(Self {
                rule,
                resolved_path: EntityPath::parse_strict(path)?,
            })
        } else {
            Ok(Self {
                rule,
                resolved_path: EntityPath::parse_strict(&expression_sub)?,
            })
        }
    }

    pub fn parse_forgiving(expression: &str, subst_env: &EntityPathSubs) -> Self {
        let rule = EntityPathRule::new(expression);
        let expression_sub = Self::substitute_variables(&rule, subst_env);

        if expression_sub == "/**" {
            Self {
                rule,
                resolved_path: EntityPath::root(),
            }
        } else if let Some(path) = expression_sub.strip_suffix("/**") {
            Self {
                rule,
                resolved_path: EntityPath::parse_forgiving(path),
            }
        } else {
            Self {
                rule,
                resolved_path: EntityPath::parse_forgiving(&expression_sub),
            }
        }
    }
}

impl std::fmt::Display for ResolvedEntityPathRule {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            rule,
            resolved_path: path,
        } = self;

        f.write_fmt(format_args!(
            "{path}{}{}",
            if path.is_root() { "" } else { "/" },
            if rule.include_subtree() { "**" } else { "" }
        ))
    }
}

impl PartialEq for ResolvedEntityPathRule {
    fn eq(&self, other: &Self) -> bool {
        // Careful! This has to check the same fields as `Ord`/`Hash`!
        self.rule.include_subtree() == other.rule.include_subtree()
            && self.resolved_path == other.resolved_path
    }
}

impl Eq for ResolvedEntityPathRule {}

impl std::cmp::Ord for ResolvedEntityPathRule {
    /// Most specific last, which means recursive first.
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Careful! This has to check the same fields as `PartialEq`/`Hash`!
        (&self.resolved_path, !self.rule.include_subtree())
            .cmp(&(&other.resolved_path, !other.rule.include_subtree()))
    }
}

impl std::cmp::PartialOrd for ResolvedEntityPathRule {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::hash::Hash for ResolvedEntityPathRule {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.rule.include_subtree(), self.resolved_path.hash()).hash(state);
    }
}

#[cfg(test)]
mod tests {
    use crate::path::entity_path_filter::{ResolvedEntityPathRule, split_whitespace_smart};
    use crate::{EntityPath, EntityPathFilter, EntityPathSubs, RuleEffect};

    #[test]
    fn test_resolved_rule_order() {
        use std::cmp::Ordering;

        fn check_total_order(rules: &[ResolvedEntityPathRule]) {
            fn ordering_str(ord: Ordering) -> &'static str {
                match ord {
                    Ordering::Greater => ">",
                    Ordering::Equal => "=",
                    Ordering::Less => "<",
                }
            }

            for (i, x) in rules.iter().enumerate() {
                for (j, y) in rules.iter().enumerate() {
                    let actual_ordering = x.cmp(y);
                    let expected_ordering = i.cmp(&j);
                    assert!(
                        actual_ordering == expected_ordering,
                        "Got {x:?} {} {y:?}; expected {x:?} {} {y:?}",
                        ordering_str(actual_ordering),
                        ordering_str(expected_ordering),
                    );
                }
            }
        }

        let rules = [
            "/**",
            "/apa",
            "/world/**",
            "/world/",
            "/world/car",
            "/world/car/driver",
            "/x/y/z",
        ];
        let rules = rules
            .map(|rule| ResolvedEntityPathRule::parse_forgiving(rule, &EntityPathSubs::empty()));
        check_total_order(&rules);
    }

    #[test]
    fn test_entity_path_filter() {
        let subst_env = EntityPathSubs::empty();

        let properties = format!("{}/**", EntityPath::properties());

        let filter = EntityPathFilter::parse_forgiving(format!(
            r#"
            - {properties}
            + /world/**
            - /world/
            - /world/car/**
            + /world/car/driver
            "#
        ))
        .resolve_forgiving(&subst_env);

        for (path, expected_effect) in [
            (properties.as_str(), Some(RuleEffect::Exclude)),
            ("/unworldly", None),
            ("/world", Some(RuleEffect::Exclude)),
            ("/world/house", Some(RuleEffect::Include)),
            ("/world/car", Some(RuleEffect::Exclude)),
            ("/world/car/hood", Some(RuleEffect::Exclude)),
            ("/world/car/driver", Some(RuleEffect::Include)),
            ("/world/car/driver/head", Some(RuleEffect::Exclude)),
        ] {
            assert_eq!(
                filter.most_specific_match(&EntityPath::from(path)),
                expected_effect,
                "path: {path:?}",
            );
        }

        assert_eq!(
            EntityPathFilter::parse_forgiving("/**")
                .resolve_forgiving(&subst_env)
                .formatted(),
            format!("+ /**\n- {properties}")
        );
    }

    #[test]
    fn test_entity_path_filter_subs() {
        // Make sure we use a string longer than `$origin` here.
        // We can't do in-place substitution.
        let subst_env = EntityPathSubs::new_with_origin(&EntityPath::from("/annoyingly/long/path"));

        let properties = format!("{}/**", EntityPath::properties());

        let filter = EntityPathFilter::parse_forgiving(format!(
            r#"
        + {properties}
        + $origin/**
        - $origin
        - $origin/car/**
        + $origin/car/driver
        "#
        ))
        .resolve_forgiving(&subst_env);

        for (path, expected_effect) in [
            (properties.as_str(), Some(RuleEffect::Include)),
            ("/unworldly", None),
            ("/annoyingly/long/path", Some(RuleEffect::Exclude)),
            ("/annoyingly/long/path/house", Some(RuleEffect::Include)),
            ("/annoyingly/long/path/car", Some(RuleEffect::Exclude)),
            ("/annoyingly/long/path/car/hood", Some(RuleEffect::Exclude)),
            (
                "/annoyingly/long/path/car/driver",
                Some(RuleEffect::Include),
            ),
            (
                "/annoyingly/long/path/car/driver/head",
                Some(RuleEffect::Exclude),
            ),
        ] {
            assert_eq!(
                filter.most_specific_match(&EntityPath::from(path)),
                expected_effect,
                "path: {path:?}",
            );
        }

        assert_eq!(
            EntityPathFilter::parse_forgiving("/**")
                .resolve_forgiving(&subst_env)
                .formatted(),
            format!("+ /**\n- {properties}")
        );
    }

    #[test]
    fn test_entity_path_filter_subtree() {
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /world/**
        - /world/car/**
        + /world/car/driver
        - /world/car/driver/head/**
        - /world/city
        - /world/houses/**
        "#,
        )
        .resolve_forgiving(&EntityPathSubs::empty());

        for (path, expected) in [
            ("/2D", false),
            ("/2D/image", false),
            ("/world", true),
            ("/world/car", true),
            ("/world/car/driver", true),
            ("/world/car/driver/head", false),
            ("/world/car/driver/head/ear", false),
            ("/world/city", true),
            ("/world/city/block", true),
            ("/world/houses", false),
            ("/world/houses/1", false),
            ("/world/houses/1/roof", false),
        ] {
            assert_eq!(
                filter.is_anything_in_subtree_included(&EntityPath::from(path)),
                expected,
                "path: {path:?}",
            );
        }
    }

    #[test]
    fn test_is_superset_of() {
        let subst_env = EntityPathSubs::empty();

        struct TestCase {
            filter: &'static str,
            contains: Vec<&'static str>,
            not_contains: Vec<&'static str>,
        }

        let cases = [
            TestCase {
                filter: "+ /**",
                contains: [
                    "",
                    "+ /**",
                    "+ /a/**",
                    r#"+ /a/**
                   + /b/c
                   + /b/d
                   "#,
                ]
                .into(),
                not_contains: [].into(),
            },
            TestCase {
                filter: "+ /a/**",
                contains: ["+ /a/**", "+ /a", "+ /a/b/**"].into(),
                not_contains: [
                    "+ /**",
                    "+ /b/**",
                    "+ /b",
                    r#"+ /a/b
                   + /b
                   "#,
                ]
                .into(),
            },
            TestCase {
                filter: r#"
                + /a
                + /b/c
                "#,
                contains: ["+ /a", "+ /b/c"].into(),
                not_contains: ["+ /a/**", "+ /b/**", "+ /b/c/d"].into(),
            },
            TestCase {
                filter: r#"
                + /**
                - /b/c
                "#,
                contains: ["+ /a", "+ /a/**", "+ /b", "+ /b/c/d"].into(),
                not_contains: ["+ /b/**", "+ /b/c"].into(),
            },
            TestCase {
                filter: r#"
                + /**
                - /b/c/**
                "#,
                contains: ["+ /a", "+ /a/**", "+ /b"].into(),
                not_contains: ["+ /b/**", "+ /b/c", "+ /b/c/d"].into(),
            },
        ];

        for case in &cases {
            let filter =
                EntityPathFilter::parse_forgiving(case.filter).resolve_forgiving(&subst_env);
            for contains in &case.contains {
                let contains_filter =
                    EntityPathFilter::parse_forgiving(contains).resolve_forgiving(&subst_env);
                assert!(
                    filter.is_superset_of(&contains_filter),
                    "Expected {:?} to fully contain {:?}, but it didn't",
                    filter.formatted(),
                    contains_filter.formatted(),
                );
            }
            for not_contains in &case.not_contains {
                let not_contains_filter =
                    EntityPathFilter::parse_forgiving(not_contains).resolve_forgiving(&subst_env);
                assert!(
                    !filter.is_superset_of(&not_contains_filter),
                    "Expected {:?} to NOT fully contain {:?}, but it did",
                    filter.formatted(),
                    not_contains_filter.formatted(),
                );
            }
        }
    }

    #[test]
    fn test_split_whitespace_smart() {
        assert_eq!(split_whitespace_smart("/world"), vec!["/world"]);
        assert_eq!(split_whitespace_smart("a b c"), vec!["a", "b", "c"]);
        assert_eq!(split_whitespace_smart("a\nb\tc  "), vec!["a", "b", "c"]);
        assert_eq!(split_whitespace_smart(r"a\ b c"), vec![r"a\ b", "c"]);

        assert_eq!(
            split_whitespace_smart(" + a - b + c"),
            vec!["+ a", "- b", "+ c"]
        );
        assert_eq!(
            split_whitespace_smart("+ a -\n b + c"),
            vec!["+ a", "-", "b", "+ c"]
        );
        assert_eq!(
            split_whitespace_smart("/weird/path- +/oth- erpath"),
            vec!["/weird/path-", "+/oth-", "erpath"]
        );
        assert_eq!(
            split_whitespace_smart(r"+world/** -/world/points"),
            vec!["+world/**", "-/world/points"]
        );
        assert_eq!(
            split_whitespace_smart(r"+ world/** - /world/points"),
            vec!["+ world/**", "- /world/points"]
        );
    }

    #[test]
    fn test_formatted() {
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        - /__properties/**
        "#,
        )
        .resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(filter.formatted_without_properties(), "+ /**");

        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        + /__properties/**
        "#,
        )
        .resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(
            filter.formatted_without_properties(),
            "+ /**\n+ /__properties/**"
        );
    }

    #[test]
    fn test_evaluate_filter() {
        let subst_env = EntityPathSubs::empty();

        // Test with a simple include-all filter
        let filter = EntityPathFilter::parse_forgiving("+ /**").resolve_forgiving(&subst_env);

        let eval = filter.evaluate(&EntityPath::from("/world"));
        assert!(eval.subtree_included, "/**should include /world subtree");
        assert!(eval.matches, "/**should match /world");
        assert!(!eval.matches_exactly, "/** doesn't exactly match /world");

        // Test with complex filter rules
        let filter = EntityPathFilter::parse_forgiving(
            r"
            + /world/**
            - /world/car/**
            + /world/car/driver
            ",
        )
        .resolve_forgiving(&subst_env);

        // Test /world - should be included
        let eval = filter.evaluate(&EntityPath::from("/world"));
        assert!(eval.subtree_included, "/world subtree should be included");
        assert!(eval.matches, "/world should match");
        assert!(
            !eval.matches_exactly,
            "/world should not match exactly (matched by /world/** subtree rule)"
        );

        // Test /world/house - should be included by /world/**
        let eval = filter.evaluate(&EntityPath::from("/world/house"));
        assert!(
            eval.subtree_included,
            "/world/house subtree should be included"
        );
        assert!(eval.matches, "/world/house should match");
        assert!(
            !eval.matches_exactly,
            "/world/house should not match exactly (matched by /world/** subtree rule)"
        );

        // Test /world/car - should not match but subtree should be included (has exception deeper)
        let eval = filter.evaluate(&EntityPath::from("/world/car"));
        assert!(
            eval.subtree_included,
            "/world/car subtree should be included (has /world/car/driver exception)"
        );
        assert!(!eval.matches, "/world/car should not match");
        assert!(
            !eval.matches_exactly,
            "/world/car should not match exactly (excluded by - /world/car/**)"
        );

        // Test /world/car/driver - should be included
        let eval = filter.evaluate(&EntityPath::from("/world/car/driver"));
        assert!(
            eval.subtree_included,
            "/world/car/driver subtree should be included"
        );
        assert!(eval.matches, "/world/car/driver should match");
        assert!(
            eval.matches_exactly,
            "/world/car/driver should match exactly (non-subtree rule)"
        );

        // Test /world/car/hood - should be excluded
        let eval = filter.evaluate(&EntityPath::from("/world/car/hood"));
        assert!(
            !eval.subtree_included,
            "/world/car/hood subtree should be excluded"
        );
        assert!(!eval.matches, "/world/car/hood should not match");
        assert!(
            !eval.matches_exactly,
            "/world/car/hood should not match exactly (excluded)"
        );

        // Test /other - should be excluded (no matching rule)
        let eval = filter.evaluate(&EntityPath::from("/other"));
        assert!(!eval.subtree_included, "/other subtree should be excluded");
        assert!(!eval.matches, "/other should not match");
        assert!(
            !eval.matches_exactly,
            "/other should not match exactly (no matching rule)"
        );

        // Test exact match without subtree
        let filter = EntityPathFilter::parse_forgiving(
            r"
            + /world
            + /world/car/driver
            ",
        )
        .resolve_forgiving(&subst_env);

        let eval = filter.evaluate(&EntityPath::from("/world"));
        assert!(eval.subtree_included, "/world should be included");
        assert!(eval.matches, "/world should match");
        assert!(
            eval.matches_exactly,
            "/world should match exactly (non-subtree rule)"
        );

        // Children should not be included (no subtree rule)
        let eval = filter.evaluate(&EntityPath::from("/world/house"));
        assert!(
            !eval.subtree_included,
            "/world/house should not be included (parent has no subtree rule)"
        );
        assert!(!eval.matches, "/world/house should not match");
        assert!(
            !eval.matches_exactly,
            "/world/house should not match exactly (no rule for this path)"
        );

        // Test subtree_included when there's an include rule deeper in the tree
        let filter = EntityPathFilter::parse_forgiving(
            r"
            + /world/car/driver
            ",
        )
        .resolve_forgiving(&subst_env);

        let eval = filter.evaluate(&EntityPath::from("/world"));
        assert!(
            eval.subtree_included,
            "/world should have subtree_included=true because /world/car/driver is included"
        );
        assert!(!eval.matches, "/world should not match directly");
        assert!(
            !eval.matches_exactly,
            "/world should not match exactly (no rule for this path)"
        );

        let eval = filter.evaluate(&EntityPath::from("/world/car"));
        assert!(
            eval.subtree_included,
            "/world/car should have subtree_included=true because /world/car/driver is included"
        );
        assert!(!eval.matches, "/world/car should not match directly");
        assert!(
            !eval.matches_exactly,
            "/world/car should not match exactly (no rule for this path)"
        );
    }

    #[test]
    fn test_unresolved() {
        // We should omit the properties from the unresolved filter.
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        - /__properties/**
        - /test123
        + /test345
        "#,
        );
        let resolved = filter.resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(
            resolved.unresolved(),
            EntityPathFilter::parse_forgiving(
                r#"
                + /**
                - /test123
                + /test345
                "#
            )
        );

        // If not explicitly mentioned, it should roundtrip.
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        - /test123
        + /test345
        "#,
        );
        let resolved = filter.resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(resolved.unresolved(), filter);

        // If the properties are _included_ they should be present.
        // We should omit the properties from the unresolved filter.
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        + /__properties/**
        - /test123
        + /test345
        "#,
        );
        let resolved = filter.resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(
            resolved.unresolved(),
            EntityPathFilter::parse_forgiving(
                r#"
                + /**
                + /__properties/**
                - /test123
                + /test345
                "#
            )
        );

        // If the subpaths of properties are _excluded_ they should be present.
        // We should omit the properties from the unresolved filter.
        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /**
        - /__properties/test123/**
        - /test123
        + /test345
        "#,
        );
        let resolved = filter.resolve_forgiving(&EntityPathSubs::empty());
        assert_eq!(
            resolved.unresolved(),
            EntityPathFilter::parse_forgiving(
                r#"
                + /**
                - /__properties/test123/**
                - /test123
                + /test345
                "#
            )
        );
    }
}
