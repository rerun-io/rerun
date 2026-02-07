use std::ops::Range;

use egui::{Color32, NumExt as _, Widget as _};
use itertools::Itertools as _;
use smallvec::SmallVec;

use crate::{UiExt as _, icons, list_item};

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
        getrandom::fill(&mut random_bytes).expect("Couldn't get random bytes");

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

    /// Return the current session ID of the filter widget.
    ///
    /// This returns `Some` value iff the filter is active AND the query is non-empty.
    ///
    /// Rationale: this is primarily used to keep track of a different collapse state while a search
    /// session is ongoing. When the filter is active but the query is empty, we display all
    /// entities without filtering, so the collapse state is the same as when the filter is
    /// inactive.
    pub fn session_id(&self) -> Option<egui::Id> {
        let state = self.inner_state.as_ref()?;
        (!state.filter_query.is_empty()).then_some(state.session_id)
    }

    /// Return a filter matcher for the current query.
    pub fn filter(&self) -> FilterMatcher {
        FilterMatcher::new(self.query())
    }

    /// Display the filter widget as a section title.
    ///
    /// In this mode, the UI serves primarily as a section title. The filter is active when
    /// explicitly turned on using the search button, which creates a session that is ended by
    /// clicking the close button.
    ///
    /// Note: this uses [`egui::Ui::available_width`] to determine the location of the right-aligned
    /// search button, as usual for [`list_item::ListItem`]-based widgets.
    pub fn section_title_ui(
        &mut self,
        ui: &mut egui::Ui,
        section_title: impl Into<egui::WidgetText>,
    ) -> Option<egui::Response> {
        let mut toggle_search_clicked = false;

        let is_searching = self.inner_state.is_some();

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
            ui.list_item()
                .interactive(false)
                .force_background(ui.tokens().section_header_color)
                .show_flat(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        if self.inner_state.is_some()
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                            })
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

                            let response =
                                egui::TextEdit::singleline(&mut inner_state.filter_query)
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
                    .action_button(
                        if is_searching {
                            &icons::CLOSE
                        } else {
                            &icons::SEARCH
                        },
                        if is_searching {
                            "Stop search"
                        } else {
                            "Search"
                        },
                        || {
                            toggle_search_clicked = true;
                        },
                    ),
                );
        });

        // defer button handling because we can't mutably borrow `self` in both closures above
        if toggle_search_clicked {
            if self.inner_state.is_none() {
                self.activate("");
            } else {
                self.inner_state = None;
            }
        }

        title_response
    }

    /// Display the filter widget as a search field.
    ///
    /// In this mode, the filter is active as soon as the query is non-empty. The session remains
    /// active until the query is cleared.
    pub fn search_field_ui(&mut self, ui: &mut egui::Ui, hint_text: impl Into<egui::WidgetText>) {
        let inner_state = self.inner_state.get_or_insert_with(Default::default);

        let textedit_id = ui.id().with("textedit");
        let response = ui.ctx().read_response(textedit_id);

        let visuals = response
            .as_ref()
            .map(|r| ui.style().interact(r))
            .unwrap_or_else(|| &ui.visuals().widgets.inactive);

        let selection_stroke = ui.visuals().selection.stroke;
        let stroke = if response.is_some_and(|r| r.has_focus()) {
            selection_stroke
        } else {
            let mut stroke = visuals.bg_stroke;
            stroke.width = selection_stroke.width;
            stroke
        };

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(3, 2))
            .fill(ui.visuals().extreme_bg_color)
            .stroke(stroke)
            .corner_radius(visuals.corner_radius)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(19.0);

                    ui.add_enabled_ui(false, |ui| ui.small_icon_button(&icons::SEARCH, "Search"));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !inner_state.filter_query.is_empty()
                            && ui.small_icon_button(&icons::CLOSE, "Close").clicked()
                        {
                            *inner_state = Default::default();
                        }

                        ui.add(
                            egui::TextEdit::singleline(&mut inner_state.filter_query)
                                .id(textedit_id)
                                .frame(false)
                                .hint_text(hint_text)
                                .desired_width(ui.available_width()),
                        )
                    });
                });
            });

        if self
            .inner_state
            .as_ref()
            .is_some_and(|state| state.filter_query.is_empty())
        {
            self.inner_state = None;
        }
    }
}

