use std::ops::Range;

use egui::{Color32, NumExt as _, Widget as _};
use itertools::Itertools;

use re_entity_db::external::re_chunk_store::external::re_chunk::external::nohash_hasher::IntMap;

use crate::{list_item, UiExt as _};

/// State for the filter widget when it is toggled on.
#[derive(Debug, Clone)]
struct InnerState {
    /// The filter query string.
    filter_query: String,

    /// This ID is recreated every time the filter is toggled and tracks the current filtering
    /// session.
    ///
    /// This can be useful for client code to store session-specific state (e.g., the state of tree
    /// collapsed-ness).
    session_id: egui::Id,
}

impl Default for InnerState {
    fn default() -> Self {
        let mut random_bytes = [0u8; 8];
        getrandom::getrandom(&mut random_bytes).expect("Couldn't get random bytes");

        Self {
            filter_query: String::new(),

            // create a new session id each time the filter is toggled
            session_id: egui::Id::new(random_bytes),
        }
    }
}

/// State and UI for the filter widget.
///
/// The filter widget is designed as a toggle between a title widget and the filter text field.
/// [`Self`] is responsible for storing the widget state as well as the query text typed by the
/// user. [`FilterMatcher`] performs the actual filtering.
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    /// The current state of the filter widget.
    ///
    /// This is `None` when the filter is not active.
    inner_state: Option<InnerState>,

    /// Should the text field be focused?
    ///
    /// Set to `true` upon clicking on the search button.
    request_focus: bool,
}

impl FilterState {
    /// Activate the filter.
    ///
    /// This is the same as clicking the "loupe" icon button.
    pub fn activate(&mut self, query: &str) {
        self.inner_state = Some(InnerState {
            filter_query: query.to_owned(),
            ..Default::default()
        });
        self.request_focus = true;
    }

    /// Is the filter currently active?
    pub fn is_active(&self) -> bool {
        self.inner_state.is_some()
    }

    /// Return the filter if any.
    ///
    /// Returns `None` if the filter is disabled. Returns `Some(query)` if the filter is enabled
    /// (even if the query string is empty, in which case it should match nothing).
    pub fn query(&self) -> Option<&str> {
        self.inner_state
            .as_ref()
            .map(|state| state.filter_query.as_str())
    }

    /// Return the current session ID of the filter widget, if active.
    pub fn session_id(&self) -> Option<egui::Id> {
        self.inner_state.as_ref().map(|state| state.session_id)
    }

    /// Return a filter matcher for the current query.
    pub fn filter(&self) -> FilterMatcher {
        FilterMatcher::new(self.query())
    }

    /// Display the filter widget.
    ///
    /// Note: this uses [`egui::Ui::available_width`] to determine the location of the right-aligned
    /// search button, as usual for [`list_item::ListItem`]-based widgets.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        section_title: impl Into<egui::WidgetText>,
    ) -> Option<egui::Response> {
        let mut button_clicked = false;

        let icon = if self.inner_state.is_none() {
            &crate::icons::SEARCH
        } else {
            &crate::icons::CLOSE
        };

        // precompute the title layout such that we know the size we need for the list item content
        let section_title = section_title.into();
        let galley = section_title.into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::FontSelection::default(),
        );
        let text_width = galley.size().x;

        let mut title_response = None;

        list_item::list_item_scope(ui, ui.next_auto_id(), |ui| {
            ui.list_item().interactive(false).show_flat(
                ui,
                list_item::CustomContent::new(|ui, _| {
                    if self.inner_state.is_some()
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                    {
                        self.inner_state = None;
                    }

                    if let Some(inner_state) = self.inner_state.as_mut() {
                        // we add additional spacing for aesthetic reasons (active text edits have a
                        // fat border)
                        ui.spacing_mut().text_edit_width =
                            (ui.max_rect().width() - 10.0).at_least(0.0);

                        // TODO(ab): ideally _all_ text edits would be styled this way, but we
                        // require egui support for that (https://github.com/emilk/egui/issues/3284)
                        ui.visuals_mut().widgets.hovered.expansion = 0.0;
                        ui.visuals_mut().widgets.active.expansion = 0.0;
                        ui.visuals_mut().widgets.open.expansion = 0.0;
                        ui.visuals_mut().widgets.active.fg_stroke.width = 1.0;
                        ui.visuals_mut().selection.stroke.width = 1.0;

                        let response = egui::TextEdit::singleline(&mut inner_state.filter_query)
                            .lock_focus(true)
                            .ui(ui);

                        if self.request_focus {
                            self.request_focus = false;
                            response.request_focus();
                        }
                    } else {
                        title_response = Some(ui.label(galley));
                    }
                })
                .with_content_width(text_width)
                .action_button(icon, || {
                    button_clicked = true;
                }),
            );
        });

        // defer button handling because we can't mutably borrow `self` in both closures above
        if button_clicked {
            if self.inner_state.is_none() {
                self.activate("");
            } else {
                self.inner_state = None;
            }
        }

        title_response
    }
}

