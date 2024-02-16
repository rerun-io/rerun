//! The main Rerun drop-down menu found in the top panel.

use egui::NumExt as _;

use re_log_types::TimeZone;
use re_ui::{ReUi, UICommand};
use re_viewer_context::{StoreContext, SystemCommand, SystemCommandSender};

use crate::App;

const SPACING: f32 = 12.0;

impl App {
    pub fn rerun_menu_button_ui(
        &mut self,
        frame: &eframe::Frame,
        _store_context: Option<&StoreContext<'_>>,
        ui: &mut egui::Ui,
    ) {
        // let desired_icon_height = ui.max_rect().height() - 2.0 * ui.spacing_mut().button_padding.y;
        let desired_icon_height = ui.max_rect().height() - 4.0; // TODO(emilk): figure out this fudge
        let desired_icon_height = desired_icon_height.at_most(28.0); // figma size 2023-02-03

        let image = re_ui::icons::RERUN_MENU
            .as_image()
            .max_height(desired_icon_height);

        ui.menu_image_button(image, |ui| {
            ui.set_min_width(220.0);

            ui.menu_button("About", |ui| self.about_rerun_ui(frame, ui));

            ui.add_space(SPACING);

            UICommand::ToggleCommandPalette.menu_button_ui(ui, &self.command_sender);

            ui.add_space(SPACING);

            UICommand::Open.menu_button_ui(ui, &self.command_sender);

            #[cfg(not(target_arch = "wasm32"))]
            {
                self.save_buttons_ui(ui, _store_context);

                UICommand::CloseCurrentRecording.menu_button_ui(ui, &self.command_sender);

                ui.add_space(SPACING);

                // On the web the browser controls the zoom
                let zoom_factor = ui.ctx().zoom_factor();
                ui.weak(format!("Current zoom: {:.0}%", zoom_factor * 100.0))
                    .on_hover_text(
                        "The UI zoom level on top of the operating system's default value",
                    );
                UICommand::ZoomIn.menu_button_ui(ui, &self.command_sender);
                UICommand::ZoomOut.menu_button_ui(ui, &self.command_sender);
                ui.add_enabled_ui(zoom_factor != 1.0, |ui| {
                    UICommand::ZoomReset.menu_button_ui(ui, &self.command_sender)
                });

                UICommand::ToggleFullscreen.menu_button_ui(ui, &self.command_sender);

                ui.add_space(SPACING);
            }

            {
                UICommand::ResetViewer.menu_button_ui(ui, &self.command_sender);

                #[cfg(not(target_arch = "wasm32"))]
                UICommand::OpenProfiler.menu_button_ui(ui, &self.command_sender);

                UICommand::ToggleMemoryPanel.menu_button_ui(ui, &self.command_sender);

                #[cfg(debug_assertions)]
                UICommand::ToggleEguiDebugPanel.menu_button_ui(ui, &self.command_sender);
            }

            ui.add_space(SPACING);

            ui.menu_button("Options", |ui| {
                ui.style_mut().wrap = Some(false);
                options_menu_ui(
                    &self.command_sender,
                    &self.re_ui,
                    ui,
                    frame,
                    &mut self.state.app_options,
                );
            });

            #[cfg(debug_assertions)]
            ui.menu_button("Debug", |ui| {
                ui.style_mut().wrap = Some(false);
                debug_menu_options_ui(
                    &self.re_ui,
                    ui,
                    &mut self.state.app_options,
                    &self.command_sender,
                );

                ui.label("egui debug options:");
                egui_debug_options_ui(&self.re_ui, ui);
            });

            ui.add_space(SPACING);

            UICommand::OpenWebHelp.menu_button_ui(ui, &self.command_sender);
            UICommand::OpenRerunDiscord.menu_button_ui(ui, &self.command_sender);

            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.add_space(SPACING);
                UICommand::Quit.menu_button_ui(ui, &self.command_sender);
            }
        });
    }

    fn about_rerun_ui(&self, frame: &eframe::Frame, ui: &mut egui::Ui) {
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
        } = *self.build_info();

        ui.style_mut().wrap = Some(false);

        let git_hash_suffix = if git_hash.is_empty() {
            String::new()
        } else {
            let short_git_hash = &git_hash[..std::cmp::min(git_hash.len(), 7)];
            format!("({short_git_hash})")
        };

        let mut label = format!(
            "{crate_name} {version} {git_hash_suffix}\n\
            {target_triple}"
        );

        if !rustc_version.is_empty() {
            label += &format!("\nrustc {rustc_version}");
            if !llvm_version.is_empty() {
                label += &format!(", LLVM {llvm_version}");
            }
        }

        if !datetime.is_empty() {
            label += &format!("\nbuilt {datetime}");
        }

        ui.label(label);

        if let Some(render_state) = frame.wgpu_render_state() {
            render_state_ui(ui, render_state);
        }
    }

    // TODO(emilk): support saving data on web
    #[cfg(not(target_arch = "wasm32"))]
    fn save_buttons_ui(&mut self, ui: &mut egui::Ui, store_view: Option<&StoreContext<'_>>) {
        use re_ui::UICommandSender;

        let file_save_in_progress = self.background_tasks.is_file_save_in_progress();

        let save_button = UICommand::Save.menu_button(ui.ctx());
        let save_selection_button = UICommand::SaveSelection.menu_button(ui.ctx());
        let save_blueprint = UICommand::SaveBlueprint.menu_button(ui.ctx());

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
                ui.horizontal(|ui| {
                    ui.add(save_blueprint);
                    ui.spinner();
                });
            });
        } else {
            let entity_db_is_nonempty = store_view
                .and_then(|view| view.recording)
                .map_or(false, |recording| !recording.is_empty());
            ui.add_enabled_ui(entity_db_is_nonempty, |ui| {
                if ui
                    .add(save_button)
                    .on_hover_text("Save all data to a Rerun data file (.rrd)")
                    .clicked()
                {
                    ui.close_menu();
                    self.command_sender.send_ui(UICommand::Save);
                }

                // We need to know the loop selection _before_ we can even display the
                // button, as this will determine whether its grayed out or not!
                // TODO(cmc): In practice the loop (green) selection is always there
                // at the moment so…
                let loop_selection = self.state.loop_selection(store_view);

                if ui
                    .add_enabled(loop_selection.is_some(), save_selection_button)
                    .on_hover_text(
                        "Save data for the current loop selection to a Rerun data file (.rrd)",
                    )
                    .clicked()
                {
                    ui.close_menu();
                    self.command_sender.send_ui(UICommand::SaveSelection);
                }

                if ui
                    .add(save_blueprint)
                    .on_hover_text(
                        "Save the current blueprint to a Rerun blueprint file (.blueprint)",
                    )
                    .clicked()
                {
                    ui.close_menu();
                    self.command_sender.send_ui(UICommand::SaveBlueprint);
                }
            });
        }
    }
}

