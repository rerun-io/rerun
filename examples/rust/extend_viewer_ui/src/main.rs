use re_viewer::external::{re_data_store, re_log_types};

#[global_allocator]
static GLOBAL: re_memory::AccountingAllocator<mimalloc::MiMalloc> =
    re_memory::AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    // Listen for SDK connection
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
        persist_state: true,
    };

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
        self.rerun_app.save(storage);
    }

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
            self.log_db_ui(ui, log_db);
        } else {
            ui.label("No log database loaded yet.");
        }
    }

    #[allow(clippy::unused_self)]
    fn log_db_ui(&self, ui: &mut egui::Ui, log_db: &re_data_store::LogDb) {
        // Shows how you can inspect the loaded data:

        if let Some(recording_info) = log_db.recording_info() {
            ui.label(format!("Application ID: {}", recording_info.application_id));
        }

        let timeline = re_log_types::Timeline::log_time();

        ui.separator();
        ui.strong("Entities:");
        for entity_path in log_db.entity_db.entity_paths() {
            ui.collapsing(entity_path.to_string(), |ui| {
                if let Some(mut components) = log_db
                    .entity_db
                    .data_store
                    .all_components(&timeline, entity_path)
                {
                    components.sort();
                    for component in components {
                        ui.label(component.to_string());
                    }
                }
            });
        }
    }
}