// --

/// Full-text, case-insensitive matcher.
pub struct FilterMatcher {
    /// Lowercase keywords.
    ///
    /// If this is `None`, the filter is inactive and the matcher will accept everything. If this
    /// is `Some([])`, the matcher will reject any input.
    keywords: Option<Vec<Keyword>>,
}

impl FilterMatcher {
    fn new(query: Option<&str>) -> Self {
        Self {
            keywords: query.map(|s| s.split_whitespace().map(Keyword::new).collect()),
        }
    }

    /// Is the filter currently active?
    pub fn is_active(&self) -> bool {
        self.keywords.is_some()
    }

    /// Is the filter set to match everything?
    ///
    /// Can be used by client code to short-circuit more expansive matching logic.
    pub fn matches_everything(&self) -> bool {
        self.keywords.is_none()
    }

    /// Is the filter set to match nothing?
    ///
    /// Can be used by client code to short-circuit more expansive matching logic.
    pub fn matches_nothing(&self) -> bool {
        self.keywords
            .as_ref()
            .is_some_and(|keywords| keywords.is_empty())
    }

    // /// Does the given text match the filter?
    // pub fn matches(&self, text: &str) -> bool {
    //     match self.keywords.as_deref() {
    //         None => true,
    //         Some([]) => false,
    //         Some(keywords) => {
    //             let lowercase_input = text.to_lowercase();
    //             keywords
    //                 .iter()
    //                 .all(|keyword| lowercase_input.contains(keyword))
    //         }
    //     }
    // }

    // /// Does the given hierarchy match the filter?
    // ///
    // /// To match, each of the keyword must be present in at least one of the parts of the hierarchy.
    // pub fn matches_hierarchy<'a>(&self, hierarchy: impl IntoIterator<Item = &'a str>) -> bool {
    //     match self.keywords.as_deref() {
    //         None => true,
    //         Some([]) => false,
    //         Some(keywords) => {
    //             let mut keyword_matches = vec![false; keywords.len()];
    //
    //             for part in hierarchy {
    //                 let lowercase_input = part.to_lowercase();
    //                 for (i, keyword) in keywords.iter().enumerate() {
    //                     if !keyword_matches[i] && lowercase_input.contains(keyword) {
    //                         keyword_matches[i] = true;
    //
    //                         if keyword_matches.iter().all(|&b| b) {
    //                             return true;
    //                         }
    //                     }
    //                 }
    //             }
    //
    //             false
    //         }
    //     }
    // }

