//! This example shows how to wrap the Rerun Viewer in your own GUI.

use rerun::external::{
    arrow, eframe, egui, re_chunk_store, re_crash_handler, re_entity_db, re_grpc_server, re_log,
    re_log_types, re_memory, re_sdk_types, re_viewer, tokio,
};

// By using `re_memory::AccountingAllocator` Rerun can keep track of exactly how much memory it is using,
// and prune the data store when it goes above a certain limit.
// By using `mimalloc` we get faster allocations.
#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_thread_token = re_viewer::MainThreadToken::i_promise_i_am_on_the_main_thread();

    // Direct calls using the `log` crate to stderr. Control with `RUST_LOG=debug` etc.
    re_log::setup_logging();

    // Install handlers for panics and crashes that prints to stderr and send
    // them to Rerun analytics (if the `analytics` feature is on in `Cargo.toml`).
    re_crash_handler::install_crash_handlers(re_viewer::build_info());

    // Listen for gRPC connections from Rerun's logging SDKs.
    // There are other ways of "feeding" the viewer though - all you need is a `re_log_channel::LogReceiver`.
    let rx = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9876".parse()?,
        Default::default(),
        re_grpc_server::shutdown::never(),
    );

    let mut native_options = re_viewer::native::eframe_options(None);
    native_options.viewport = native_options
        .viewport
        .with_app_id("rerun_extend_viewer_ui_example");

    let startup_options = re_viewer::StartupOptions::default();

    // This is used for analytics, if the `analytics` feature is on in `Cargo.toml`
    let app_env = re_viewer::AppEnvironment::Custom("My Wrapper".to_owned());

    let window_title = "My Customized Viewer";
    eframe::run_native(
        window_title,
        native_options,
        Box::new(move |cc| {
            re_viewer::customize_eframe_and_setup_renderer(cc)?;

            let mut rerun_app = re_viewer::App::new(
                main_thread_token,
                re_viewer::build_info(),
                app_env,
                startup_options,
                cc,
                None,
                re_viewer::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?,
            );
            rerun_app.add_log_receiver(rx);
            Ok(Box::new(MyApp { rerun_app }))
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

        if let Some(entity_db) = self.rerun_app.recording_db() {
            entity_db_ui(ui, entity_db);
        } else {
            ui.label("No log database loaded yet.");
        }
    }
}

/// Show the content of the log database.
fn entity_db_ui(ui: &mut egui::Ui, entity_db: &re_entity_db::EntityDb) {
    if let Some(store_info) = entity_db.store_info() {
        ui.label(format!("Application ID: {}", store_info.application_id()));
    }

    // There can be many timelines, but the `log_time` timeline is always there:
    let timeline = re_log_types::TimelineName::log_time();

    ui.separator();

    ui.strong("Entities:");

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for entity_path in entity_db.sorted_entity_paths() {
                ui.collapsing(entity_path.to_string(), |ui| {
                    entity_ui(ui, entity_db, timeline, entity_path);
                });
            }
        });
}

fn entity_ui(
    ui: &mut egui::Ui,
    entity_db: &re_entity_db::EntityDb,
    timeline: re_log_types::TimelineName,
    entity_path: &re_log_types::EntityPath,
) {
    // Each entity can have many components (e.g. position, color, radius, …):
    if let Some(components) = entity_db
        .storage_engine()
        .store()
        .all_components_on_timeline_sorted(&timeline, entity_path)
    {
        for component in &components {
            ui.collapsing(component.to_string(), |ui| {
                component_ui(ui, entity_db, timeline, entity_path, *component);
            });
        }
    }
}

fn component_ui(
    ui: &mut egui::Ui,
    entity_db: &re_entity_db::EntityDb,
    timeline: re_log_types::TimelineName,
    entity_path: &re_log_types::EntityPath,
    component: re_sdk_types::ComponentIdentifier,
) {
    // You can query the data for any time point, but for now
    // just show the last value logged for each component:
    let query = re_chunk_store::LatestAtQuery::latest(timeline);

    let results = entity_db
        .storage_engine()
        .cache()
        .latest_at(&query, entity_path, [component]);

    if let Some(data) = results.component_batch_raw(component) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Iterate over all the instances (e.g. all the points in the point cloud):

                let num_instances = data.len();
                for i in 0..num_instances {
                    ui.label(format_arrow(&*data.slice(i, 1)));
                }
            });
    };
}

fn format_arrow(array: &dyn arrow::array::Array) -> String {
    use arrow::util::display::{ArrayFormatter, FormatOptions};

    let num_bytes = array.get_buffer_memory_size();
    if array.len() == 1 && num_bytes < 256 {
        // Print small items:
        let options = FormatOptions::default();
        if let Ok(formatter) = ArrayFormatter::try_new(array, &options) {
            return formatter.value(0).to_string();
        }
    }

    // Fallback:
    format!("{num_bytes} bytes")
}
