//! The command types that act on a target:
//!
//! - [`RecordingCommand`]: commands that act on a specific recording.
//! - [`RedapServerCommand`]: commands that act on a specific Redap server.
//! - [`TableCommand`]: commands that act on a specific Redap entry (dataset or table).
//!
//! [`CommandKind`] names any of them, or a [`UICommand`], without a target.

mod command_kind;
mod environment;
mod recording_command;
mod redap_server_command;
mod table_command;

pub use self::command_kind::{CommandKind, CommandScope};
pub use self::environment::CommandEnvironment;
pub use self::recording_command::{
    RecordingCommand, RecordingCommandKind, RecordingCommandSender, SetPlaybackSpeed,
};
pub use self::redap_server_command::{
    RedapServerCommand, RedapServerCommandKind, RedapServerCommandSender,
};
pub use self::table_command::{TableCommand, TableCommandKind, TableCommandSender};

use egui::KeyboardShortcut;
use re_ui::{UICommand, consume_best_shortcut};
use smallvec::{SmallVec, smallvec};

/// The keyboard shortcut for refreshing the current view.
///
/// `Cmd-R` is the natural "reload" shortcut on Mac; elsewhere `F5` is used.
///
/// This does not work in a web browser, unless we also prevent the browser from handling the shortcut
/// (which we currently don't do). The fix for that would be to call `.prevent_default()` on the event in eframe.
pub fn refresh_shortcuts(os: egui::os::OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
    use egui::{Key, KeyboardShortcut, Modifiers};
    if os == egui::os::OperatingSystem::Mac {
        smallvec![
            KeyboardShortcut::new(Modifiers::COMMAND, Key::R),
            KeyboardShortcut::new(Modifiers::NONE, Key::F5),
        ]
    } else {
        smallvec![KeyboardShortcut::new(Modifiers::NONE, Key::F5)]
    }
}

/// The keyboard shortcut for undoing a blueprint edit.
pub fn undo_shortcuts() -> SmallVec<[KeyboardShortcut; 2]> {
    use egui::{Key, KeyboardShortcut, Modifiers};
    smallvec![KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)]
}

/// The keyboard shortcuts for redoing a blueprint edit.
pub fn redo_shortcuts(os: egui::os::OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
    use egui::{Key, KeyboardShortcut, Modifiers};
    if os == egui::os::OperatingSystem::Mac {
        smallvec![
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
            KeyboardShortcut::new(Modifiers::COMMAND, Key::Y),
        ]
    } else {
        smallvec![
            KeyboardShortcut::new(Modifiers::CTRL, Key::Y),
            KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::Z),
        ]
    }
}

/// A command bound to its target from a [`CommandEnvironment`], ready to dispatch.
#[derive(Clone, Debug)]
pub enum BoundCommand {
    Ui(UICommand),
    Recording(RecordingCommand),
    RedapServer(RedapServerCommand),
    Table(TableCommand),
}

/// Consume the "timeline" keyboard shortcuts: playback controls bound to
/// space / arrow / home / end keys, plus the playback-speed chord.
///
/// These must be consumed early (in `on_begin_pass`) so egui doesn't first use them to
/// move keyboard focus or scroll. The result is environment-free: pair it with the active
/// recording via [`RecordingCommandKind::for_environment`] once that is known (which is
/// why this is kept separate from [`listen_for_kb_shortcuts`]).
pub fn consume_timeline_shortcut(egui_ctx: &egui::Context) -> Option<RecordingCommandKind> {
    use strum::IntoEnumIterator as _;

    let os = egui_ctx.os();

    let commands = RecordingCommandKind::iter()
        .filter(|kind| kind.is_timeline())
        .flat_map(|kind| {
            kind.kb_shortcuts(os)
                .into_iter()
                .map(move |kb_shortcut| (kb_shortcut, kind))
        })
        .collect();

    consume_best_shortcut(egui_ctx, commands)
        .or_else(|| RecordingCommandKind::handle_playback_chord(egui_ctx))
}

/// Listen for all non-timeline keyboard shortcuts, resolved against `env`.
///
/// Call this where the [`CommandEnvironment`] is live (e.g. while building the frame's UI),
/// so that recording commands target the current recording and context-dependent shortcuts
/// can pick their target. Timeline shortcuts are handled separately, and earlier, by
/// [`consume_timeline_shortcut`].
///
/// Note that all shortcuts must be matched together, so that e.g. `Cmd-Shift-S` is checked
/// before `Cmd-S` even if the two shortcuts belong to different command types.
pub fn listen_for_kb_shortcuts(
    egui_ctx: &egui::Context,
    env: &CommandEnvironment,
) -> Option<BoundCommand> {
    use strum::IntoEnumIterator as _;

    #[derive(Clone, Copy)]
    enum Matched {
        Ui(UICommand),
        Recording(RecordingCommandKind),
        RedapServer(RedapServerCommandKind),
        Table(TableCommandKind),
    }

    let os = egui_ctx.os();

    let commands = itertools::chain!(
        UICommand::iter().flat_map(|cmd| {
            cmd.kb_shortcuts(os)
                .into_iter()
                .map(move |kb_shortcut| (kb_shortcut, Matched::Ui(cmd)))
        }),
        // Timeline commands (space/arrows) are consumed earlier, in `on_begin_pass`
        // via `consume_timeline_shortcut`, so exclude them here to avoid handling them twice.
        // Recording and table commands share the undo and redo shortcuts, and only the one
        // that can run in the current environment consumes them.
        RecordingCommandKind::iter()
            .filter(|kind| !kind.is_timeline() && kind.for_environment(env).is_some())
            .flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, Matched::Recording(kind)))
            }),
        // Only bind (and thus consume) context-dependent shortcuts that can actually run in the
        // current environment. Otherwise e.g. `Cmd-R` would be swallowed by the server-refresh
        // command even when no server is selected.
        //
        // Table commands come before server commands: when both could handle a shortcut
        // (table refresh and server refresh share one), the more specific table command wins.
        TableCommandKind::iter()
            .filter(|kind| kind.for_environment(env).is_some())
            .flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, Matched::Table(kind)))
            }),
        RedapServerCommandKind::iter()
            .filter(|kind| kind.for_environment(env).is_some())
            .flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, Matched::RedapServer(kind)))
            }),
    )
    .collect();

    match consume_best_shortcut(egui_ctx, commands)? {
        Matched::Ui(cmd) => Some(BoundCommand::Ui(cmd)),
        Matched::Recording(kind) => kind.for_environment(env).map(BoundCommand::Recording),
        Matched::RedapServer(kind) => kind.for_environment(env).map(BoundCommand::RedapServer),
        Matched::Table(kind) => kind.for_environment(env).map(BoundCommand::Table),
    }
}

