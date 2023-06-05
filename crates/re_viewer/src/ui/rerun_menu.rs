//! The main Rerun drop-down menu found in the top panel.

use egui::NumExt as _;
use itertools::Itertools as _;

use re_log_types::StoreKind;
use re_ui::Command;
use re_viewer_context::AppOptions;

use crate::App;

pub fn rerun_menu_button_ui(ui: &mut egui::Ui, frame: &mut eframe::Frame, app: &mut App) {
    // let desired_icon_height = ui.max_rect().height() - 2.0 * ui.spacing_mut().button_padding.y;
    let desired_icon_height = ui.max_rect().height() - 4.0; // TODO(emilk): figure out this fudge
    let desired_icon_height = desired_icon_height.at_most(28.0); // figma size 2023-02-03

    let icon_image = app.re_ui().icon_image(&re_ui::icons::RERUN_MENU);
    let image_size = icon_image.size_vec2() * (desired_icon_height / icon_image.size_vec2().y);
    let texture_id = icon_image.texture_id(ui.ctx());

    ui.menu_image_button(texture_id, image_size, |ui| {
        ui.set_min_width(220.0);
        let spacing = 12.0;

        ui.menu_button("About", |ui| about_rerun_ui(ui, app.build_info()));

        ui.add_space(spacing);

        Command::ToggleCommandPalette.menu_button_ui(ui, &mut app.pending_commands);

        ui.add_space(spacing);

        #[cfg(not(target_arch = "wasm32"))]
        {
            Command::Open.menu_button_ui(ui, &mut app.pending_commands);

            save_buttons_ui(ui, app);

            ui.add_space(spacing);

            // On the web the browser controls the zoom
            let zoom_factor = app.app_options().zoom_factor;
            ui.weak(format!("Zoom {:.0}%", zoom_factor * 100.0))
                .on_hover_text("The zoom factor applied on top of the OS scaling factor.");
            Command::ZoomIn.menu_button_ui(ui, &mut app.pending_commands);
            Command::ZoomOut.menu_button_ui(ui, &mut app.pending_commands);
            ui.add_enabled_ui(zoom_factor != 1.0, |ui| {
                Command::ZoomReset.menu_button_ui(ui, &mut app.pending_commands)
            });

            Command::ToggleFullscreen.menu_button_ui(ui, &mut app.pending_commands);

            ui.add_space(spacing);
        }

        {
            Command::ResetViewer.menu_button_ui(ui, &mut app.pending_commands);

            #[cfg(not(target_arch = "wasm32"))]
            Command::OpenProfiler.menu_button_ui(ui, &mut app.pending_commands);

            Command::ToggleMemoryPanel.menu_button_ui(ui, &mut app.pending_commands);
        }

        ui.add_space(spacing);

        ui.menu_button("Recordings", |ui| {
            recordings_menu(ui, app);
        });

        ui.menu_button("Blueprints", |ui| {
            blueprints_menu(ui, app);
        });

        ui.menu_button("Options", |ui| {
            options_menu_ui(ui, frame, app.app_options_mut());
        });

        ui.add_space(spacing);
        ui.hyperlink_to(
            "Help",
            "https://www.rerun.io/docs/getting-started/viewer-walkthrough",
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.add_space(spacing);
            Command::Quit.menu_button_ui(ui, &mut app.pending_commands);
        }
    });
}

fn about_rerun_ui(ui: &mut egui::Ui, build_info: &re_build_info::BuildInfo) {
    let re_build_info::BuildInfo {
        crate_name,
        version,
        rustc_version,
        llvm_version,
        git_hash,
        git_branch: _,
        is_in_rerun_workspace: _,
        target_triple,
        datetime,
    } = *build_info;

    ui.style_mut().wrap = Some(false);

    let rustc_version = if rustc_version.is_empty() {
        "unknown"
    } else {
        rustc_version
    };

    let llvm_version = if llvm_version.is_empty() {
        "unknown"
    } else {
        llvm_version
    };

    let short_git_hash = &git_hash[..std::cmp::min(git_hash.len(), 7)];

    ui.label(format!(
        "{crate_name} {version} ({short_git_hash})\n\
        {target_triple}\n\
        rustc {rustc_version}\n\
        LLVM {llvm_version}\n\
        Built {datetime}",
    ));

    ui.add_space(12.0);
    ui.hyperlink_to("www.rerun.io", "https://www.rerun.io/");
}

