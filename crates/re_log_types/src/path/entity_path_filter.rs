use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;

use crate::EntityPath;

/// A set of substitutions for entity paths.
#[derive(Default)]
pub struct EntityPathSubs(pub HashMap<String, String>);

impl EntityPathSubs {
    /// Create a new set of substitutions from a single origin.
    pub fn new_with_origin(origin: &EntityPath) -> Self {
        Self(std::iter::once(("origin".to_owned(), origin.to_string())).collect())
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
/// `EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
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
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EntityPathFilter {
    rules: BTreeMap<EntityPathRule, RuleEffect>,
}

impl std::hash::Hash for EntityPathFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.formatted().hash(state);
    }
}

impl std::fmt::Debug for EntityPathFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityPathFilter({:?})", self.formatted())
    }
}

#[derive(Clone, Debug)]
pub struct EntityPathRule {
    // We need to store the raw expression to be able to round-trip the filter
    // when it contains substitutions.
    pub raw_expression: String,

    pub path: EntityPath,

    /// If true, ALSO include children and grandchildren of this path (recursive rule).
    pub include_subtree: bool,
}

impl PartialEq for EntityPathRule {
    fn eq(&self, other: &Self) -> bool {
        self.raw_expression == other.raw_expression
    }
}

impl Eq for EntityPathRule {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleEffect {
    Include,
    Exclude,
}

impl std::ops::AddAssign for EntityPathFilter {
    /// The union of all rules
    #[inline]
    fn add_assign(&mut self, mut rhs: Self) {
        self.rules.append(&mut rhs.rules);
    }
}

impl std::iter::Sum for EntityPathFilter {
    /// The union of all rules
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = EntityPathFilter::default();
        for item in iter {
            sum += item;
        }
        sum
    }
}

impl EntityPathFilter {
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
    /// The rest of the line is trimmed and treated as an entity path.
    ///
    /// Conflicting rules are resolved by the last rule.
    pub fn parse_forgiving(rules: &str, subst_env: &EntityPathSubs) -> Self {
        Self::from_query_expressions(rules.split('\n'), subst_env)
    }