    //TODO
    pub fn matches_hierarchy_v2<'a>(
        &self,
        hierarchy: impl IntoIterator<Item = &'a str>,
    ) -> Option<HierarchyRanges> {
        match self.keywords.as_deref() {
            None => Some(HierarchyRanges::default()),
            Some([]) => None,
            Some(keywords) => {
                let hierarchy = hierarchy.into_iter().map(str::to_lowercase).collect_vec();

                let all_ranges = keywords
                    .iter()
                    .map(|keyword| keyword.match_hierarchy(hierarchy.iter().map(String::as_str)))
                    .collect_vec();

                // all keywords must match!
                if all_ranges.iter().any(|ranges| ranges.is_empty()) {
                    None
                } else {
                    let mut result = HierarchyRanges::default();
                    for ranges in all_ranges {
                        result.merge(ranges);
                    }
                    Some(result)
                }
            }
        }
    }

    // /// Match the input text and return match ranges if any.
    // ///
    // /// This function does apply the full matching semantics:
    // /// - It returns `None` when there is no match.
    // /// - It returns `Some` when the filter is inactive (and thus matches everything), or if there
    // ///   is an actual match.
    // ///
    // /// See [`format_matching_text`] for formatting text according to the returned ranges.
    // pub fn find_matches(&self, text: &str) -> Option<impl Iterator<Item = Range<usize>> + '_> {
    //     let keywords = match self.keywords.as_deref() {
    //         None => {
    //             return Some(Either::Left(std::iter::empty()));
    //         }
    //         Some([]) => {
    //             return None;
    //         }
    //         Some(keywords) => keywords,
    //     };
    //
    //     let lower_case_text = text.to_lowercase();
    //
    //     let mut all_ranges = vec![];
    //     for keyword in keywords {
    //         if lower_case_text.contains(keyword) {
    //             all_ranges.extend(single_keyword_matches(&lower_case_text, keyword));
    //         } else {
    //             return None;
    //         }
    //     }
    //
    //     Some(Either::Right(MergeRanges::new(all_ranges)))
    // }

    // /// Find match ranges for any of the keywords in the provided input.
    // ///
    // /// Note that this function does not perform any actual matching semantics. It just provides
    // /// highlighting information for a hierarchy part that has already been tested for match using
    // /// [`Self::matches_hierarchy`].
    // pub fn find_ranges_for_keywords(&self, text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    //     let keywords = match self.keywords.as_deref() {
    //         None | Some([]) => {
    //             return Either::Left(std::iter::empty());
    //         }
    //
    //         Some(keywords) => keywords,
    //     };
    //
    //     let lower_case_text = text.to_lowercase();
    //
    //     let all_ranges = keywords
    //         .iter()
    //         .flat_map(|keyword| single_keyword_matches(&lower_case_text, keyword))
    //         .collect_vec();
    //
    //     Either::Right(MergeRanges::new(all_ranges))
    // }

    // /// Returns a formatted version of the text with the matching sections highlighted.
    // ///
    // /// Returns `None` when there is no match (so nothing should be displayed).
    // /// Returns `Some` when the filter is inactive (and thus matches everything), or if there is an
    // /// actual match.
    // pub fn matches_formatted(&self, ctx: &egui::Context, text: &str) -> Option<egui::WidgetText> {
    //     self.find_matches(text)
    //         .map(|match_iter| format_matching_text(ctx, text, match_iter, None))
    // }
}