// --

/// Full-text, case-insensitive matcher.
///
/// All keywords must match for the filter to match (`AND` semantics).
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

    /// Match a path and return the highlight ranges if any.
    ///
    /// `None`: the filter is active, but the path didn't match the keyword
    /// `Some(ranges)`: either the filter is inactive (i.e., it matches everything), or it is active
    /// all keywords matched at least once (including when there are no keywords at all).
    pub fn match_path<'a>(&self, path: impl IntoIterator<Item = &'a str>) -> Option<PathRanges> {
        match self.keywords.as_deref() {
            None | Some([]) => Some(PathRanges::default()),

            Some(keywords) => {
                let path = path.into_iter().map(str::to_lowercase).collect_vec();

                let all_ranges = keywords
                    .iter()
                    .map(|keyword| keyword.match_path(path.iter().map(String::as_str)))
                    .collect_vec();

                // all keywords must match!
                if all_ranges.iter().any(|ranges| ranges.is_empty()) {
                    None
                } else {
                    let mut result = PathRanges::default();
                    for ranges in all_ranges {
                        result.merge(ranges);
                    }
                    Some(result)
                }
            }
        }
    }
}

/// A single keyword from a query.
///
/// ## Semantics
///
/// If the keyword has a single part, it can match anywhere in any part of the tested path, unless
/// `match_from_first_part_start` and/or `match_to_last_part_end`, which have the same behavior as
/// regex's `^` and `$`.
///
/// If the keyword has multiple parts, e.g. "first/second", the tested path must have at least one instance of contiguous
/// parts which match the corresponding keyword parts. In that context, the keyword parts have the
/// following behavior:
/// - First keyword part: `^part$` if `match_from_first_part_start`, `part$` otherwise
/// - Last keyword part: `^part$` if `match_to_last_part_end`, `^part` otherwise
/// - Other keyword parts: `^part$`
#[derive(Debug, Clone, PartialEq)]
struct Keyword {
    /// The parts of a keyword.
    ///
    /// To match, a path needs to have some contiguous parts which each match the corresponding
    /// keyword parts.
    parts: Vec<String>,
    match_from_first_part_start: bool,
    match_to_last_part_end: bool,
}

impl Keyword {
    /// Create a [`Self`] based on a keyword string.
    ///
    /// The string must not contain any whitespace!
    fn new(mut keyword: &str) -> Self {
        // Invariant: keywords are not empty
        debug_assert!(!keyword.is_empty());
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

    /// Match the keyword against the provided path.
    ///
    /// An empty [`PathRanges`] means that the keyword didn't match the path.
    ///
    /// Implementation notes:
    /// - This function is akin to a "sliding window" of the keyword parts against the path parts,
    ///   trying to find some "alignment" yielding a match.
    /// - We must be thorough as we want to find _all_ match highlights (i.e., we don't early out as
    ///   soon as we find a match).
    fn match_path<'a>(&self, lowercase_path: impl ExactSizeIterator<Item = &'a str>) -> PathRanges {
        let mut state_machines = vec![];

        let path_length = lowercase_path.len();

        for (path_part_index, path_part) in lowercase_path.into_iter().enumerate() {
            // Only start a new state machine if it has a chance to be matched entirely.
            if self.parts.len() <= (path_length - path_part_index) {
                state_machines.push(MatchStateMachine::new(self));
            }

            for state_machine in &mut state_machines {
                state_machine.process_next_path_part(path_part, path_part_index);
            }
        }

        state_machines
            .into_iter()
            .filter_map(|state_machine| {
                if state_machine.did_match() {
                    Some(state_machine.ranges)
                } else {
                    None
                }
            })
            .fold(PathRanges::default(), |mut acc, ranges| {
                acc.merge(ranges);
                acc
            })
    }
}

/// Accumulates highlight ranges for the various parts of a path.
///
/// The ranges are accumulated and stored unmerged and unordered, but are _always_ ordered and
/// merged when read, which only happens with [`Self::remove`].
#[derive(Debug, Default)]
pub struct PathRanges {
    ranges: ahash::HashMap<usize, Vec<Range<usize>>>,
}