    /// Build a filter from a list of query expressions.
    ///
    /// Each item in the iterator should be a query expression.
    ///
    /// The first character should be `+` or `-`. If missing, `+` is assumed.
    /// The rest of the expression is trimmed and treated as an entity path.
    ///
    /// Conflicting rules are resolved by the last rule.
    pub fn from_query_expressions<'a>(
        rules: impl IntoIterator<Item = &'a str>,
        subst_env: &EntityPathSubs,
    ) -> Self {
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

            let rule = EntityPathRule::parse_forgiving(path_pattern, subst_env);

            filter.add_rule(effect, rule);
        }

        filter
    }

    /// Creates a new entity path filter that includes only a single entity.
    pub fn single_entity_filter(entity_path: &EntityPath) -> Self {
        let mut filter = Self::default();
        filter.add_exact(entity_path.clone());
        filter
    }

    /// Creates a new entity path filter that includes a single subtree.
    pub fn subtree_entity_filter(entity_path: &EntityPath) -> Self {
        let mut filter = Self::default();
        filter.add_subtree(entity_path.clone());
        filter
    }

    pub fn add_rule(&mut self, effect: RuleEffect, rule: EntityPathRule) {
        self.rules.insert(rule, effect);
    }

    pub fn iter_expressions(&self) -> impl Iterator<Item = String> + '_ {
        self.rules.iter().map(|(rule, effect)| {
            let mut s = String::new();
            s.push_str(match effect {
                RuleEffect::Include => "+ ",
                RuleEffect::Exclude => "- ",
            });
            s.push_str(&rule.raw_expression);
            s
        })
    }

    pub fn formatted(&self) -> String {
        self.iter_expressions().join("\n")
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

    pub fn is_included(&self, path: &EntityPath) -> bool {
        let effect = self
            .most_specific_match(path)
            .unwrap_or(RuleEffect::Exclude);
        match effect {
            RuleEffect::Include => true,
            RuleEffect::Exclude => false,
        }
    }

    /// Is there a rule for this exact entity path (ignoring subtree)?
    pub fn is_exact_included(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, effect)| {
            effect == &RuleEffect::Include && !rule.include_subtree && rule.path == *entity_path
        })
    }

    /// Include this entity, but not the subtree.
    pub fn add_exact(&mut self, clone: EntityPath) {
        self.rules
            .insert(EntityPathRule::exact(clone), RuleEffect::Include);
    }

    /// Include this entity with subtree.
    pub fn add_subtree(&mut self, clone: EntityPath) {
        self.rules.insert(
            EntityPathRule::including_subtree(clone),
            RuleEffect::Include,
        );
    }

    /// Remove any rule for the given entity path (ignoring whether or not that rule includes the subtree).
    pub fn remove_rule_for(&mut self, entity_path: &EntityPath) {
        self.rules.retain(|rule, _| rule.path != *entity_path);
    }

    /// Is there any rule for this entity path?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn contains_rule_for_exactly(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, _)| rule.path == *entity_path)
    }

    /// Is this entity path explicitly included?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn is_explicitly_included(&self, entity_path: &EntityPath) -> bool {
        self.rules
            .iter()
            .any(|(rule, effect)| rule.path == *entity_path && effect == &RuleEffect::Include)
    }

    /// Is this entity path explicitly excluded?
    ///
    /// Whether or not the subtree is included is NOT important.
    pub fn is_explicitly_excluded(&self, entity_path: &EntityPath) -> bool {
        self.rules
            .iter()
            .any(|(rule, effect)| rule.path == *entity_path && effect == &RuleEffect::Exclude)
    }

    /// Is anything under this path included (including self)?
    pub fn is_anything_in_subtree_included(&self, path: &EntityPath) -> bool {
        for (rule, effect) in &self.rules {
            if effect == &RuleEffect::Include && rule.path.starts_with(path) {
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
                        if rule.include_subtree {
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
    /// If this returns `true` there should not exist any [`EntityPath`] for which [`Self::is_included`]
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
                        .find(|(r, _)| r.matches(&other_rule.path))
                    {
                        match self_effect {
                            RuleEffect::Include => {
                                // If the other rule includes the subtree, but the matching
                                // rule doesn't, then we don't fully contain the other rule.
                                if other_rule.include_subtree && !self_rule.include_subtree {
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
                        .find(|(r, _)| r.matches(&self_rule.path))
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
}

impl EntityPathRule {
    /// Match this path, but not children.
    #[inline]
    pub fn exact(path: EntityPath) -> Self {
        Self {
            raw_expression: path.to_string(),
            path,
            include_subtree: false,
        }
    }

    /// Match this path and any entity in its subtree.
    #[inline]
    pub fn including_subtree(path: EntityPath) -> Self {
        Self {
            raw_expression: format!("{path}/**",),
            path,
            include_subtree: true,
        }
    }

    pub fn parse_forgiving(expression: &str, subst_env: &EntityPathSubs) -> Self {
        let raw_expression = expression.trim().to_owned();

        // TODO(#5528): This is a very naive implementation of variable substitution.
        // unclear if we want to do this here, push this down into `EntityPath::parse`,
        // or even supported deferred evaluation on the `EntityPath` itself.
        let mut expression_sub = raw_expression.clone();
        for (key, value) in &subst_env.0 {
            expression_sub = expression_sub.replace(format!("${key}").as_str(), value);
            expression_sub = expression_sub.replace(format!("${{{key}}}").as_str(), value);
        }

        if expression == "/**" {
            Self {
                raw_expression,
                path: EntityPath::root(),
                include_subtree: true,
            }
        } else if let Some(path) = expression_sub.strip_suffix("/**") {
            Self {
                raw_expression,
                path: EntityPath::parse_forgiving(path),
                include_subtree: true,
            }
        } else {
            Self {
                raw_expression,
                path: EntityPath::parse_forgiving(&expression_sub),
                include_subtree: false,
            }
        }
    }

    #[inline]
    pub fn matches(&self, path: &EntityPath) -> bool {
        if self.include_subtree {
            path.starts_with(&self.path)
        } else {
            path == &self.path
        }
    }
}

impl std::cmp::Ord for EntityPathRule {
    /// Most specific last, which means recursive first.
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.path, !self.include_subtree).cmp(&(&other.path, !other.include_subtree))
    }
}

impl std::cmp::PartialOrd for EntityPathRule {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use crate::{EntityPath, EntityPathFilter, EntityPathRule, EntityPathSubs, RuleEffect};

    #[test]
    fn test_rule_order() {
        let subst_env = Default::default();

        use std::cmp::Ordering;

        fn check_total_order(rules: &[EntityPathRule]) {
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
        let rules = rules.map(|rule| EntityPathRule::parse_forgiving(rule, &subst_env));
        check_total_order(&rules);
    }

    #[test]
    fn test_entity_path_filter() {
        let subst_env = Default::default();

        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /world/**
        - /world/
        - /world/car/**
        + /world/car/driver
        "#,
            &subst_env,
        );

        for (path, expected_effect) in [
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
            EntityPathFilter::parse_forgiving("/**", &subst_env).formatted(),
            "+ /**"
        );
    }

    #[test]
    fn test_entity_path_filter_subs() {
        // Make sure we use a string longer than `$origin` here.
        // We can't do in-place substitution.
        let subst_env = EntityPathSubs::new_with_origin(&EntityPath::from("/annoyingly/long/path"));

        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + $origin/**
        - $origin
        - $origin/car/**
        + $origin/car/driver
        "#,
            &subst_env,
        );

        for (path, expected_effect) in [
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
            EntityPathFilter::parse_forgiving("/**", &subst_env).formatted(),
            "+ /**"
        );
    }

    #[test]
    fn test_entity_path_filter_subtree() {
        let subst_env = Default::default();

        let filter = EntityPathFilter::parse_forgiving(
            r#"
        + /world/**
        - /world/car/**
        + /world/car/driver
        - /world/car/driver/head/**
        - /world/city
        - /world/houses/**
        "#,
            &subst_env,
        );

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
        let subst_env = Default::default();

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
            let filter = EntityPathFilter::parse_forgiving(case.filter, &subst_env);
            for contains in &case.contains {
                let contains_filter = EntityPathFilter::parse_forgiving(contains, &subst_env);
                assert!(
                    filter.is_superset_of(&contains_filter),
                    "Expected {:?} to fully contain {:?}, but it didn't",
                    filter.formatted(),
                    contains_filter.formatted(),
                );
            }
            for not_contains in &case.not_contains {
                let not_contains_filter =
                    EntityPathFilter::parse_forgiving(not_contains, &subst_env);
                assert!(
                    !filter.is_superset_of(&not_contains_filter),
                    "Expected {:?} to NOT fully contain {:?}, but it did",
                    filter.formatted(),
                    not_contains_filter.formatted(),
                );
            }
        }
    }
}
