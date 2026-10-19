//! `AppKit` presentation of the shared viewer command catalog.

#![expect(unsafe_code)] // AppKit target/action callbacks require Objective-C interop.

use std::cell::RefCell;

use egui::{Key, KeyboardShortcut, os::OperatingSystem};
use objc2::rc::Retained;
use objc2::{DefinedClass as _, MainThreadOnly as _, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSDownArrowFunctionKey, NSEndFunctionKey, NSEventModifierFlags,
    NSF11FunctionKey, NSHomeFunctionKey, NSLeftArrowFunctionKey, NSMenu, NSMenuItem,
    NSMenuItemValidation, NSRightArrowFunctionKey, NSUpArrowFunctionKey,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use re_ui::{CommandKind, RedapServerCommandKind, UICommand};

use crate::command_catalog::{
    CommandContext, MenuGroup, MenuItem, catalog_commands, primary_shortcut,
};

struct MenuTargetState {
    commands: Vec<CommandKind>,
    pending: RefCell<Vec<CommandKind>>,
    ctx: RefCell<egui::Context>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. All ivars stay on the main thread.
    #[unsafe(super = NSObject)]
    #[name = "RerunCommandMenuTarget"]
    #[thread_kind = objc2::MainThreadOnly]
    #[ivars = MenuTargetState]
    struct MenuTarget;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for MenuTarget {}

    // SAFETY: The method implements AppKit's BOOL-returning validation selector.
    unsafe impl NSMenuItemValidation for MenuTarget {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            item.isEnabled()
        }
    }

    impl MenuTarget {
        // SAFETY: AppKit calls this action with the NSMenuItem that owns it.
        #[unsafe(method(runViewerCommand:))]
        fn run_command(&self, item: &NSMenuItem) {
            if let Ok(index) = usize::try_from(item.tag())
                && let Some(kind) = self.ivars().commands.get(index)
            {
                self.ivars().pending.borrow_mut().push(*kind);
                self.ivars().ctx.borrow().request_repaint();
            }
        }
    }
);

struct MenuEntry {
    kind: CommandKind,
    item: Retained<NSMenuItem>,
    shortcut: Option<KeyboardShortcut>,
    shortcut_active: bool,
}

struct NativeMenu {
    // AppKit does not retain menu-item targets; this owns the target for the menu's lifetime.
    target: Retained<MenuTarget>,
    entries: Vec<MenuEntry>,
    groups: Vec<NativeMenuGroup>,
}

struct NativeMenuGroup {
    kind: MenuGroup,
    item: Retained<NSMenuItem>,
    dividers: Vec<(usize, Retained<NSMenuItem>)>,
}

thread_local! {
    static MENU: RefCell<Option<NativeMenu>> = const { RefCell::new(None) };
}

/// Commands chosen from the menu since the last call.
///
/// Must be drained from `App::logic`, which eframe keeps calling while the window is minimized
/// or occluded, so that e.g. Quit works without a visible window.
pub fn take_pending() -> Vec<CommandKind> {
    MENU.with_borrow(|menu| {
        menu.as_ref()
            .map(|menu| menu.target.ivars().pending.take())
            .unwrap_or_default()
    })
}

pub fn update(ctx: &egui::Context, commands: &CommandContext) {
    re_tracing::profile_function!();

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    MENU.with_borrow_mut(|menu| {
        if menu.is_none() {
            *menu = NativeMenu::new(mtm, ctx);
        }
        if let Some(menu) = menu {
            menu.update(ctx, commands);
        }
    });
}

impl NativeMenu {
    fn new(mtm: MainThreadMarker, ctx: &egui::Context) -> Option<Self> {
        let app = NSApplication::sharedApplication(mtm);
        let menubar = app.mainMenu()?;
        let application_item = menubar.itemAtIndex(0)?;
        let application_menu = application_item.submenu()?;

        let kinds: Vec<_> = catalog_commands().collect();
        let target = MenuTarget::alloc(mtm).set_ivars(MenuTargetState {
            commands: kinds.clone(),
            pending: RefCell::default(),
            ctx: RefCell::new(ctx.clone()),
        });
        // SAFETY: NSObject's init initializes the allocated subclass and retains its ivars.
        let target: Retained<MenuTarget> = unsafe { msg_send![super(target), init] };

        // AppKit owns the Services and visibility actions; keep their targets and internal dividers.
        let mut application_items = Vec::new();
        for item in application_menu.itemArray() {
            if matches!(item.action(), Some(action) if action == sel!(orderFrontStandardAboutPanel:) || action == sel!(terminate:))
                || (application_items.is_empty() && item.isSeparatorItem())
            {
                continue;
            }
            application_items.push(item);
        }
        while application_items
            .last()
            .is_some_and(|item| item.isSeparatorItem())
        {
            application_items.pop();
        }
        application_menu.removeAllItems();

        let mut entries = Vec::new();
        let mut groups = Vec::new();
        for group in MenuGroup::ALL {
            let (menu, group_item) = if group == MenuGroup::Application {
                (application_menu.clone(), application_item.clone())
            } else {
                let menu = NSMenu::new(mtm);
                menu.setTitle(&NSString::from_str(group.text()));
                menu.setAutoenablesItems(false);
                let item = NSMenuItem::new(mtm);
                item.setTitle(&NSString::from_str(group.text()));
                item.setSubmenu(Some(&menu));
                menubar.addItem(&item);
                (menu, item)
            };

            let mut dividers = Vec::new();
            for (index, &item) in group.items().iter().enumerate() {
                let kind = match item {
                    MenuItem::Command(kind) => kind,
                    MenuItem::Separator => {
                        let divider = NSMenuItem::separatorItem(mtm);
                        menu.addItem(&divider);
                        dividers.push((index, divider));
                        continue;
                    }
                };
                if kind == CommandKind::Ui(UICommand::Quit) && !application_items.is_empty() {
                    for item in &application_items {
                        menu.addItem(item);
                    }
                    menu.addItem(&NSMenuItem::separatorItem(mtm));
                }
                // SAFETY: The action signature matches MenuTarget::run_command, and target is
                // retained in NativeMenu for as long as these menu items can invoke it.
                let item = unsafe {
                    let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &NSString::from_str(kind.text()),
                        Some(sel!(runViewerCommand:)),
                        &NSString::new(),
                    );
                    item.setTarget(Some(&target));
                    item
                };
                // Tags index into `catalog_commands()`, which yields commands in this same order.
                // Out-of-range tags are ignored by `run_command`.
                re_log::debug_assert_eq!(kinds.get(entries.len()), Some(&kind));
                item.setTag(entries.len().try_into().unwrap_or(-1));
                item.setToolTip(Some(&NSString::from_str(kind.tooltip())));
                menu.addItem(&item);
                entries.push(MenuEntry {
                    kind,
                    item,
                    shortcut: primary_shortcut(kind, OperatingSystem::Mac),
                    shortcut_active: false,
                });
            }
            groups.push(NativeMenuGroup {
                kind: group,
                item: group_item,
                dividers,
            });
        }

