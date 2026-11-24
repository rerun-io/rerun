use agent_client_protocol_schema::v1::SessionConfigSelectOption;
use re_agent_tools::ModelPreference;

/// The best offered model according to the ordered preferences.
///
/// Among matches for one preference, newer recognizable versions come first.
/// An unversioned first match is retained. Otherwise, unversioned candidates are
/// skipped and equal versions retain the agent's order.
pub fn preferred_model<'a>(
    models: &[&'a SessionConfigSelectOption],
    preferences: &[ModelPreference],
) -> Option<&'a SessionConfigSelectOption> {
    preferences.iter().find_map(|preference| {
        let matching = models
            .iter()
            .copied()
            .filter(|model| {
                // Agents may expose the model family in either the machine ID or the display name.
                preference.matches(&model.value.0) || preference.matches(&model.name)
            })
            .map(|model| {
                (
                    model,
                    ModelVersion::parse(&model.name)
                        .or_else(|| ModelVersion::parse(&model.value.0)),
                )
            });
        // We may have multiple matches; try to pick the newest version.
        newest(matching)
    })
}

/// The best model ID according to the ordered preferences.
pub fn preferred_model_id<'a>(
    models: &[&'a str],
    preferences: &[ModelPreference],
) -> Option<&'a str> {
    preferences.iter().find_map(|preference| {
        newest(
            models
                .iter()
                .copied()
                .filter(|model| preference.matches(model))
                .map(|model| (model, ModelVersion::parse(model))),
        )
    })
}

/// Replaces the current winner only when both versions are known and the candidate is newer.
/// An unversioned first candidate therefore remains the winner.
fn newest<T>(models: impl Iterator<Item = (T, Option<ModelVersion>)>) -> Option<T> {
    models
        .reduce(|best, candidate| match (&best.1, &candidate.1) {
            (Some(a), Some(b)) if b > a => candidate,
            _ => best,
        })
        .map(|(model, _)| model)
}

/// Numeric version components ordered lexicographically, with trailing zeroes normalized away.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ModelVersion(Vec<u32>);

