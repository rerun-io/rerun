//! Generic fuzzy matching, used e.g. by the command palette.
//!
//! [`FuzzyQuery`] parses what the user typed, and [`FuzzyMatch`] describes how well
//! a candidate string matched it, including which characters to highlight.

use std::collections::BTreeSet;

use egui::Color32;
use egui::text::{ByteIndex, LayoutJob, TextFormat};
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32String};

use crate::egui_ext::LayoutJobExt as _;

pub struct FuzzyQuery {
    raw_query: String,

    /// The parsed query: each whitespace-separated word must match (in any order).
    pattern: Pattern,

    /// The matcher owns scratch buffers that we want to reuse between matches.
    /// `Mutex` so that [`Self::try_match`] can stay `&self`.
    matcher: parking_lot::Mutex<Matcher>,
}

impl FuzzyQuery {
    pub fn new(raw_query: String) -> Self {
        Self {
            pattern: Pattern::new(
                raw_query.trim(),
                CaseMatching::Smart,
                Normalization::Smart,
                AtomKind::Fuzzy,
            ),
            raw_query,
            matcher: parking_lot::Mutex::new(Matcher::new(nucleo_matcher::Config::DEFAULT)),
        }
    }

    /// Returns exactly what the user entered
    pub fn raw_query(&self) -> &str {
        &self.raw_query
    }