// /// Full-text, case-insensitive matcher.
// pub struct FilterMatcher {
//     /// Lowercase keywords.
//     ///
//     /// If this is `None`, the filter is inactive and the matcher will accept everything. If this
//     /// is `Some([])`, the matcher will reject any input.
//     keywords: Option<Vec<String>>,
// }
//
// impl FilterMatcher {
//     fn new(query: Option<&str>) -> Self {
//         Self {
//             keywords: query.map(|s| s.split_whitespace().map(str::to_lowercase).collect()),
//         }
//     }
//
//     /// Is the filter currently active?
//     pub fn is_active(&self) -> bool {
//         self.keywords.is_some()
//     }
//
//     /// Is the filter set to match everything?
//     ///
//     /// Can be used by client code to short-circuit more expansive matching logic.
//     pub fn matches_everything(&self) -> bool {
//         self.keywords.is_none()
//     }
//
//     /// Is the filter set to match nothing?
//     ///
//     /// Can be used by client code to short-circuit more expansive matching logic.
//     pub fn matches_nothing(&self) -> bool {
//         self.keywords
//             .as_ref()
//             .is_some_and(|keywords| keywords.is_empty())
//     }
//
//     /// Does the given text match the filter?
//     pub fn matches(&self, text: &str) -> bool {
//         match self.keywords.as_deref() {
//             None => true,
//             Some([]) => false,
//             Some(keywords) => {
//                 let lowercase_input = text.to_lowercase();
//                 keywords
//                     .iter()
//                     .all(|keyword| lowercase_input.contains(keyword))
//             }
//         }
//     }
//
//     /// Does the given hierarchy match the filter?
//     ///
//     /// To match, each of the keyword must be present in at least one of the parts of the hierarchy.
//     pub fn matches_hierarchy<'a>(&self, hierarchy: impl IntoIterator<Item = &'a str>) -> bool {
//         match self.keywords.as_deref() {
//             None => true,
//             Some([]) => false,
//             Some(keywords) => {
//                 let mut keyword_matches = vec![false; keywords.len()];
//
//                 for part in hierarchy {
//                     let lowercase_input = part.to_lowercase();
//                     for (i, keyword) in keywords.iter().enumerate() {
//                         if !keyword_matches[i] && lowercase_input.contains(keyword) {
//                             keyword_matches[i] = true;
//
//                             if keyword_matches.iter().all(|&b| b) {
//                                 return true;
//                             }
//                         }
//                     }
//                 }
//
//                 false
//             }
//         }
//     }
//
//     /// Match the input text and return match ranges if any.
//     ///
//     /// This function does apply the full matching semantics:
//     /// - It returns `None` when there is no match.
//     /// - It returns `Some` when the filter is inactive (and thus matches everything), or if there
//     ///   is an actual match.
//     ///
//     /// See [`format_matching_text`] for formatting text according to the returned ranges.
//     pub fn find_matches(&self, text: &str) -> Option<impl Iterator<Item = Range<usize>> + '_> {
//         let keywords = match self.keywords.as_deref() {
//             None => {
//                 return Some(Either::Left(std::iter::empty()));
//             }
//             Some([]) => {
//                 return None;
//             }
//             Some(keywords) => keywords,
//         };
//
//         let lower_case_text = text.to_lowercase();
//
//         let mut all_ranges = vec![];
//         for keyword in keywords {
//             if lower_case_text.contains(keyword) {
//                 all_ranges.extend(single_keyword_matches(&lower_case_text, keyword));
//             } else {
//                 return None;
//             }
//         }
//
//         Some(Either::Right(MergeRanges::new(all_ranges)))
//     }
//
//     /// Find match ranges for any of the keywords in the provided input.
//     ///
//     /// Note that this function does not perform any actual matching semantics. It just provides
//     /// highlighting information for a hierarchy part that has already been tested for match using
//     /// [`Self::matches_hierarchy`].
//     pub fn find_ranges_for_keywords(&self, text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
//         let keywords = match self.keywords.as_deref() {
//             None | Some([]) => {
//                 return Either::Left(std::iter::empty());
//             }
//
//             Some(keywords) => keywords,
//         };
//
//         let lower_case_text = text.to_lowercase();
//
//         let all_ranges = keywords
//             .iter()
//             .flat_map(|keyword| single_keyword_matches(&lower_case_text, keyword))
//             .collect_vec();
//
//         Either::Right(MergeRanges::new(all_ranges))
//     }
//
//     /// Returns a formatted version of the text with the matching sections highlighted.
//     ///
//     /// Returns `None` when there is no match (so nothing should be displayed).
//     /// Returns `Some` when the filter is inactive (and thus matches everything), or if there is an
//     /// actual match.
//     pub fn matches_formatted(&self, ctx: &egui::Context, text: &str) -> Option<egui::WidgetText> {
//         self.find_matches(text)
//             .map(|match_iter| format_matching_text(ctx, text, match_iter, None))
//     }
// }