fn recordings_menu(ui: &mut egui::Ui, app: &mut App) {
    let store_dbs = app
        .store_hub
        .recordings()
        .sorted_by_key(|store_db| store_db.store_info().map(|ri| ri.started))
        .collect_vec();

    if store_dbs.is_empty() {
        ui.weak("(empty)");
        return;
    }

    ui.style_mut().wrap = Some(false);
    for store_db in &store_dbs {
        let info = if let Some(store_info) = store_db.store_info() {
            format!(
                "{} - {}",
                store_info.application_id,
                store_info.started.format()
            )
        } else {
            "<UNKNOWN>".to_owned()
        };
        if ui
            .radio(
                app.state.recording_id().as_ref() == Some(store_db.store_id()),
                info,
            )
            .clicked()
        {
            app.state.set_recording_id(store_db.store_id().clone());
        }
    }
}

fn blueprints_menu(ui: &mut egui::Ui, app: &mut App) {
    let app_id = app.selected_app_id();
    let blueprint_dbs = app
        .store_hub
        .blueprints()
        .sorted_by_key(|store_db| store_db.store_info().map(|ri| ri.started))
        .filter(|log| {
            log.store_info()
                .map_or(false, |ri| ri.application_id == app_id)
        })
        .collect_vec();

    if blueprint_dbs.is_empty() {
        ui.weak("(empty)");
        return;
    }

    ui.style_mut().wrap = Some(false);
    for store_db in blueprint_dbs
        .iter()
        .filter(|log| log.store_kind() == StoreKind::Blueprint)
    {
        let info = if let Some(store_info) = store_db.store_info() {
            if store_info.is_app_default_blueprint() {
                format!("{} - Default Blueprint", store_info.application_id,)
            } else {
                format!(
                    "{} - {}",
                    store_info.application_id,
                    store_info.started.format()
                )
            }
        } else {
            "<UNKNOWN>".to_owned()
        };
        if ui
            .radio(
                app.state.selected_blueprint_by_app.get(&app_id) == Some(store_db.store_id()),
                info,
            )
            .clicked()
        {
            app.state
                .selected_blueprint_by_app
                .insert(app_id.clone(), store_db.store_id().clone());
        }
    }
}

fn options_menu_ui(ui: &mut egui::Ui, _frame: &mut eframe::Frame, options: &mut AppOptions) {
    ui.style_mut().wrap = Some(false);

    if ui
        .checkbox(&mut options.show_metrics, "Show performance metrics")
        .on_hover_text("Show metrics for milliseconds/frame and RAM usage in the top bar.")
        .clicked()
    {
        ui.close_menu();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui
            .checkbox(&mut options.experimental_space_view_screenshots, "(experimental) Space View screenshots")
            .on_hover_text("Allow taking screenshots of 2D & 3D space views via their context menu. Does not contain labels.")
            .clicked()
        {
            ui.close_menu();
        }
    }

    #[cfg(debug_assertions)]
    {
        ui.separator();
        ui.label("Debug:");

        egui_debug_options_ui(ui);
        ui.separator();
        debug_menu_options_ui(ui, options, _frame);
    }
}

// TODO(emilk): support saving data on web
#[cfg(not(target_arch = "wasm32"))]
fn save_buttons_ui(ui: &mut egui::Ui, app: &mut App) {
    let file_save_in_progress = app.background_tasks.is_file_save_in_progress();

    let save_button = Command::Save.menu_button(ui.ctx());
    let save_selection_button = Command::SaveSelection.menu_button(ui.ctx());

    if file_save_in_progress {
        ui.add_enabled_ui(false, |ui| {
            ui.horizontal(|ui| {
                ui.add(save_button);
                ui.spinner();
            });
            ui.horizontal(|ui| {
                ui.add(save_selection_button);
                ui.spinner();
            });
        });
    } else {
        let store_db_is_nonempty = app
            .recording_db()
            .map_or(false, |store_db| !store_db.is_empty());
        ui.add_enabled_ui(store_db_is_nonempty, |ui| {
            if ui
                .add(save_button)
                .on_hover_text("Save all data to a Rerun data file (.rrd)")
                .clicked()
            {
                ui.close_menu();
                app.pending_commands.push(Command::Save);
            }

            // We need to know the loop selection _before_ we can even display the
            // button, as this will determine whether its grayed out or not!
            // TODO(cmc): In practice the loop (green) selection is always there
            // at the moment so...
            let loop_selection = app.state.loop_selection();

            if ui
                .add_enabled(loop_selection.is_some(), save_selection_button)
                .on_hover_text(
                    "Save data for the current loop selection to a Rerun data file (.rrd)",
                )
                .clicked()
            {
                ui.close_menu();
                app.pending_commands.push(Command::SaveSelection);
            }
        });
    }
}

