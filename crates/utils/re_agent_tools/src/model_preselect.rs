/// Which model a session selects before its first prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelPreselect {
    /// The model and effort the user picked last, or a fast model if they picked none.
    #[default]
    Previous,

    /// A model of this category, ignoring the model and effort the user picked.
    ///
    /// Applies to this session only: the user's picks stay saved for other sessions.
    Category(ModelCategory),
}

impl From<ModelCategory> for ModelPreselect {
    fn from(category: ModelCategory) -> Self {
        Self::Category(category)
    }
}

/// A kind of model, which each agent resolves to one of the models it offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelCategory {
    /// The cheapest capable model, for mechanical background work.
    ///
    /// The user should not pay flagship prices for a background task.
    VeryFast,

    /// Quick to answer, and capable enough for most work in the viewer.
    ///
    /// The agent panel should feel fast, and flagship models are often too slow for that.
    #[default]
    Fast,
}

impl ModelCategory {
    /// The ranked preferences that pick a model of this category, best first.
    pub fn preferences(self) -> Vec<ModelPreference> {
        let preferences: &[&[&str]] = match self {
            Self::VeryFast => &[
                &["haiku"],           // Anthropic: Claude
                &["luna"],            // OpenAI: Codex
                &["gemini", "flash"], // Google: Gemini
                &["gpt", "mini"],     // OpenAI: older GPT models
            ],
            Self::Fast => &[
                &["sonnet"],          // Anthropic: Claude
                &["terra"],           // OpenAI: Codex
                &["gemini", "flash"], // Google: Gemini
                &["gpt", "mini"],     // OpenAI: older GPT models
            ],
        };
        preferences
            .iter()
            .map(|tokens| ModelPreference::new(tokens))
            .collect()
    }
}

/// Defines a preferred model through multiple tokens that are expected to appear in the model name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPreference {
    tokens: Vec<String>,
}

impl ModelPreference {
    /// Creates a preference from explicit tokens. An empty preference matches nothing.
    pub fn new(tokens: &[&str]) -> Self {
        Self {
            tokens: tokens.iter().map(|token| (*token).to_owned()).collect(),
        }
    }

    /// All tokens must occur in this one ID or name; partial words do not match.
    /// Tokens are separated by non-ASCII-alphanumeric characters.
    pub fn matches(&self, model: &str) -> bool {
        !self.tokens.is_empty()
            && self.tokens.iter().all(|word| {
                !word.is_empty()
                    && model
                        .split(|c: char| !c.is_ascii_alphanumeric())
                        .any(|token| token.eq_ignore_ascii_case(word))
            })
    }
}