fn render_state_ui(ui: &mut egui::Ui, render_state: &egui_wgpu::RenderState) {
    let wgpu_adapter_details_ui = |ui: &mut egui::Ui, adapter: &eframe::wgpu::Adapter| {
        let info = &adapter.get_info();

        let wgpu::AdapterInfo {
            name,
            vendor,
            device,
            device_type,
            driver,
            driver_info,
            backend,
        } = &info;

        // Example values:
        // > name: "llvmpipe (LLVM 16.0.6, 256 bits)", device_type: Cpu, backend: Vulkan, driver: "llvmpipe", driver_info: "Mesa 23.1.6-arch1.4 (LLVM 16.0.6)"
        // > name: "Apple M1 Pro", device_type: IntegratedGpu, backend: Metal, driver: "", driver_info: ""
        // > name: "ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)", device_type: IntegratedGpu, backend: Gl, driver: "", driver_info: ""

        egui::Grid::new("adapter_info").show(ui, |ui| {
            ui.label("Backend");
            ui.label(backend.to_str()); // TODO(wgpu#5170): Use std::fmt::Display for backend.
            ui.end_row();

            ui.label("Device Type");
            ui.label(format!("{device_type:?}"));
            ui.end_row();

            if !name.is_empty() {
                ui.label("Name");
                ui.label(format!("{name:?}"));
                ui.end_row();
            }
            if !driver.is_empty() {
                ui.label("Driver");
                ui.label(format!("{driver:?}"));
                ui.end_row();
            }
            if !driver_info.is_empty() {
                ui.label("Driver info");
                ui.label(format!("{driver_info:?}"));
                ui.end_row();
            }
            if *vendor != 0 {
                // TODO(emilk): decode using https://github.com/gfx-rs/wgpu/blob/767ac03245ee937d3dc552edc13fe7ab0a860eec/wgpu-hal/src/auxil/mod.rs#L7
                ui.label("Vendor");
                ui.label(format!("0x{vendor:04X}"));
                ui.end_row();
            }
            if *device != 0 {
                ui.label("Device");
                ui.label(format!("0x{device:02X}"));
                ui.end_row();
            }
        });
    };

    let wgpu_adapter_ui = |ui: &mut egui::Ui, adapter: &eframe::wgpu::Adapter| {
        let info = &adapter.get_info();
        // TODO(wgpu#5170): Use std::fmt::Display for backend.
        ui.label(info.backend.to_str()).on_hover_ui(|ui| {
            wgpu_adapter_details_ui(ui, adapter);
        });
    };

    egui::Grid::new("wgpu_info").num_columns(2).show(ui, |ui| {
        ui.label("Rendering backend:");
        wgpu_adapter_ui(ui, &render_state.adapter);
        ui.end_row();

        #[cfg(not(target_arch = "wasm32"))]
        if 1 < render_state.available_adapters.len() {
            ui.label("Other rendering backends:");
            ui.vertical(|ui| {
                for adapter in &*render_state.available_adapters {
                    if adapter.get_info() != render_state.adapter.get_info() {
                        wgpu_adapter_ui(ui, adapter);
                    }
                }
            });
            ui.end_row();
        }
    });
}