#[cfg(debug_assertions)]
fn egui_debug_options_ui(ui: &mut egui::Ui) {
    let mut debug = ui.style().debug;
    let mut any_clicked = false;

    any_clicked |= ui
        .checkbox(&mut debug.debug_on_hover, "Ui debug on hover")
        .on_hover_text("However over widgets to see their rectangles")
        .changed();
    any_clicked |= ui
        .checkbox(&mut debug.show_expand_width, "Show expand width")
        .on_hover_text("Show which widgets make their parent wider")
        .changed();
    any_clicked |= ui
        .checkbox(&mut debug.show_expand_height, "Show expand height")
        .on_hover_text("Show which widgets make their parent higher")
        .changed();
    any_clicked |= ui.checkbox(&mut debug.show_resize, "Show resize").changed();
    any_clicked |= ui
        .checkbox(
            &mut debug.show_interactive_widgets,
            "Show interactive widgets",
        )
        .on_hover_text("Show an overlay on all interactive widgets.")
        .changed();

    // This option currently causes the viewer to hang.
    // any_clicked |= ui
    //     .checkbox(&mut debug.show_blocking_widget, "Show blocking widgets")
    //     .on_hover_text("Show what widget blocks the interaction of another widget.")
    //     .changed();

    if any_clicked {
        let mut style = (*ui.ctx().style()).clone();
        style.debug = debug;
        ui.ctx().set_style(style);
    }
}

#[cfg(debug_assertions)]
fn debug_menu_options_ui(ui: &mut egui::Ui, options: &mut AppOptions, _frame: &mut eframe::Frame) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui.button("Mobile size").clicked() {
            // frame.set_window_size(egui::vec2(375.0, 812.0)); // iPhone 12 mini
            _frame.set_window_size(egui::vec2(375.0, 667.0)); //  iPhone SE 2nd gen
            _frame.set_fullscreen(false);
            ui.close_menu();
        }
        ui.separator();
    }

    if ui.button("Log info").clicked() {
        re_log::info!("Logging some info");
    }

    ui.checkbox(
        &mut options.show_picking_debug_overlay,
        "Picking Debug Overlay",
    )
    .on_hover_text("Show a debug overlay that renders the picking layer information using the `debug_overlay.wgsl` shader.");

    ui.menu_button("Crash", |ui| {
        #[allow(clippy::manual_assert)]
        if ui.button("panic!").clicked() {
            panic!("Intentional panic");
        }

        if ui.button("panic! during unwind").clicked() {
            struct PanicOnDrop {}

            impl Drop for PanicOnDrop {
                fn drop(&mut self) {
                    panic!("Second intentional panic in Drop::drop");
                }
            }

            let _this_will_panic_when_dropped = PanicOnDrop {};
            panic!("First intentional panic");
        }

        if ui.button("SEGFAULT").clicked() {
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/main/sadness-generator/src/lib.rs

            /// This is the fixed address used to generate a segfault. It's possible that
            /// this address can be mapped and writable by the your process in which case a
            /// crash may not occur
            #[cfg(target_pointer_width = "64")]
            pub const SEGFAULT_ADDRESS: u64 = u32::MAX as u64 + 0x42;
            #[cfg(target_pointer_width = "32")]
            pub const SEGFAULT_ADDRESS: u32 = 0x42;

            let bad_ptr: *mut u8 = SEGFAULT_ADDRESS as _;
            #[allow(unsafe_code)]
            // SAFETY: this is not safe. We are _trying_ to crash.
            unsafe {
                std::ptr::write_volatile(bad_ptr, 1);
            }
        }

        if ui.button("Stack overflow").clicked() {
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/main/sadness-generator/src/lib.rs
            fn recurse(data: u64) -> u64 {
                let mut buff = [0u8; 256];
                buff[..9].copy_from_slice(b"junk data");

                let mut result = data;
                for c in buff {
                    result += c as u64;
                }

                if result == 0 {
                    result
                } else {
                    recurse(result) + 1
                }
            }

            recurse(42);
        }
    });
}

// ---