impl PathRanges {
    /// Merge another [`Self`].
    pub fn merge(&mut self, other: Self) {
        #[expect(clippy::iter_over_hash_type)] // We sort on remove
        for (part_index, part_ranges) in other.ranges {
            self.ranges
                .entry(part_index)
                .or_default()
                .extend(part_ranges);
        }
    }

    /// Add ranges to a given part index.
    pub fn extend(&mut self, part_index: usize, ranges: impl IntoIterator<Item = Range<usize>>) {
        self.ranges.entry(part_index).or_default().extend(ranges);
    }

    /// Add a single range to a given part index.
    pub fn push(&mut self, part_index: usize, range: Range<usize>) {
        self.ranges.entry(part_index).or_default().push(range);
    }

    /// Remove the ranges for the given part and (if any) return them sorted and merged.
    pub fn remove(
        &mut self,
        part_index: usize,
    ) -> Option<impl Iterator<Item = Range<usize>> + use<>> {
        self.ranges.remove(&part_index).map(MergeRanges::new)
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    /// Convert to a `Vec` based structure.
    #[cfg(test)]
    fn into_vec(mut self, length: usize) -> Vec<Vec<Range<usize>>> {
        let result = (0..length)
            .map(|i| {
                self.remove(i)
                    .map(|iter| iter.collect_vec())
                    .unwrap_or_default()
            })
            .collect();

        debug_assert!(self.is_empty());

        result
    }
}

// ---

#[derive(Debug)]
enum MatchState {
    InProgress,
    Match,
    NoMatch,
}

/// State machine used to test a given keyword against a given sequence of path parts.
#[derive(Debug)]
struct MatchStateMachine<'a> {
    /// The keyword we're matching with.
    keyword: &'a Keyword,

    /// Which part of the keyword are we currently matching?
    current_keyword_part: usize,

    /// Our current state.
    state: MatchState,

    /// The highlight ranges we've gathered so far.
    ranges: PathRanges,
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

    fn did_match(&self) -> bool {
        matches!(self.state, MatchState::Match)
    }

    fn process_next_path_part(&mut self, part: &str, part_index: usize) {
        if matches!(self.state, MatchState::Match | MatchState::NoMatch) {
            return;
        }

        let keyword_part = &self.keyword.parts[self.current_keyword_part];

        let has_part_after = self.current_keyword_part < self.keyword.parts.len() - 1;
        let has_part_before = 0 < self.current_keyword_part;
        let must_match_from_start = has_part_before || self.keyword.match_from_first_part_start;
        let must_match_to_end = has_part_after || self.keyword.match_to_last_part_end;

        let mut ranges = SmallVec::<[Range<usize>; 2]>::new();
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

// ---

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

// ---

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
    fn test_keyword_match_path() {
        fn match_and_normalize(query: &str, lowercase_path: &[&str]) -> Vec<Vec<Range<usize>>> {
            Keyword::new(query)
                .match_path(lowercase_path.iter().copied())
                .into_vec(lowercase_path.len())
        }

        assert_eq!(match_and_normalize("a", &["a"]), vec![vec![0..1]]);
        assert_eq!(match_and_normalize("a", &["aaa"]), vec![vec![0..3]]);

        assert_eq!(
            match_and_normalize("a/", &["aaa", "aaa"]),
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
            match_and_normalize("a/b/c/", &["aaa", "b", "cccc"])
                .into_iter()
                .flatten()
                .count()
                == 0,
        );

        assert_eq!(
            match_and_normalize("ab/cd", &["xxxab", "cdab", "cdxxx"]),
            vec![vec![3..5], vec![0..4], vec![0..2]]
        );

        assert_eq!(
            match_and_normalize("ab/ab", &["xxxab", "ab", "abxxx"]),
            vec![vec![3..5], vec![0..2], vec![0..2]]
        );
    }

    #[test]
    fn test_match_path() {
        fn match_and_normalize(query: &str, path: &[&str]) -> Option<Vec<Vec<Range<usize>>>> {
            FilterMatcher::new(Some(query))
                .match_path(path.iter().copied())
                .map(|ranges| ranges.into_vec(path.len()))
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
    }
}
