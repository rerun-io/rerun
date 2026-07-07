//! Snapshot test for how the command palette renders rows:
//! enabled vs. disabled (grayed-out) commands, and long paths that wrap to several lines.

use egui::Vec2;
use re_log_types::EntityPath;
use re_ui::{CmdRow, FuzzyQuery, RowState, SyntaxHighlighting as _, paint_command_row};

/// Builds a plain command row, highlighting the characters matched by `query`.
#[expect(clippy::fn_params_excessive_bools)] // test helper 🤷
fn command_row(
    ui: &egui::Ui,
    query: &FuzzyQuery,
    text: &str,
    enabled: bool,
    selected: bool,
) -> CmdRow {
    let fuzzy_match = query.try_match(text.to_owned()).expect("should match");

    let text_color = if !enabled {
        ui.visuals().weak_text_color()
    } else if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().widgets.inactive.fg_stroke.color
    };

    let job = egui::text::LayoutJob::simple(
        fuzzy_match.target().to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        text_color,
        f32::INFINITY,
    );

    let job = if enabled {
        fuzzy_match.highlight_matching_text(ui.style(), &job, selected)
    } else {
        job
    };

    CmdRow {
        job,
        kb_shortcut: String::new(),
        tooltip: None,
    }
}

#[test]
fn command_palette_enabled_vs_disabled() {
    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(320.0, 100.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                let query = FuzzyQuery::new("close".to_owned());

                egui::Frame {
                    inner_margin: 4.0.into(),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    // (text, enabled, selected)
                    let rows = [
                        ("Close current recording", false, false),
                        ("Close all recordings", true, true),
                        ("Copy link to selected time range", true, false),
                    ];

                    for (text, enabled, selected) in rows {
                        let row = command_row(ui, &query, text, enabled, selected);
                        let state = if !enabled {
                            RowState::Disabled
                        } else if selected {
                            RowState::Selected
                        } else {
                            RowState::Normal
                        };
                        paint_command_row(ui, row, state);
                    }
                });
            });

    harness.run();
    harness.snapshot("command_palette_enabled_vs_disabled");
}

/// A path wider than the palette should wrap onto several lines instead of overflowing.
#[test]
fn command_palette_wraps_long_paths() {
    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(320.0, 160.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                egui::Frame {
                    inner_margin: 4.0.into(),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    // (path, selected)
                    let rows = [
                        ("/short/entity", false),
                        (
                            "/world/robot/arm/left/gripper/camera/a_very_long_leaf_entity_name",
                            false,
                        ),
                        // Selected paths drop syntax highlighting (it clashes with the
                        // selection background) and render as plain selection-colored text:
                        (
                            "/world/robot/arm/left/gripper/camera/a_very_long_leaf_entity_name",
                            true,
                        ),
                    ];

                    for (path, selected) in rows {
                        let mut job = EntityPath::from(path).syntax_highlighted(ui.style());
                        if selected {
                            for section in &mut job.sections {
                                section.format.color = ui.visuals().selection.stroke.color;
                            }
                        }

                        let row = CmdRow {
                            job,
                            kb_shortcut: String::new(),
                            tooltip: None,
                        };
                        let state = if selected {
                            RowState::Selected
                        } else {
                            RowState::Normal
                        };
                        paint_command_row(ui, row, state);
                    }
                });
            });

    harness.run();
    harness.snapshot("command_palette_wraps_long_paths");
}

/// A minimal provider with two groups, the first of which has more
/// entries than the palette shows without expanding.
struct ManyCommandsProvider;

impl re_ui::CommandPaletteProvider<String> for ManyCommandsProvider {
    fn all_matching(&mut self, _query: &FuzzyQuery) -> Vec<re_ui::MatchGroup<String>> {
        let group_a = (0..8)
            .map(|i| re_ui::MatchedCmd {
                command: format!("First group command {i}"),
                fuzzy_match: re_ui::FuzzyMatch::lowest(format!("First group command {i}")),
                enabled: true,
            })
            .collect();

        let group_b = (0..2)
            .map(|i| re_ui::MatchedCmd {
                command: format!("Second group command {i}"),
                fuzzy_match: re_ui::FuzzyMatch::lowest(format!("Second group command {i}")),
                enabled: true,
            })
            .collect();

        vec![group_a, group_b]
    }

    fn cmd_row(&self, ui: &egui::Ui, cmd: &re_ui::MatchedCmd<String>, selected: bool) -> CmdRow {
        let text_color = if selected {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        CmdRow {
            job: egui::text::LayoutJob::simple(
                cmd.fuzzy_match.target().to_owned(),
                egui::TextStyle::Button.resolve(ui.style()),
                text_color,
                f32::INFINITY,
            ),
            kb_shortcut: String::new(),
            tooltip: None,
        }
    }
}

/// Truncated groups end in a "+ N more" button which is keyboard-selectable
/// and expands the group when activated.
#[test]
fn command_palette_expand_truncated_group() {
    let mut palette = re_ui::CommandPalette::default();
    palette.toggle(); // Make it visible.

    let mut harness =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, Vec2::new(500.0, 320.0))
            .build_ui(move |ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                let selected = palette.show(ui.ctx(), &mut ManyCommandsProvider);
                assert!(selected.is_none(), "nothing should get selected");
            });

    harness.run();
    harness.snapshot("command_palette_truncated_group");

    // Navigate down to the "+ 3 more" button of the first group and press enter:
    for _ in 0..5 {
        harness.key_press(egui::Key::ArrowDown);
        harness.run();
    }
    harness.key_press(egui::Key::Enter);
    harness.run();

    harness.snapshot("command_palette_expanded_group");
}