#[test]
fn undo_redo_shortcuts_preserve_text_editing() {
    use egui::{Event, Key, RawInput, os::OperatingSystem};

    let env = CommandEnvironment {
        recording: Some(re_log_types::StoreId::empty_recording()),
        redap_server: None,
        has_editable_redap_server: false,
        table: None,
        has_table_blueprint: false,
    };
    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Nix,
    ] {
        for kind in [
            RecordingCommandKind::UndoBlueprintEdit,
            RecordingCommandKind::RedoBlueprintEdit,
        ] {
            for shortcut in kind.kb_shortcuts(os) {
                for editing_text in [false, true] {
                    let ctx = egui::Context::default();
                    ctx.set_os(os);
                    ctx.begin_pass(RawInput {
                        events: vec![Event::Key {
                            key: shortcut.logical_key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: shortcut.modifiers,
                        }],
                        ..Default::default()
                    });
                    if editing_text {
                        let id = egui::Id::unique("text_edit");
                        egui::text_edit::TextEditState::default().store(&ctx, id);
                        ctx.memory_mut(|memory| memory.request_focus(id));
                        assert!(ctx.text_edit_focused());
                    }

                    let command = listen_for_kb_shortcuts(&ctx, &env);
                    if editing_text {
                        assert!(command.is_none(), "{os:?}: {shortcut:?}");
                        assert!(ctx.input_mut(|input| input.consume_shortcut(&shortcut)));
                    } else {
                        assert!(
                            matches!(command, Some(BoundCommand::Recording(command)) if command.kind == kind)
                        );
                        assert!(
                            !ctx.input(
                                |input| input.key_pressed(Key::Z) || input.key_pressed(Key::Y)
                            )
                        );
                    }
                    ctx.end_pass().textures_delta.clear();
                }
            }
        }
    }
}

#[test]
fn check_for_clashing_command_shortcuts() {
    use egui::os::OperatingSystem;

    fn clashes(a: KeyboardShortcut, b: KeyboardShortcut) -> bool {
        if a.logical_key != b.logical_key {
            return false;
        }

        if a.modifiers.alt != b.modifiers.alt {
            return false;
        }

        if a.modifiers.shift != b.modifiers.shift {
            return false;
        }

        // On Non-Mac, command is interpreted as ctrl!
        (a.modifiers.command || a.modifiers.ctrl) == (b.modifiers.command || b.modifiers.ctrl)
    }

    use strum::IntoEnumIterator as _;

    for os in [
        OperatingSystem::Mac,
        OperatingSystem::Windows,
        OperatingSystem::Nix,
    ] {
        // All shortcuts of all command types, so we also catch clashes across types:
        let all_commands: Vec<(String, KeyboardShortcut)> = itertools::chain!(
            UICommand::iter().flat_map(|cmd| {
                cmd.kb_shortcuts(os)
                    .into_iter()
                    .map(move |shortcut| (format!("Ui::{cmd:?}"), shortcut))
            }),
            RecordingCommandKind::iter().flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |shortcut| (format!("Recording::{kind:?}"), shortcut))
            }),
            RedapServerCommandKind::iter().flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |shortcut| (format!("RedapServer::{kind:?}"), shortcut))
            }),
            TableCommandKind::iter().flat_map(|kind| {
                kind.kb_shortcuts(os)
                    .into_iter()
                    .map(move |shortcut| (format!("Table::{kind:?}"), shortcut))
            }),
        )
        .collect();

        // Server refresh and table refresh intentionally share the refresh shortcut:
        // `listen_for_kb_shortcuts` resolves both against the environment, and prefers
        // the more specific table refresh when an entry is being viewed.
        // Recording and table undo and redo share their shortcuts too, and an environment
        // never has both a recording and a table.
        let intentional_clash = |a: &str, b: &str| {
            [
                ("RedapServer::Refresh", "Table::Refresh"),
                ("Recording::UndoBlueprintEdit", "Table::UndoBlueprintEdit"),
                ("Recording::RedoBlueprintEdit", "Table::RedoBlueprintEdit"),
            ]
            .iter()
            .any(|pair| (a, b) == *pair || (b, a) == *pair)
        };

        for (a_name, a_shortcut) in &all_commands {
            for (b_name, b_shortcut) in &all_commands {
                if a_name == b_name || intentional_clash(a_name, b_name) {
                    continue;
                }
                assert!(
                    !clashes(*a_shortcut, *b_shortcut),
                    "Command '{a_name}' and '{b_name}' have overlapping keyboard shortcuts: {:?} vs {:?}",
                    a_shortcut.format(&egui::ModifierNames::NAMES, true),
                    b_shortcut.format(&egui::ModifierNames::NAMES, true),
                );
            }
        }
    }
}