#[derive(Debug, Clone, PartialEq)]
struct Keyword {
    parts: Vec<String>,

    match_from_first_part_start: bool,

    match_to_last_part_end: bool,
}

impl Keyword {
    /// Create a [`Self`] based on a keyword string.
    ///
    /// The string must not contain any whitespace!
    fn new(mut keyword: &str) -> Self {
        debug_assert!(!keyword.contains(char::is_whitespace));

        let match_from_first_part_start = if let Some(k) = keyword.strip_prefix('/') {
            keyword = k;
            true
        } else {
            false
        };

        let match_to_last_part_end = if let Some(k) = keyword.strip_suffix('/') {
            keyword = k;
            true
        } else {
            false
        };

        let parts = keyword.split('/').map(str::to_lowercase).collect();

        Self {
            parts,
            match_from_first_part_start,
            match_to_last_part_end,
        }
    }

    //TODO: docstring / ranges are not sorted nor merged
    fn match_hierarchy<'a>(&self, hierarchy: impl IntoIterator<Item = &'a str>) -> HierarchyRanges {
        let mut state_machines = vec![];

        for (part_index, part) in hierarchy.into_iter().enumerate() {
            let lowercase_part = part.to_lowercase();

            state_machines.push(MatchStateMachine::new(self));

            for state_machine in &mut state_machines {
                state_machine.step(&lowercase_part, part_index);
            }
        }

        state_machines
            .into_iter()
            .filter_map(|state_machine| {
                if state_machine.matches() {
                    Some(state_machine.ranges)
                } else {
                    None
                }
            })
            .fold(HierarchyRanges::default(), |mut acc, ranges| {
                acc.merge(ranges);
                acc
            })
    }
}

#[derive(Debug, Default)]
pub struct HierarchyRanges {
    ranges: IntMap<usize, Vec<Range<usize>>>,
}

impl HierarchyRanges {
    pub fn merge(&mut self, other: Self) {
        for (part_index, part_ranges) in other.ranges {
            self.ranges
                .entry(part_index)
                .or_default()
                .extend(part_ranges);
        }
    }

    pub fn extend(&mut self, part_index: usize, ranges: impl IntoIterator<Item = Range<usize>>) {
        self.ranges.entry(part_index).or_default().extend(ranges);
    }

    pub fn push(&mut self, part_index: usize, range: Range<usize>) {
        self.ranges.entry(part_index).or_default().push(range);
    }

    pub fn remove(&mut self, part_index: usize) -> Option<impl Iterator<Item = Range<usize>>> {
        self.ranges
            .remove(&part_index)
            .map(|ranges| MergeRanges::new(ranges).into_iter())
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
    }
}

#[derive(Debug)]
enum MatchState {
    InProgress,
    Match,
    NoMatch,
}

#[derive(Debug)]
struct MatchStateMachine<'a> {
    keyword: &'a Keyword,
    current_keyword_part: usize,
    state: MatchState,
    ranges: HierarchyRanges,
}

impl<'a> MatchStateMachine<'a> {
    fn new(keyword: &'a Keyword) -> Self {
        Self {
            keyword,
            current_keyword_part: 0,
            state: MatchState::InProgress,
            ranges: Default::default(),
        }
    }

    fn matches(&self) -> bool {
        matches!(self.state, MatchState::Match)
    }