impl ModelVersion {
    /// Extracts a single dotted numeric version, rejecting ambiguous or date-like model names.
    ///
    /// Trailing zero components are removed so versions such as `6` and `6.0` compare equally.
    fn parse(model: &str) -> Option<Self> {
        let mut numbers = model
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
            .filter(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()));
        let number = numbers.next()?;
        if numbers.next().is_some() {
            return None;
        }
        let mut parts = number
            .split('.')
            .map(|part| {
                if part.is_empty() || part.len() > 2 {
                    return None;
                }
                part.parse::<u32>().ok()
            })
            .collect::<Option<Vec<_>>>()?;
        while parts.len() > 1 && parts.last() == Some(&0) {
            parts.pop();
        }
        Some(Self(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Matching accepts model IDs and display names without matching inside unrelated words.
    #[test]
    fn model_preferences_match_model_tokens() {
        // Model names and IDs match when they contain all preference tokens.
        let gemini_flash = ModelPreference::new(&["gemini", "flash"]);
        let gpt_mini = ModelPreference::new(&["gpt", "mini"]);
        assert!(gemini_flash.matches("Gemini 2.5 Flash"));
        assert!(gemini_flash.matches("gemini-2.5-flash"));
        assert!(gemini_flash.matches("Flash Gemini"));
        assert!(gpt_mini.matches("gpt-4o-mini"));
        assert!(gpt_mini.matches("GPT-4.1 Mini"));

        // Missing, partial, or empty tokens must not produce a match.
        assert!(!gemini_flash.matches("GLM-4.7 Flash"));
        assert!(!gemini_flash.matches("Gemini 2.5 Pro"));
        assert!(!gpt_mini.matches("Gemini 2.5 Pro"));
        assert!(!gpt_mini.matches("other-mini"));
        assert!(!ModelPreference::new(&["mini"]).matches("Gemini 2.5 Pro"));
        assert!(!ModelPreference::new(&["sonnet"]).matches("claude-sonnetish"));
        assert!(!ModelPreference::new(&["luna"]).matches("lunar"));
        assert!(!ModelPreference::new(&[]).matches("gpt-4o-mini"));
        assert!(!ModelPreference::new(&[""]).matches("gpt-4o-mini"));
        assert!(!ModelPreference::new(&["gpt", ""]).matches("gpt-4o-mini"));
    }

    /// Family preferences take priority over versions and exclude unrelated models.
    #[test]
    fn preference_order_precedes_version_order() {
        let sonnet = SessionConfigSelectOption::new("sonnet", "Claude Sonnet");
        let terra = SessionConfigSelectOption::new("gpt-6-terra", "GPT-6 Terra");
        let gemini = SessionConfigSelectOption::new("gemini-2.5-flash", "Gemini 2.5 Flash");
        assert_eq!(
            preferred_model(
                &[&gemini, &terra, &sonnet],
                &crate::ModelCategory::Fast.preferences()
            )
            .map(|model| &model.value),
            Some(&sonnet.value)
        );
        let glm = SessionConfigSelectOption::new("glm-flash", "GLM-4.7 Flash");
        let gemini_pro = SessionConfigSelectOption::new("gemini-2.5-pro", "Gemini 2.5 Pro");
        let gpt_mini = SessionConfigSelectOption::new("gpt-4o-mini", "GPT-4o Mini");
        assert_eq!(
            preferred_model(&[&glm, &gemini], &crate::ModelCategory::Fast.preferences())
                .map(|model| &model.value),
            Some(&gemini.value)
        );
        assert_eq!(
            preferred_model(
                &[&gemini_pro, &gpt_mini],
                &crate::ModelCategory::Fast.preferences()
            )
            .map(|model| &model.value),
            Some(&gpt_mini.value)
        );
        assert!(
            preferred_model(
                &[&glm, &gemini_pro],
                &crate::ModelCategory::Fast.preferences()
            )
            .is_none()
        );
    }

    /// Versions compare numerically while same-version variants retain their offered order.
    #[test]
    fn newer_versions_win_but_ties_keep_agent_order() {
        let old = SessionConfigSelectOption::new("5.6", "openai/GPT-5.6 Terra");
        let fast = SessionConfigSelectOption::new("fast", "openai/GPT-6.1 Terra Fast");
        let newest = SessionConfigSelectOption::new("6.10", "openai/GPT-6.10 Terra");
        let regular = SessionConfigSelectOption::new("regular", "openai/GPT-6.1 Terra");
        let preferences = [ModelPreference::new(&["terra"])];
        assert_eq!(
            preferred_model(&[&old, &fast, &regular], &preferences).map(|model| &model.value),
            Some(&fast.value)
        );
        assert_eq!(
            preferred_model(&[&old, &newest, &regular], &preferences).map(|model| &model.value),
            Some(&newest.value)
        );
        assert_eq!(
            preferred_model_id(&["gpt-6.9-terra", "gpt-6.10-terra"], &preferences),
            Some("gpt-6.10-terra")
        );
    }

    /// Unknown versions never replace a winner or allow an unversioned first match to be replaced.
    #[test]
    fn ambiguous_versions_keep_agent_order() {
        let alias = SessionConfigSelectOption::new("default", "Terra");
        let newer = SessionConfigSelectOption::new("newer", "GPT-6 Terra");
        let preferences = [ModelPreference::new(&["terra"])];
        assert_eq!(
            preferred_model(&[&alias, &newer], &preferences).map(|model| &model.value),
            Some(&alias.value)
        );
        let older = SessionConfigSelectOption::new("older", "GPT-5.6 Terra");
        assert_eq!(
            preferred_model(&[&older, &alias, &newer], &preferences).map(|model| &model.value),
            Some(&newer.value)
        );
        assert_eq!(ModelVersion::parse("claude-sonnet-4-5-20250929"), None);
        assert_eq!(
            ModelVersion::parse("GPT-6.0 Terra"),
            ModelVersion::parse("GPT-6 Terra")
        );
    }
}
