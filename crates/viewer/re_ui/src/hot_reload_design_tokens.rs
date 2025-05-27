use crate::DesignTokens;

struct DesignTokensPerTheme {
    dark: DesignTokens,
    light: DesignTokens,
}

impl DesignTokensPerTheme {
    #[cfg(not(hot_reload_design_tokens))]
    fn load() -> anyhow::Result<Self> {
        Ok(Self {
            dark: DesignTokens::load(egui::Theme::Dark, include_str!("../data/dark_theme.ron"))?,
            light: DesignTokens::load(egui::Theme::Light, include_str!("../data/light_theme.ron"))?,
        })
    }

    #[cfg(hot_reload_design_tokens)]
    fn load() -> anyhow::Result<Self> {
        #![expect(clippy::unwrap_used)] // TODO: replace unwraps with expect or proper error handling

        let data_path = std::fs::canonicalize(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"),
        )
        .unwrap();

        Ok(Self {
            dark: DesignTokens::load(
                egui::Theme::Dark,
                &std::fs::read_to_string(data_path.join("dark_theme.ron")).unwrap(),
            )?,
            light: DesignTokens::load(
                egui::Theme::Light,
                &std::fs::read_to_string(data_path.join("light_theme.ron")).unwrap(),
            )?,
        })
    }
}

#[cfg(not(hot_reload_design_tokens))]
mod design_token_access {
    use super::*;
    use std::sync::OnceLock;

    pub fn design_tokens_per_theme() -> &'static DesignTokensPerTheme {
        static DESIGN_TOKENS: OnceLock<DesignTokensPerTheme> = OnceLock::new();
        DESIGN_TOKENS
            .get_or_init(|| DesignTokensPerTheme::load().expect("Failed to load design tokens"))
    }
}

#[cfg(hot_reload_design_tokens)]
mod design_token_access {
    #![expect(clippy::unwrap_used)] // TODO: replace unwraps with expect or proper error handling

    use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
    use parking_lot::{Mutex, RwLock};
    use std::sync::{OnceLock, mpsc};
    use std::thread;

    use super::DesignTokensPerTheme;

    static CURRENT_TOKENS: OnceLock<RwLock<&'static DesignTokensPerTheme>> = OnceLock::new();

    pub fn hot_reload_design_tokens() {
        let design_tokens = match DesignTokensPerTheme::load() {
            Ok(design_tokens) => design_tokens,
            Err(err) => {
                re_log::error!("Failed to reload design tokens: {err}");
                return;
            }
        };

        if let Some(current) = CURRENT_TOKENS.get() {
            *current.write() = Box::leak(Box::new(design_tokens));
        } else {
            re_log::warn!("Failed to update design tokens: CURRENT_TOKENS is not initialized.");
        }
    }

    type Callback = Box<dyn Fn() + Send>;

    static STUFF_TO_DO_ON_HOT_RELOAD: OnceLock<Mutex<Vec<Callback>>> = OnceLock::new();

    pub fn install_hot_reload<F>(f: F)
    where
        F: Fn() + Send + 'static,
    {
        let stuff = STUFF_TO_DO_ON_HOT_RELOAD.get_or_init(Default::default);
        stuff.lock().push(Box::new(f));
        setup_file_watcher();
    }

    fn run_all_hot_reloading() {
        for stuff in STUFF_TO_DO_ON_HOT_RELOAD.get().unwrap().lock().iter() {
            stuff();
        }
    }

    fn setup_file_watcher() {
        static WATCHER_INIT: OnceLock<()> = OnceLock::new();

        WATCHER_INIT.get_or_init(|| {
            // Spawn watcher thread
            thread::Builder::new()
                .name("re_ui design token hot reloader".to_owned())
                .spawn(|| {
                    let (tx, rx) = mpsc::channel();

                    let mut watcher: RecommendedWatcher = Watcher::new(
                        move |res: Result<Event, notify::Error>| {
                            if let Ok(event) = res {
                                if event.kind.is_modify() {
                                    tx.send(()).ok();
                                }
                            }
                        },
                        notify::Config::default(),
                    )
                    .unwrap();

                    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
                    let path = std::fs::canonicalize(path).unwrap();
                    re_log::debug!("Watching for file changes in {}", path.display());

                    watcher.watch(&path, RecursiveMode::Recursive).unwrap();

                    while rx.recv().is_ok() {
                        // Small delay to avoid rapid reloads
                        thread::sleep(std::time::Duration::from_millis(100));

                        run_all_hot_reloading();
                    }
                })
                .unwrap();

            re_log::debug!("Hot-reloading of design tokens enabled.");
        });
    }

    pub fn design_tokens_per_theme() -> &'static DesignTokensPerTheme {
        let current = CURRENT_TOKENS.get_or_init(|| {
            let design_tokens =
                DesignTokensPerTheme::load().expect("Failed to load initial design tokens");
            RwLock::new(Box::leak(Box::new(design_tokens)))
        });

        *current.read()
    }
}

pub fn design_tokens_of(theme: egui::Theme) -> &'static DesignTokens {
    match theme {
        egui::Theme::Dark => &design_token_access::design_tokens_per_theme().dark,
        egui::Theme::Light => &design_token_access::design_tokens_per_theme().light,
    }
}

#[cfg(hot_reload_design_tokens)]
pub use design_token_access::{hot_reload_design_tokens, install_hot_reload};