    fn step(&mut self, part: &str, part_index: usize) {
        if matches!(self.state, MatchState::Match | MatchState::NoMatch) {
            return;
        }

        let keyword_part = &self.keyword.parts[self.current_keyword_part];

        let has_part_after = self.current_keyword_part < self.keyword.parts.len() - 1;
        let has_part_before = 0 < self.current_keyword_part;
        let must_match_from_start = has_part_before || self.keyword.match_from_first_part_start;
        let must_match_to_end = has_part_after || self.keyword.match_to_last_part_end;

        let mut ranges = vec![];
        match (must_match_from_start, must_match_to_end) {
            (false, false) => {
                ranges.extend(single_keyword_matches(part, keyword_part));
            }

            (true, false) => {
                if part.starts_with(keyword_part) {
                    ranges.push(0..keyword_part.len());
                }
            }

            (false, true) => {
                if part.ends_with(keyword_part) {
                    ranges.push(part.len() - keyword_part.len()..part.len());
                }
            }

            (true, true) => {
                if part == keyword_part {
                    ranges.push(0..part.len());
                }
            }
        }

        if ranges.is_empty() {
            self.state = MatchState::NoMatch;
        } else {
            self.ranges.extend(part_index, ranges);
            self.current_keyword_part += 1;
        }

        if self.current_keyword_part == self.keyword.parts.len() {
            self.state = MatchState::Match;
        }
    }
}

/// Given a list of highlight sections defined by start/end indices and a string, produce a properly
/// highlighted [`egui::WidgetText`].
pub fn format_matching_text(
    ctx: &egui::Context,
    text: &str,
    match_iter: impl Iterator<Item = Range<usize>>,
    text_color: Option<egui::Color32>,
) -> egui::WidgetText {
    let mut current = 0;
    let mut job = egui::text::LayoutJob::default();

    let color = text_color.unwrap_or(Color32::PLACEHOLDER);

    for Range { start, end } in match_iter {
        if current < start {
            job.append(
                &text[current..start],
                0.0,
                egui::TextFormat {
                    font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                    color,
                    ..Default::default()
                },
            );
        }

        job.append(
            &text[start..end],
            0.0,
            egui::TextFormat {
                font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                color,
                background: ctx.style().visuals.selection.bg_fill,
                ..Default::default()
            },
        );

        current = end;
    }

    if current < text.len() {
        job.append(
            &text[current..],
            0.0,
            egui::TextFormat {
                font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                color,
                ..Default::default()
            },
        );
    }

    job.into()
}

/// Helper function to extract all matches of a given keyword in the given text.
fn single_keyword_matches<'a>(
    lower_case_text: &'a str,
    keyword: &'a str,
) -> impl Iterator<Item = Range<usize>> + 'a {
    let keyword_len = keyword.len();
    let mut start = 0;
    std::iter::from_fn(move || {
        let index = lower_case_text[start..].find(keyword)?;
        let start_index = start + index;
        start = start_index + keyword_len;
        Some(start_index..(start_index + keyword_len))
    })
}

/// Given a vector of ranges, iterate over the sorted, merged ranges.
struct MergeRanges {
    ranges: Vec<Range<usize>>,
    current: Option<Range<usize>>,
}

impl MergeRanges {
    fn new(mut ranges: Vec<Range<usize>>) -> Self {
        ranges.sort_by_key(|r| usize::MAX - r.start);
        let current = ranges.pop();
        Self { ranges, current }
    }
}

impl Iterator for MergeRanges {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut current = self.current.take()?;

        while let Some(next) = self.ranges.pop() {
            if next.start <= current.end {
                current.end = current.end.max(next.end);
            } else {
                self.current = Some(next);
                return Some(current);
            }
        }

        Some(current)
    }
}

#[cfg(test)]
mod test {
    #![expect(clippy::single_range_in_vec_init)]

    use super::*;

    #[test]
    fn test_merge_range() {
        // merge to one
        assert_eq!(MergeRanges::new(vec![0..10, 5..15]).collect_vec(), [0..15]);
        assert_eq!(MergeRanges::new(vec![5..15, 0..10]).collect_vec(), [0..15]);
        assert_eq!(
            MergeRanges::new(vec![0..4, 3..4, 1..2, 5..15, 5..15, 0..10]).collect_vec(),
            [0..15]
        );
        assert_eq!(MergeRanges::new(vec![0..11, 11..15]).collect_vec(), [0..15]);

        // independent
        assert_eq!(
            MergeRanges::new(vec![0..5, 11..15]).collect_vec(),
            [0..5, 11..15]
        );
        assert_eq!(
            MergeRanges::new(vec![11..15, 0..5]).collect_vec(),
            [0..5, 11..15]
        );

        // mixed
        assert_eq!(
            MergeRanges::new(vec![0..5, 20..30, 3..6, 25..27, 30..35]).collect_vec(),
            [0..6, 20..35]
        );
    }

