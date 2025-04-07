//! This example shows how to wrap the Rerun Viewer in your own GUI.

use std::sync::Arc;

use re_viewer::external::{eframe, egui, egui::mutex::Mutex, re_log, re_memory};
use re_viewer::AsyncRuntimeHandle;

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
    // There are other ways of "feeding" the viewer though - all you need is a `re_smart_channel::Receiver`.
    let (rx, _) = re_grpc_server::spawn_with_recv(
        "0.0.0.0:9876".parse()?,
        "75%".parse()?,
        re_grpc_server::shutdown::never(),
    );

    let mut native_options = re_viewer::native::eframe_options(None);
    native_options.viewport = native_options
        .viewport
        .with_app_id("rerun_extend_viewer_ui_example");

    let shared_state: Arc<Mutex<SharedState>> = Default::default();

    let startup_options = re_viewer::StartupOptions {
        callbacks: Some(
            re_viewer::Callbacks::builder()
                .on_selection_change({
                    let shared_state = shared_state.clone();
                    move |items| {
                        shared_state.lock().current_selection = items;
                    }
                })
                .on_timeline_change({
                    let shared_state = shared_state.clone();
                    move |timeline, time| {
                        let mut shared_state = shared_state.lock();
                        shared_state.current_timeline = timeline.name().as_str().to_owned();
                        shared_state.current_time = time.as_f64();
                    }
                })
                .on_time_update({
                    let shared_state = shared_state.clone();
                    move |time| {
                        let mut shared_state = shared_state.lock();
                        shared_state.current_time = time.as_f64();
                    }
                })
                .build(),
        ),
        ..Default::default()
    };

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
                &app_env,
                startup_options,
                cc,
                AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()?,
            );
            rerun_app.add_log_receiver(rx);
            Ok(Box::new(MyApp {
                rerun_app,
                shared_state,
            }))
        }),
    )?;

    Ok(())
}

#[derive(Default)]
struct SharedState {
    current_selection: Vec<re_viewer::CallbackSelectionItem>,
    current_time: f64,
    current_timeline: String,
}

struct MyApp {
    rerun_app: re_viewer::App,
    shared_state: Arc<Mutex<SharedState>>,
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

        {
            let shared_state = self.shared_state.lock();

            ui.vertical(|ui| {
                for item in &shared_state.current_selection {
                    selection_item_ui(ui, item);
                }

                ui.separator();

                ui.label(format!(
                    "Current timeline: {}",
                    shared_state.current_timeline
                ));
                ui.label(format!("Current time: {}", shared_state.current_time));
            });
        }
    }
}

fn selection_item_ui(ui: &mut egui::Ui, item: &re_viewer::CallbackSelectionItem) {
    match item {
        re_viewer::CallbackSelectionItem::Entity {
            entity_path,
            instance_id,
            view_name,
            position,
        } => {
            ui.vertical(|ui| {
                if let Some(instance_id) = instance_id.specific_index().map(|id| id.get()) {
                    ui.label(format!("Entity {entity_path}[{instance_id}]"));
                } else {
                    ui.label(format!("Entity {entity_path}"));
                }
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(format!("View name: {view_name:?}"));
                });
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(format!("Position: {position:?}"));
                });
            });
        }
        re_viewer::CallbackSelectionItem::View { view_id, view_name } => {
            ui.label(format!("View {view_name}"));
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(format!("View ID: {}", view_id.uuid()));
            });
        }
        re_viewer::CallbackSelectionItem::Container {
            container_id,
            container_name,
        } => {
            ui.label(format!("Container {container_name}"));
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(format!("Container ID: {}", container_id.uuid()));
            });
        }
    }
}