fn options_menu_ui(
    command_sender: &re_viewer_context::CommandSender,
    re_ui: &ReUi,
    ui: &mut egui::Ui,
    frame: &eframe::Frame,
    app_options: &mut re_viewer_context::AppOptions,
) {
    re_ui
        .checkbox(
            ui,
            &mut app_options.show_metrics,
            "Show performance metrics",
        )
        .on_hover_text("Show metrics for milliseconds/frame and RAM usage in the top bar");

    ui.horizontal(|ui| {
        ui.label("Timezone:");
        re_ui
            .radio_value(ui, &mut app_options.time_zone, TimeZone::Utc, "UTC")
            .on_hover_text("Display timestamps in UTC");
        re_ui
            .radio_value(ui, &mut app_options.time_zone, TimeZone::Local, "Local")
            .on_hover_text("Display timestamps in the local timezone");
    });

    {
        ui.add_space(SPACING);
        ui.label("Experimental features:");
        experimental_feature_ui(command_sender, re_ui, ui, app_options);
    }

    if let Some(_backend) = frame
        .wgpu_render_state()
        .map(|state| state.adapter.get_info().backend)
    {
        // Adapter switching only implemented for web so far.
        // For native it's less well defined since the application may be embedded in another application that reads arguments differently.
        #[cfg(target_arch = "wasm32")]
        {
            ui.add_space(SPACING);
            if _backend == wgpu::Backend::BrowserWebGpu {
                UICommand::RestartWithWebGl.menu_button_ui(ui, command_sender);
            } else {
                UICommand::RestartWithWebGpu.menu_button_ui(ui, command_sender);
            }
        }
    }
}