    // #[test]
    // fn test_match_all() {
    //     let inactive_matcher = FilterMatcher::new(None);
    //
    //     assert_eq!(
    //         inactive_matcher.find_matches("haystack").unwrap().count(),
    //         0
    //     );
    // }
    //
    // #[test]
    // fn test_match_nothing() {
    //     let inactive_matcher = FilterMatcher::new(Some(""));
    //
    //     assert!(inactive_matcher.find_matches("haystack").is_none());
    // }
    //
    // #[test]
    // fn test_match() {
    //     let matcher = FilterMatcher::new(Some("str tru re"));
    //
    //     // filter active but doesn't match
    //     assert!(matcher.find_matches("struct").is_none());
    //
    //     assert_eq!(
    //         matcher.find_matches("structure").unwrap().collect_vec(),
    //         [0..4, 7..9]
    //     );
    // }

    // #[test]
    // fn test_match_hierarchy() {
    //     let matcher = FilterMatcher::new(Some("one TWo three"));
    //
    //     // matches
    //     assert!(matcher.matches_hierarchy(["oNe", "two", "three"]));
    //     assert!(matcher.matches_hierarchy(["tHRee", "One", "two"]));
    //     assert!(matcher.matches_hierarchy(["three", "one", "nothing", "two"]));
    //     assert!(matcher.matches_hierarchy(["thrEEone", "nothing", "TWO"]));
    //     assert!(matcher.matches_hierarchy(["three", "twONE"]));
    //
    //     // doesn't match
    //     assert!(!matcher.matches_hierarchy(["one", "two", "four"]));
    // }
    //
    // #[test]
    // fn test_find_ranges_for_keywords() {
    //     let matcher = FilterMatcher::new(Some("one two three"));
    //
    //     assert_eq!(matcher.find_ranges_for_keywords("haystack").count(), 0);
    //     assert_eq!(
    //         matcher.find_ranges_for_keywords("xxONExx").collect_vec(),
    //         [2..5]
    //     );
    //     assert_eq!(
    //         matcher.find_ranges_for_keywords("xxTWonExx").collect_vec(),
    //         [2..7]
    //     );
    //     assert_eq!(
    //         matcher
    //             .find_ranges_for_keywords("xxTWonExthree")
    //             .collect_vec(),
    //         [2..7, 8..13]
    //     );
    // }

    #[test]
    fn test_keyword() {
        assert_eq!(
            Keyword::new("a"),
            Keyword {
                parts: vec!["a".into()],
                match_from_first_part_start: false,
                match_to_last_part_end: false
            }
        );

        assert_eq!(
            Keyword::new("/a"),
            Keyword {
                parts: vec!["a".into()],
                match_from_first_part_start: true,
                match_to_last_part_end: false
            }
        );

        assert_eq!(
            Keyword::new("a/"),
            Keyword {
                parts: vec!["a".into()],
                match_from_first_part_start: false,
                match_to_last_part_end: true
            }
        );

        assert_eq!(
            Keyword::new("/a/"),
            Keyword {
                parts: vec!["a".into()],
                match_from_first_part_start: true,
                match_to_last_part_end: true
            }
        );

        assert_eq!(
            Keyword::new("a/b"),
            Keyword {
                parts: vec!["a".into(), "b".into()],
                match_from_first_part_start: false,
                match_to_last_part_end: false
            }
        );

        assert_eq!(
            Keyword::new("a/b/"),
            Keyword {
                parts: vec!["a".into(), "b".into()],
                match_from_first_part_start: false,
                match_to_last_part_end: true
            }
        );

        assert_eq!(
            Keyword::new("/a/b/c/d"),
            Keyword {
                parts: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                match_from_first_part_start: true,
                match_to_last_part_end: false
            }
        );
    }

