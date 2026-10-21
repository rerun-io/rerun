//! Contents for application-level modals.

use std::fmt::Write as _;

#[cfg(debug_assertions)]
use re_ui::RecordingCommandKind;

const SPACING: f32 = 12.0;

/// The About UI serves two purposes:
///
/// A) Tell users about what Rerun is, in case they just stumbled upon it online.
/// B) Show detailed build information, that can be used when reporting bugs.
pub fn about_rerun_ui(
    ui: &mut egui::Ui,
    build_info: &re_build_info::BuildInfo,
    render_state: Option<&egui_wgpu::RenderState>,
) {
    let re_build_info::BuildInfo {
        crate_name,
        features,
        version,
        rustc_version,
        llvm_version,
        git_hash,
        git_branch: _,
        is_in_rerun_workspace: _,
        target_triple,
        datetime,
        is_debug_build,
    } = build_info;

    ui.set_max_width(400.0);

    let logo_size = 68.0;

    ui.horizontal(|ui|{
        ui.add(
            re_ui::icons::RERUN_LOGO
                .as_image()
                .fit_to_exact_size(egui::Vec2::splat(logo_size))
                .corner_radius(4.0)
                .alt_text("Rerun"),
        );

        ui.vertical(|ui|{
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                "Rerun is a toolchain for robotics and physical AI that makes it easy to log, query, visualize, and train on multi-rate, multimodal data.",
            );

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("Learn more at ");
                ui.hyperlink_to("rerun.io", "https://rerun.io/");
                ui.label(".");
            });
        });
    });

    ui.add_space(SPACING);

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    let version = {
        let git_hash_suffix = if git_hash.is_empty() {
            String::new()
        } else {
            let short_git_hash = &git_hash[..std::cmp::min(git_hash.len(), 7)];
            format!(" ({short_git_hash})")
        };

        let debug_label = if *is_debug_build { " (debug)" } else { "" };

        format!("{version}{git_hash_suffix}{debug_label}")
    };

    egui::Grid::new("build_info").num_columns(2).show(ui, |ui| {
        ui.label("Crate");
        ui.label(crate_name.as_ref());
        ui.end_row();

        ui.label("Version");
        ui.label(version);
        ui.end_row();

        if !datetime.is_empty() {
            ui.label("Built");
            ui.label(datetime.as_ref());
            ui.end_row();
        }

        // It is really the features of `rerun-cli` (the `rerun` binary) that are interesting.
        // For the web-viewer (`crate_name: "re_viewer"`) it is much less interesting.
        if crate_name == "rerun-cli" && !features.is_empty() {
            ui.label("Features");
            ui.label(features.as_ref());
            ui.end_row();
        }

        ui.label("Platform");
        ui.label(target_triple.as_ref());
        ui.end_row();

        if !rustc_version.is_empty() {
            ui.label("Compiler");
            let mut compiler = format!("rustc {rustc_version}");
            if !llvm_version.is_empty() {
                write!(compiler, ", LLVM {llvm_version}").ok();
            }
            ui.label(compiler);
            ui.end_row();
        }
    });

    if let Some(render_state) = render_state {
        ui.add_space(SPACING);
        render_state_ui(ui, render_state);
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
            device_pci_bus_id: _,
            driver,
            driver_info,
            backend,
            subgroup_min_size: _,
            subgroup_max_size: _,
            transient_saves_memory: _,
            limit_bucket: _,
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
            ui.label(match device_type {
                wgpu::DeviceType::Other => "Other",
                wgpu::DeviceType::IntegratedGpu => "Integrated GPU",
                wgpu::DeviceType::DiscreteGpu => "Discrete GPU",
                wgpu::DeviceType::VirtualGpu => "Virtual GPU",
                wgpu::DeviceType::Cpu => "CPU",
            });
            ui.end_row();

            if !name.is_empty() {
                ui.label("Name");
                ui.label(name);
                ui.end_row();
            }
            if !driver.is_empty() {
                ui.label("Driver");
                ui.label(driver);
                ui.end_row();
            }
            if !driver_info.is_empty() {
                ui.label("Driver info");
                ui.label(driver_info);
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
        ui.label("Rendering backend");
        wgpu_adapter_ui(ui, &render_state.adapter);
        ui.end_row();

        #[cfg(not(target_arch = "wasm32"))]
        if 1 < render_state.available_adapters.len() {
            ui.label("Other rendering backends");
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

#[cfg(debug_assertions)]
fn egui_debug_options_ui(ui: &mut egui::Ui) {
    use re_ui::UiExt as _;

    let mut debug = ui.style().debug;
    let mut any_clicked = false;

    any_clicked |= ui
        .re_checkbox(&mut debug.debug_on_hover, "Ui debug on hover")
        .on_hover_text("However over widgets to see their rectangles")
        .changed();
    any_clicked |= ui
        .re_checkbox(&mut debug.show_expand_width, "Show expand width")
        .on_hover_text("Show which widgets make their parent wider")
        .changed();
    any_clicked |= ui
        .re_checkbox(&mut debug.show_expand_height, "Show expand height")
        .on_hover_text("Show which widgets make their parent higher")
        .changed();
    any_clicked |= ui
        .re_checkbox(&mut debug.show_resize, "Show resize")
        .changed();
    any_clicked |= ui
        .re_checkbox(
            &mut debug.show_interactive_widgets,
            "Show interactive widgets",
        )
        .on_hover_text("Show an overlay on all interactive widgets")
        .changed();

    if any_clicked {
        let mut style = (*ui.global_style()).clone();
        style.debug = debug;
        ui.set_global_style(style);
    }
}

#[cfg(debug_assertions)]
use re_viewer_context::CommandSender;

#[cfg(debug_assertions)]
fn debug_menu_options_ui(
    ui: &mut egui::Ui,
    app_options: &mut re_viewer_context::AppOptions,
    active_recording_id: Option<&re_log_types::StoreId>,
    command_sender: &CommandSender,
) {
    use re_ui::UiExt as _;

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.horizontal(|ui| {
            ui.label("Command line");
            ui.monospace(std::env::args().collect::<Vec<_>>().join(" "));
        });

        if ui.button("Mobile size").clicked() {
            // let size = egui::vec2(375.0, 812.0); // iPhone 12 mini
            let size = egui::vec2(375.0, 667.0); //  iPhone SE 2nd gen
            ui.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            ui.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ui.close();
        }
    }

    if ui.button("Log something at INFO level").clicked() {
        re_log::info!("Logging some info");
    }

    if ui.button("Log an error").clicked() {
        // The same shape a failed server call arrives in: a summary, and the rest as details.
        let err =
            re_error::StructuredError::parse("invalid lance input (Internal)").with_details([
                "Server: rerun://example.com:443",
                "Endpoint: /GetTableSchema",
                "trace-id: ad66019921fce81f3f56462f9a8dbd63",
                "dataset url: file:///path/to/file",
                r#"metadata: {"x-request-trace-id": "ad66019921fce81f3f56462f9a8dbd63"}"#,
            ]);
        re_log::error!("{err}");
    }

    RecordingCommandKind::ToggleBlueprintInspectionPanel.menu_button_ui(
        ui,
        active_recording_id,
        command_sender,
    );

    ui.re_checkbox(&mut app_options.show_metrics, "Show top bar diagnostics")
        .on_hover_text(
            "Show the debug-build warning, milliseconds/frame, RAM usage, FPS, and connection \
         diagnostics in the top bar",
        );

    ui.horizontal(|ui| {
        ui.label("Blueprint GC");
        ui.re_radio_value(&mut app_options.blueprint_gc, true, "Enabled");
        ui.re_radio_value(&mut app_options.blueprint_gc, false, "Disabled");
    });

    ui.re_checkbox(
        &mut app_options.show_picking_debug_overlay,
        "Picking Debug Overlay",
    )
    .on_hover_text(
        "Show a debug overlay that renders the picking layer information using the \
        `debug_overlay.wgsl` shader.",
    );

    ui.menu_button("Crash", |ui| {
        #[expect(clippy::manual_assert)]
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
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/065f3dd9c1c318630e539375165cf74961b44bcc/sadness-generator/src/lib.rs

            /// This is the fixed address used to generate a segfault. It's possible that
            /// this address can be mapped and writable by the your process in which case a
            /// crash may not occur
            #[cfg(target_pointer_width = "64")]
            pub const SEGFAULT_ADDRESS: u64 = u32::MAX as u64 + 0x42;
            #[cfg(target_pointer_width = "32")]
            pub const SEGFAULT_ADDRESS: u32 = 0x42;

            let bad_ptr: *mut u8 = SEGFAULT_ADDRESS as _;
            #[expect(unsafe_code)]
            // SAFETY: this is not safe. We are _trying_ to crash.
            unsafe {
                std::ptr::write_volatile(bad_ptr, 1);
            }
        }

        if ui.button("Stack overflow").clicked() {
            // Taken from https://github.com/EmbarkStudios/crash-handling/blob/065f3dd9c1c318630e539375165cf74961b44bcc/sadness-generator/src/lib.rs
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

#[cfg(debug_assertions)]
pub fn debug_tools_ui(
    ui: &mut egui::Ui,
    app_options: &mut re_viewer_context::AppOptions,
    active_recording_id: Option<&re_log_types::StoreId>,
    command_sender: &CommandSender,
) {
    debug_menu_options_ui(ui, app_options, active_recording_id, command_sender);

    ui.add_space(SPACING);
    ui.label("egui debug options");
    ui.weak(format!("pixels_per_point: {}", ui.pixels_per_point()));
    egui_debug_options_ui(ui);
}