        Some(Self {
            target,
            entries,
            groups,
        })
    }

    fn update(&mut self, ctx: &egui::Context, commands: &CommandContext) {
        re_tracing::profile_function!();
        self.target.ivars().ctx.replace(ctx.clone());

        let editing_text = ctx.text_edit_focused();
        for entry in &mut self.entries {
            set_hidden(&entry.item, !commands.is_visible(entry.kind));
            let enabled = commands.resolve(entry.kind).is_some();
            if entry.item.isEnabled() != enabled {
                entry.item.setEnabled(enabled);
            }

            let shortcut_active = entry.shortcut.is_some_and(|shortcut| {
                enabled && !(editing_text && re_ui::conflicts_with_text_editing(&shortcut))
                    // Table refresh and server refresh share a shortcut; the more specific target wins.
                    && !(entry.kind == CommandKind::RedapServer(RedapServerCommandKind::Refresh)
                        && commands.environment.table.is_some())
            });
            if entry.shortcut_active != shortcut_active {
                entry.shortcut_active = shortcut_active;
                if let Some(shortcut) = entry.shortcut.filter(|_| shortcut_active) {
                    let (key, modifiers) = key_equivalent(shortcut);
                    entry.item.setKeyEquivalent(&NSString::from_str(&key));
                    entry.item.setKeyEquivalentModifierMask(modifiers);
                } else {
                    entry.item.setKeyEquivalent(&NSString::new());
                }
            }
        }

        for group in &self.groups {
            let visible_indices: Vec<_> = commands
                .visible_items(group.kind)
                .map(|(index, _)| index)
                .collect();
            set_hidden(&group.item, visible_indices.is_empty());
            for (index, divider) in &group.dividers {
                set_hidden(divider, !visible_indices.contains(index));
            }
        }
    }
}

fn set_hidden(item: &NSMenuItem, hidden: bool) {
    if item.isHidden() != hidden {
        item.setHidden(hidden);
    }
}

fn key_equivalent(shortcut: KeyboardShortcut) -> (String, NSEventModifierFlags) {
    let key = match shortcut.logical_key {
        Key::ArrowUp => function_key(NSUpArrowFunctionKey),
        Key::ArrowDown => function_key(NSDownArrowFunctionKey),
        Key::ArrowLeft => function_key(NSLeftArrowFunctionKey),
        Key::ArrowRight => function_key(NSRightArrowFunctionKey),
        Key::Home => function_key(NSHomeFunctionKey),
        Key::End => function_key(NSEndFunctionKey),
        Key::F11 => function_key(NSF11FunctionKey),
        Key::Space => " ".to_owned(),
        key => key.symbol_or_name().to_lowercase(),
    };
    let mut modifiers = NSEventModifierFlags::empty();
    if shortcut.modifiers.command || shortcut.modifiers.mac_cmd {
        modifiers |= NSEventModifierFlags::Command;
    }
    if shortcut.modifiers.ctrl {
        modifiers |= NSEventModifierFlags::Control;
    }
    if shortcut.modifiers.alt {
        modifiers |= NSEventModifierFlags::Option;
    }
    if shortcut.modifiers.shift {
        modifiers |= NSEventModifierFlags::Shift;
    }
    (key, modifiers)
}

/// `AppKit` identifies non-character keys by private-use code points in key equivalents.
fn function_key(code: u32) -> String {
    char::from_u32(code).map(String::from).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_shortcuts_preserve_modifiers_and_special_keys() {
        let (key, modifiers) = key_equivalent(KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            Key::S,
        ));
        assert_eq!(key, "s");
        assert_eq!(
            modifiers,
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        let (key, modifiers) = key_equivalent(KeyboardShortcut::new(
            egui::Modifiers::CTRL | egui::Modifiers::ALT,
            Key::ArrowLeft,
        ));
        assert_eq!(key, "\u{f702}");
        assert_eq!(
            modifiers,
            NSEventModifierFlags::Control | NSEventModifierFlags::Option
        );
        assert_eq!(
            key_equivalent(KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::Comma)).0,
            ","
        );
    }
}
