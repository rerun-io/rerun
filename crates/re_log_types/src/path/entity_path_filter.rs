use std::collections::BTreeMap;

use crate::EntityPath;

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
    pub rules: BTreeMap<EntityPathRule, RuleEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityPathRule {
    pub path: EntityPath,

    /// If true, ALSO include children and grandchildren of this path (recursive rule).
    pub include_subtree: bool,
}

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
    pub fn parse_forgiving(rules: &str) -> Self {
        let mut filter = Self::default();

        for line in rules
            .split('\n')
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
        {
            let (effect, path_pattern) = match line.chars().next() {
                Some('+') => (RuleEffect::Include, &line[1..]),
                Some('-') => (RuleEffect::Exclude, &line[1..]),
                _ => (RuleEffect::Include, line),
            };

            let rule = EntityPathRule::parse_forgiving(path_pattern);

            filter.add_rule(effect, rule);
        }

        filter
    }

    pub fn add_rule(&mut self, effect: RuleEffect, rule: EntityPathRule) {
        self.rules.insert(rule, effect);
    }

    pub fn formatted(&self) -> String {
        let mut s = String::new();
        for (rule, effect) in &self.rules {
            s.push_str(match effect {
                RuleEffect::Include => "+ ",
                RuleEffect::Exclude => "- ",
            });
            if rule.path.is_root() && rule.include_subtree {
                // needs special casing, otherwise we end up with `//**`
                s.push_str("/**");
            } else {
                s.push_str(&rule.path.to_string());
                if rule.include_subtree {
                    s.push_str("/**");
                }
            }
            s.push('\n');
        }
        if s.ends_with('\n') {
            s.pop();
        }
        s
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

    pub fn is_exact_included(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, effect)| {
            effect == &RuleEffect::Include && !rule.include_subtree && rule.path == *entity_path
        })
    }

    pub fn add_exact(&mut self, clone: EntityPath) {
        self.rules
            .insert(EntityPathRule::exact(clone), RuleEffect::Include);
    }

    pub fn add_subtree(&mut self, clone: EntityPath) {
        self.rules.insert(
            EntityPathRule::including_subtree(clone),
            RuleEffect::Include,
        );
    }

    pub fn contains_rule_for_exactly(&self, entity_path: &EntityPath) -> bool {
        self.rules.iter().any(|(rule, _)| rule.path == *entity_path)
    }

    pub fn is_explicitly_excluded(&self, entity_path: &EntityPath) -> bool {
        self.rules
            .iter()
            .any(|(rule, effect)| rule.path == *entity_path && effect == &RuleEffect::Exclude)
    }

    pub fn is_explicitly_included(&self, entity_path: &EntityPath) -> bool {
        self.rules
            .iter()
            .any(|(rule, effect)| rule.path == *entity_path && effect == &RuleEffect::Include)
    }
}

impl EntityPathRule {
    #[inline]
    pub fn exact(path: EntityPath) -> Self {
        Self {
            path,
            include_subtree: false,
        }
    }

    #[inline]
    pub fn including_subtree(path: EntityPath) -> Self {
        Self {
            path,
            include_subtree: true,
        }
    }

    pub fn parse_forgiving(expression: &str) -> Self {
        let expression = expression.trim();
        if let Some(path) = expression.strip_suffix("/**") {
            Self {
                path: EntityPath::parse_forgiving(path),
                include_subtree: true,
            }
        } else {
            Self {
                path: EntityPath::parse_forgiving(expression),
                include_subtree: false,
            }
        }
    }

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
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.path, !self.include_subtree).cmp(&(&other.path, !other.include_subtree))
    }
}

impl std::cmp::PartialOrd for EntityPathRule {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[test]
fn test_rule_order() {
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
    let rules = rules.map(EntityPathRule::parse_forgiving);
    check_total_order(&rules);
}

#[test]
fn test_entity_path_filter() {
    let filter = EntityPathFilter::parse_forgiving(
        r#"
        + /world/**
        - /world/
        - /world/car/**
        + /world/car/driver
        "#,
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
        EntityPathFilter::parse_forgiving("/**").formatted(),
        "+ /**"
    );
}