fn experimental_feature_ui(
    command_sender: &re_viewer_context::CommandSender,
    re_ui: &ReUi,
    ui: &mut egui::Ui,
    app_options: &mut re_viewer_context::AppOptions,
) {
    #[cfg(not(target_arch = "wasm32"))]
    re_ui
        .checkbox(ui, &mut app_options.experimental_space_view_screenshots, "Space View screenshots")
        .on_hover_text("Allow taking screenshots of 2D and 3D Space Views via their context menu. Does not contain labels.");

    if re_ui
        .checkbox(
            ui,
            &mut app_options.experimental_dataframe_space_view,
            "Dataframe Space View",
        )
        .on_hover_text("Enable the experimental dataframe space view.")
        .clicked()
    {
        command_sender.send_system(SystemCommand::EnableExperimentalDataframeSpaceView(
            app_options.experimental_dataframe_space_view,
        ));
    }

    re_ui
        .checkbox(
            ui,
            &mut app_options.experimental_entity_filter_editor,
            "Entity filter DSL",
        )
        .on_hover_text("Show an entity filter DSL when selecting a space-view.");

    re_ui
        .checkbox(
            ui,
            &mut app_options.experimental_plot_query_clamping,
            "Plots: query clamping",
        )
        .on_hover_text("Toggle query clamping for the plot visualizers.");
}

#[cfg(debug_assertions)]
fn egui_debug_options_ui(re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
    let mut debug = ui.style().debug;
    let mut any_clicked = false;

    any_clicked |= re_ui
        .checkbox(ui, &mut debug.debug_on_hover, "Ui debug on hover")
        .on_hover_text("However over widgets to see their rectangles")
        .changed();
    any_clicked |= re_ui
        .checkbox(ui, &mut debug.show_expand_width, "Show expand width")
        .on_hover_text("Show which widgets make their parent wider")
        .changed();
    any_clicked |= re_ui
        .checkbox(ui, &mut debug.show_expand_height, "Show expand height")
        .on_hover_text("Show which widgets make their parent higher")
        .changed();
    any_clicked |= re_ui
        .checkbox(ui, &mut debug.show_resize, "Show resize")
        .changed();
    any_clicked |= re_ui
        .checkbox(
            ui,
            &mut debug.show_interactive_widgets,
            "Show interactive widgets",
        )
        .on_hover_text("Show an overlay on all interactive widgets")
        .changed();
    any_clicked |= re_ui
        .checkbox(ui, &mut debug.show_blocking_widget, "Show blocking widgets")
        .on_hover_text("Show what widget blocks the interaction of another widget")
        .changed();

    if any_clicked {
        let mut style = (*ui.ctx().style()).clone();
        style.debug = debug;
        ui.ctx().set_style(style);
    }
}

#[cfg(debug_assertions)]
use re_viewer_context::CommandSender;

#[cfg(debug_assertions)]
fn debug_menu_options_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    app_options: &mut re_viewer_context::AppOptions,
    command_sender: &CommandSender,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if ui.button("Mobile size").clicked() {
            // let size = egui::vec2(375.0, 812.0); // iPhone 12 mini
            let size = egui::vec2(375.0, 667.0); //  iPhone SE 2nd gen
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ui.close_menu();
        }
    }

    if ui.button("Log something at INFO level").clicked() {
        re_log::info!("Logging some info");
    }

    UICommand::ToggleBlueprintInspectionPanel.menu_button_ui(ui, command_sender);

    ui.horizontal(|ui| {
        ui.label("Blueprint GC:");
        re_ui.radio_value(ui, &mut app_options.blueprint_gc, true, "Enabled");
        re_ui.radio_value(ui, &mut app_options.blueprint_gc, false, "Disabled");
    });

    re_ui.checkbox(ui,
        &mut app_options.show_picking_debug_overlay,
        "Picking Debug Overlay",
    ).on_hover_text("Show a debug overlay that renders the picking layer information using the `debug_overlay.wgsl` shader.");

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