    /// How well does this query match the target text (if at all)?
    pub fn try_match(&self, target: String) -> Option<FuzzyMatch> {
        re_tracing::profile_function!();

        let haystack = Utf32String::from(target.as_str());
        let mut matched_char_indices = Vec::new();
        let score = self.pattern.indices(
            haystack.slice(..),
            &mut self.matcher.lock(),
            &mut matched_char_indices,
        )?;

        // With a multi-word query the indices can be unsorted and contain duplicates,
        // but `BTreeSet` takes care of both:
        Some(FuzzyMatch {
            target,
            matched_char_indices: matched_char_indices
                .into_iter()
                .map(|char_idx| char_idx as usize)
                .collect(),
            score: i64::from(score),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.raw_query.trim().is_empty()
    }
}

/// How well a [`FuzzyQuery`] matches a target string.
pub struct FuzzyMatch {
    /// What we matched on, e.g. `command.to_string()`
    target: String,

    /// Which characters (by `char` index) of the target got matched.
    matched_char_indices: BTreeSet<usize>,

    /// How well we matched (higher = better)
    score: i64,
}

impl FuzzyMatch {
    /// Indicated the lowest possible score
    pub fn lowest(target: String) -> Self {
        Self {
            target,
            matched_char_indices: BTreeSet::new(),
            score: i64::MIN,
        }
    }

    /// Indicated the highest possible score
    pub fn highest(target: String) -> Self {
        Self {
            target,
            matched_char_indices: BTreeSet::new(),
            score: i64::MAX,
        }
    }

    /// How well did the [`FuzzyQuery`] match the text?
    ///
    /// Higher = better match.
    pub fn score(&self) -> i64 {
        self.score
    }

    /// What we matched on, e.g. `command.to_string()`
    pub fn target(&self) -> &str {
        &self.target
    }

    /// How we highlight a matching character.
    ///
    /// `selected` is whether the text is on a selection background.
    fn highlight_format(style: &egui::Style, selected: bool, format: &mut TextFormat) {
        // Disregard current text color (might be a syntax highlight color):
        format.color = if selected {
            style.visuals.selection.stroke.color
        } else {
            style.visuals.strong_text_color()
        };

        // Make the color stronger:
        format.color = if format.color.intensity() > 0.5 {
            Color32::WHITE
        } else {
            Color32::BLACK
        };

        format.underline = egui::Stroke::new(1.0, format.color);
    }

    /// Format the target text, highlighting the matching characters.
    ///
    /// `selected` is whether the text is on a selection background.
    pub fn widget_text(
        &self,
        style: &egui::Style,
        font_id: &egui::FontId,
        text_color: egui::Color32,
        selected: bool,
    ) -> egui::WidgetText {
        if self.matched_char_indices.is_empty() {
            egui::RichText::new(&self.target).color(text_color).into()
        } else {
            let mut job = LayoutJob::default();
            for (char_idx, chr) in self.target.chars().enumerate() {
                let mut format = TextFormat::simple(font_id.clone(), text_color);
                if self.matched_char_indices.contains(&char_idx) {
                    Self::highlight_format(style, selected, &mut format);
                }
                job.append(&chr.to_string(), 0.0, format);
            }

            job.into()
        }
    }

    /// Highlight each letter that matches what the user inputted.
    ///
    /// This can be used on text that has been syntax-highlighted, for instance.
    ///
    /// `selected` is whether the text is on a selection background.
    pub fn highlight_matching_text(
        &self,
        style: &egui::Style,
        input_job: &LayoutJob,
        selected: bool,
    ) -> LayoutJob {
        if self.matched_char_indices.is_empty() {
            return input_job.clone();
        }

        re_log::debug_assert_eq!(
            self.target,
            input_job.text,
            "Different text in the input layout job vs what we matched on. Maybe a difference in how we format things vs how we syntax highlight them?"
        );

        // Break up the job into one-character pieces, and then highlight those:

        let mut out_job = input_job.cleared();
        let mut char_byte_idx = 0;
        for (char_idx, chr) in self.target.chars().enumerate() {
            let mut format = input_job.format_at_byte(ByteIndex(char_byte_idx)).clone();
            if self.matched_char_indices.contains(&char_idx) {
                Self::highlight_format(style, selected, &mut format);
            }
            out_job.append(&chr.to_string(), 0.0, format);
            char_byte_idx += chr.len_utf8();
        }

        out_job
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matcher reports `char` indices, and the highlighting code indexes by `char` —
    /// multi-byte characters before the match must not shift the highlight.
    #[test]
    fn fuzzy_match_indices_are_char_indices() {
        let query = FuzzyQuery::new("bar".to_owned());
        let fuzzy_match = query
            .try_match("café_bar".to_owned())
            .expect("should match");

        // With byte indices the `é` (two bytes) would shift this to {6, 7, 8}:
        assert_eq!(
            fuzzy_match.matched_char_indices,
            BTreeSet::from([5, 6, 7]),
            "expected the char indices of 'bar'"
        );

        let query = FuzzyQuery::new("rocket".to_owned());
        let fuzzy_match = query
            .try_match("🚀 rocket".to_owned())
            .expect("should match");

        // The emoji is one char but four bytes:
        assert_eq!(
            fuzzy_match.matched_char_indices,
            (2..8).collect::<BTreeSet<usize>>(),
            "expected the char indices of 'rocket'"
        );
    }

    /// The highlighted layout job must slice the target at `char` boundaries,
    /// underlining exactly the matched characters.
    #[test]
    fn highlight_matching_text_handles_multi_byte_chars() {
        let target = "café_bar".to_owned();
        let query = FuzzyQuery::new("bar".to_owned());
        let fuzzy_match = query.try_match(target.clone()).expect("should match");

        let style = egui::Style::default();
        let input_job = LayoutJob::simple(
            target.clone(),
            egui::FontId::default(),
            Color32::GRAY,
            f32::INFINITY,
        );
        let highlighted_job = fuzzy_match.highlight_matching_text(&style, &input_job, false);

        assert_eq!(highlighted_job.text, target);

        // Slicing by `byte_range` panics if a section starts or ends inside the `é`:
        let underlined: String = highlighted_job
            .sections
            .iter()
            .filter(|section| section.format.underline != egui::Stroke::NONE)
            .map(|section| {
                &highlighted_job.text[section.byte_range.start.0..section.byte_range.end.0]
            })
            .collect();
        assert_eq!(underlined, "bar");
    }

    /// A lowercase query should ignore case (smart-case),
    /// and an ASCII query should match accented characters.
    #[test]
    fn fuzzy_match_ignores_case_and_accents() {
        let query = FuzzyQuery::new("cafe".to_owned());
        let fuzzy_match = query
            .try_match("Café Racer".to_owned())
            .expect("should match despite case and accent differences");

        assert_eq!(
            fuzzy_match.matched_char_indices,
            BTreeSet::from([0, 1, 2, 3]),
            "expected the char indices of 'Café'"
        );
    }
}