    #[test]
    fn test_keyword_match_hierarchy() {
        fn match_and_normalize(query: &str, hierarchy: &[&str]) -> Vec<Vec<Range<usize>>> {
            let keyword = Keyword::new(query);
            let hierarchy = hierarchy.to_vec();

            let mut ranges = keyword.match_hierarchy(hierarchy.clone());

            let result = (0..hierarchy.len())
                .map(|i| {
                    ranges
                        .remove(i)
                        .map(|iter| iter.collect_vec())
                        .unwrap_or_else(Vec::new)
                })
                .collect();

            assert!(ranges.is_empty());

            result
        }

        assert_eq!(match_and_normalize("a", &["a"]), vec![vec![0..1]]);
        assert_eq!(match_and_normalize("a", &["aaa"]), vec![vec![0..3]]);

        assert_eq!(
            match_and_normalize("A/", &["aaa", "aaa"]),
            vec![vec![2..3], vec![2..3]]
        );

        assert_eq!(
            match_and_normalize("/a", &["aaa", "aaa"]),
            vec![vec![0..1], vec![0..1]]
        );

        assert_eq!(
            match_and_normalize("/a", &["aaa", "bbb"]),
            vec![vec![0..1], vec![]]
        );

        assert_eq!(
            match_and_normalize("a/b", &["aaa", "bbb"]),
            vec![vec![2..3], vec![0..1]]
        );

        assert_eq!(
            match_and_normalize("a/b/c", &["aaa", "b", "cccc"]),
            vec![vec![2..3], vec![0..1], vec![0..1]]
        );

        assert!(
            match_and_normalize("/a/b/c", &["aaa", "b", "cccc"])
                .into_iter()
                .flatten()
                .count()
                == 0,
        );

        assert!(
            match_and_normalize("a/B/c/", &["aaa", "b", "cccc"])
                .into_iter()
                .flatten()
                .count()
                == 0,
        );

        assert_eq!(
            match_and_normalize("ab/cd", &["xxxAb", "cDaB", "Cdxxx"]),
            vec![vec![3..5], vec![0..4], vec![0..2]]
        );

        assert_eq!(
            match_and_normalize("ab/ab", &["xxxAb", "aB", "aBxxx"]),
            vec![vec![3..5], vec![0..2], vec![0..2]]
        );
    }

    #[test]
    fn test_matches_hierarchy_v2() {
        fn match_and_normalize(query: &str, hierarchy: &[&str]) -> Option<Vec<Vec<Range<usize>>>> {
            let matcher = FilterMatcher::new(Some(query));
            let hierarchy = hierarchy.to_vec();

            matcher
                .matches_hierarchy_v2(hierarchy.clone())
                .map(|mut ranges| {
                    let result = (0..hierarchy.len())
                        .map(|i| {
                            ranges
                                .remove(i)
                                .map(|iter| iter.collect_vec())
                                .unwrap_or_else(Vec::new)
                        })
                        .collect();

                    assert!(ranges.is_empty());

                    result
                })
        }

        assert_eq!(
            match_and_normalize("ab/cd", &["xxxAb", "cDaB", "Cdxxx"]),
            Some(vec![vec![3..5], vec![0..4], vec![0..2]])
        );

        assert_eq!(
            match_and_normalize("ab/cd xx/", &["xxxAb", "cDaB", "Cdxxx"]),
            Some(vec![vec![3..5], vec![0..4], vec![0..2, 3..5]])
        );

        assert_eq!(
            match_and_normalize("ab/cd bla", &["xxxAb", "cDaB", "Cdxxx"]),
            None
        );

        //TODO: moar tests
    }
}
