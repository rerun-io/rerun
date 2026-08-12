use re_types_core::{Loggable as _, SerializedComponentBatch};

use crate::components::Text;

impl super::StateChange {
    /// Constructor for a single state value (one lane).
    pub fn single(state: impl Into<Text>) -> Self {
        Self::new().with_state([state])
    }

    /// Set the state array from optional values.
    ///
    /// A `None` entry resets that instance's state, showing a gap in its lane — something
    /// [`Self::with_state`] can't express, since it only takes present values.
    pub fn with_state_opt(
        mut self,
        state: impl IntoIterator<Item = Option<impl Into<Text>>>,
    ) -> Self {
        let res = Text::to_arrow_opt(
            state
                .into_iter()
                .map(|v| v.map(|v| std::borrow::Cow::Owned(v.into()))),
        );

        match res {
            Ok(array) => {
                self.state = Some(SerializedComponentBatch::new(
                    array,
                    Self::descriptor_state(),
                ));
            }

            #[cfg(debug_assertions)]
            Err(err) => {
                panic!(
                    "failed to serialize data for {}: {}",
                    Self::descriptor_state(),
                    re_error::format_ref(&err)
                )
            }

            #[cfg(not(debug_assertions))]
            Err(err) => {
                re_log::error!(
                    descriptor = %Self::descriptor_state(),
                    "failed to serialize data: {}",
                    re_error::format_ref(&err)
                );
            }
        }

        self
    }
}
