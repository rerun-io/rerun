//! This example shows how to wrap the Rerun Viewer in your own GUI.

use re_viewer::external::{
    arrow2, eframe, egui, re_arrow_store, re_data_store, re_log, re_log_types, re_memory, re_query,
};

// By using `re_memory::AccountingAllocator` Rerun can keep track of exactly how much memory it is using,
// and prune the data store when it goes above a certain limit.
// By using `mimalloc` we get faster allocations.
#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_native_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for TCP connections from Rerun's logging SDKs.
    // There are other ways of "feeding" the viewer though - all you need is a `re_smart_channel::Receiver`.
    let rx = re_sdk_comms::serve(
        "0.0.0.0",
        re_sdk_comms::DEFAULT_SERVER_PORT,
        Default::default(),
    )
    .await?;

    let native_options = eframe::NativeOptions {
        app_id: Some("my_app_id".to_owned()),
        ..re_viewer::native::eframe_options()
    };

    let startup_options = re_viewer::StartupOptions {
        memory_limit: re_memory::MemoryLimit {
            // Start pruning the data once we reach this much memory allocated
            limit: Some(12_000_000_000),
        },
        ..Default::default()
    };

    // This is used for analytics, if the `analytics` feature is on in `Cargo.toml`
    let app_env = re_viewer::AppEnvironment::Custom("My Wrapper".to_owned());

    let window_title = "My Customized Viewer";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());

            let re_ui = re_viewer::customize_eframe(cc);

            let rerun_app = re_viewer::App::from_receiver(
                re_viewer::build_info(),
                &app_env,
                startup_options,
                re_ui,
                cc.storage,
                rx,
            );

            Box::new(MyApp { rerun_app })
        }),
    )?;

    Ok(())
}

struct MyApp {
    rerun_app: re_viewer::App,
}

impl eframe::App for MyApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Store viewer state on disk
        self.rerun_app.save(storage);
    }

    /// Called whenever we need repainting, which could be 60 Hz.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // First add our panel(s):
        egui::SidePanel::right("my_side_panel")
            .default_width(200.0)
            .show(ctx, |ui| {
                self.ui(ui);
            });

        // Now show the Rerun Viewer in the remaining space:
        self.rerun_app.update(ctx, frame);
    }
}

impl MyApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.strong("My custom panel");
        });
        ui.separator();

        if let Some(log_db) = self.rerun_app.log_db() {
            log_db_ui(ui, log_db);
        } else {
            ui.label("No log database loaded yet.");
        }
    }
}

/// Show the content of the log database.
fn log_db_ui(ui: &mut egui::Ui, log_db: &re_data_store::LogDb) {
    if let Some(recording_info) = log_db.recording_info() {
        ui.label(format!("Application ID: {}", recording_info.application_id));
    }

    // There can be many timelines, but the `log_time` timeline is always there:
    let timeline = re_log_types::Timeline::log_time();

    ui.separator();

    ui.strong("Entities:");

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entity_path in log_db.entity_db.entity_paths() {
                ui.collapsing(entity_path.to_string(), |ui| {
                    entity_ui(ui, log_db, timeline, entity_path);
                });
            }
        });
}

fn entity_ui(
    ui: &mut egui::Ui,
    log_db: &re_data_store::LogDb,
    timeline: re_log_types::Timeline,
    entity_path: &re_log_types::EntityPath,
) {
    // Each entity can have many components (e.g. position, color, radius, …):
    if let Some(mut components) = log_db
        .entity_db
        .data_store
        .all_components(&timeline, entity_path)
    {
        components.sort(); // Make the order predicatable
        for component in components {
            ui.collapsing(component.to_string(), |ui| {
                component_ui(ui, log_db, timeline, entity_path, component);
            });
        }
    }
}

fn component_ui(
    ui: &mut egui::Ui,
    log_db: &re_data_store::LogDb,
    timeline: re_log_types::Timeline,
    entity_path: &re_log_types::EntityPath,
    component_name: re_log_types::ComponentName,
) {
    // You can query the data for any time point, but for now
    // just show the last value logged for each component:
    let query = re_arrow_store::LatestAtQuery::latest(timeline);

    if let Some((_, component)) = re_query::get_component_with_instances(
        &log_db.entity_db.data_store,
        &query,
        entity_path,
        component_name,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Iterate over all the instances (e.g. all the points in the point cloud):
                for instance_key in component.iter_instance_keys() {
                    if let Some(value) = component.lookup_arrow(&instance_key) {
                        ui.label(format_arrow(&*value));
                    }
                }
            });
    };
}

fn format_arrow(value: &dyn arrow2::array::Array) -> String {
    use re_log_types::SizeBytes as _;

    let bytes = value.total_size_bytes();
    if bytes < 256 {
        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(value, "null");
        if display(&mut string, 0).is_ok() {
            return string;
        }
    }

    // Fallback:
    format!("{bytes} bytes")
}
