//! A hidden `rerun.desktop` entry for the current user, so that the GNOME dock and Alt-Tab show the Rerun icon.
//!
//! GNOME ignores the window icon (`_NET_WM_ICON`) and instead looks up the icon in a
//! [desktop entry](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
//! whose `StartupWMClass` (X11) or basename (Wayland `app_id`) matches the window's [`crate::native::APP_ID`].
//!
//! The entry is installed best-effort at startup, and never overwrites a `rerun.desktop` that we did not write ourselves.
//! Set [`crate::env_vars::RERUN_DESKTOP_ENTRY`] to `0` to disable it.

use std::ffi::OsStr;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

/// The icon the desktop entry points at.
const APP_ICON_PNG: &[u8] = include_bytes!("../data/app_icon.png");

/// Key that marks a desktop entry as written by us, and thus ours to update.
const GENERATED_MARKER_KEY: &str = "X-Rerun-Generated";

const DESKTOP_ENTRY_PATH: &str = "applications/rerun.desktop";

/// Install (or update) the hidden desktop entry and icon in the user's XDG data home, if appropriate.
///
/// Must run before the window is created: GNOME matches a window to its desktop entry when the window is mapped.
/// Never fails: problems are logged at debug level.
pub(crate) fn install_if_missing() {
    if !is_enabled(std::env::var_os(crate::env_vars::RERUN_DESKTOP_ENTRY).as_deref()) {
        return;
    }

    re_tracing::profile_function!();

    let Some(dirs) = XdgDirs::from_values(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        std::env::var_os("XDG_DATA_DIRS").as_deref(),
    ) else {
        re_log::debug!("Skipping desktop entry: neither $XDG_DATA_HOME nor $HOME is set");
        return;
    };

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            re_log::debug!("Skipping desktop entry: failed to find the current executable: {err}");
            return;
        }
    };

    let version = re_build_info::CrateVersion::LOCAL.to_string();
    let Some(desired) = render_desktop_entry(&exe, &version) else {
        re_log::debug!(
            "Skipping desktop entry: executable path is not valid UTF-8 or contains control characters: {}",
            exe.display()
        );
        return;
    };

    dirs.apply(dirs.plan(&desired));
}

/// Parses the value of [`crate::env_vars::RERUN_DESKTOP_ENTRY`].
///
/// Only `0`, `false`, `no` and `off` (case-insensitive, surrounding whitespace ignored) disable the entry.
fn is_enabled(value: Option<&OsStr>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let value = value.to_string_lossy().trim().to_ascii_lowercase();
    !matches!(value.as_str(), "0" | "false" | "no" | "off")
}

/// The XDG base directories we read and write, per the
/// [XDG Base Directory spec](https://specifications.freedesktop.org/basedir-spec/latest/).
#[derive(Debug, PartialEq, Eq)]
struct XdgDirs {
    /// `$XDG_DATA_HOME`, defaulting to `$HOME/.local/share`.
    data_home: PathBuf,

    /// `$XDG_DATA_DIRS`, defaulting to `/usr/local/share` and `/usr/share`.
    data_dirs: Vec<PathBuf>,
}

/// What to write to the user's data home.
#[derive(Debug, Default, PartialEq, Eq)]
struct Plan {
    /// New contents of the user's `rerun.desktop`, if it needs writing.
    desktop_entry: Option<String>,

    /// The icon, if it needs writing.
    icon: Option<&'static [u8]>,
}

impl XdgDirs {
    /// Resolves the directories from the values of `$HOME`, `$XDG_DATA_HOME` and `$XDG_DATA_DIRS`.
    ///
    /// Empty and relative values are ignored, as the spec requires.
    /// Returns `None` if there is no usable data home.
    fn from_values(
        home: Option<&OsStr>,
        xdg_data_home: Option<&OsStr>,
        xdg_data_dirs: Option<&OsStr>,
    ) -> Option<Self> {
        let absolute = |value: Option<&OsStr>| {
            let path = Path::new(value?);
            path.is_absolute().then(|| path.to_owned())
        };
        let data_home = absolute(xdg_data_home)
            .or_else(|| absolute(home).map(|home| home.join(".local/share")))?;

        let data_dirs: Vec<PathBuf> = xdg_data_dirs
            .map(|dirs| {
                std::env::split_paths(dirs)
                    .filter(|dir| dir.is_absolute())
                    .collect()
            })
            .unwrap_or_default();
        let data_dirs = if data_dirs.is_empty() {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        } else {
            data_dirs
        };

        Some(Self {
            data_home,
            data_dirs,
        })
    }

    fn user_entry_path(&self) -> PathBuf {
        self.data_home.join(DESKTOP_ENTRY_PATH)
    }

    fn hicolor_dir(&self) -> PathBuf {
        self.data_home.join("icons/hicolor")
    }

    fn icon_path(&self) -> PathBuf {
        self.hicolor_dir().join("512x512/apps/rerun.png")
    }

    /// Looks at what is on disk and decides what to write.
    ///
    /// We only write a desktop entry where there is none, or where the existing one is ours.
    /// A missing user entry is not written if a system one exists, so as not to shadow it.
    /// An icon that differs from ours is only replaced if the desktop entry is ours: otherwise it may be the user's own.
    fn plan(&self, desired: &str) -> Plan {
        let user_entry_path = self.user_entry_path();
        let ours = match read_regular_file(&user_entry_path).and_then(|bytes| {
            String::from_utf8(bytes)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
        }) {
            Ok(contents) if is_generated_by_us(&contents) => Some(contents),
            Ok(_) => return Plan::default(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                re_log::debug!(
                    "Treating unreadable desktop entry as not ours: {err}\nFile path: {}",
                    user_entry_path.display()
                );
                return Plan::default();
            }
        };

        let icon = read_regular_file(&self.icon_path());
        let icon_is_missing =
            matches!(&icon, Err(err) if err.kind() == std::io::ErrorKind::NotFound);
        let icon_is_ours = matches!(&icon, Ok(bytes) if bytes == APP_ICON_PNG);

        match ours {
            None => {
                if self
                    .data_dirs
                    .iter()
                    .any(|dir| dir.join(DESKTOP_ENTRY_PATH).exists())
                {
                    return Plan::default();
                }
                Plan {
                    desktop_entry: Some(desired.to_owned()),
                    icon: icon_is_missing.then_some(APP_ICON_PNG),
                }
            }
            Some(contents) => Plan {
                desktop_entry: (contents != desired).then(|| desired.to_owned()),
                icon: (!icon_is_ours).then_some(APP_ICON_PNG),
            },
        }
    }

    /// Writes what the plan says, logging the outcome at debug level.
    fn apply(&self, plan: Plan) {
        let Plan {
            desktop_entry,
            icon,
        } = plan;

        if let Some(contents) = desktop_entry {
            let path = self.user_entry_path();
            log_write(&path, write_atomically(&path, contents.as_bytes()));
        }

        if let Some(icon) = icon {
            let path = self.icon_path();
            let result = write_atomically(&path, icon).and_then(|()| {
                // GTK only rescans an icon theme (and drops its stale `icon-theme.cache`)
                // when the mtime of the theme directory itself changes.
                std::fs::File::open(self.hicolor_dir())?.set_modified(std::time::SystemTime::now())
            });
            log_write(&path, result);
        }
    }
}

fn log_write(path: &Path, result: std::io::Result<()>) {
    match result {
        Ok(()) => re_log::debug!("Installed desktop entry file: {}", path.display()),
        Err(err) => re_log::debug!(
            "Failed to install desktop entry file: {err}\nFile path: {}",
            path.display()
        ),
    }
}

/// Reads `path`, following symlinks, but only if it is a regular file.
///
/// Reading a FIFO would block startup until something writes to it.
fn read_regular_file(path: &Path) -> std::io::Result<Vec<u8>> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    std::fs::read(path)
}

/// Whether the desktop entry contains our marker key in its `[Desktop Entry]` group.
fn is_generated_by_us(contents: &str) -> bool {
    let mut in_main_group = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main_group = line == "[Desktop Entry]";
        } else if in_main_group
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == GENERATED_MARKER_KEY
            && value.trim() == "true"
        {
            return true;
        }
    }
    false
}

/// The contents of our `rerun.desktop`, launching `exe`.
///
/// Returns `None` if `exe` is not valid UTF-8 or contains control characters,
/// which a desktop entry cannot represent.
fn render_desktop_entry(exe: &Path, version: &str) -> Option<String> {
    let exe = exe.to_str()?;
    if exe.chars().any(char::is_control) {
        return None;
    }
    let exec = escape_exec_arg(exe);

    Some(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.5\n\
         Name=Rerun\n\
         Comment=Rerun Viewer\n\
         Exec={exec}\n\
         Icon=rerun\n\
         StartupWMClass=rerun\n\
         Terminal=false\n\
         NoDisplay=true\n\
         {GENERATED_MARKER_KEY}=true\n\
         X-Rerun-Version={version}\n"
    ))
}

/// Escapes a single argument for the `Exec` key, per the
/// [Desktop Entry spec](https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html).
fn escape_exec_arg(arg: &str) -> String {
    const RESERVED: &[char] = &[
        ' ', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`',
    ];

    let quoted = if arg.contains(RESERVED) {
        let mut quoted = String::from('"');
        for c in arg.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                quoted.push('\\');
            }
            quoted.push(c);
        }
        quoted.push('"');
        quoted
    } else {
        arg.to_owned()
    };

    // Field codes come first, then the general string escaping that applies to every value.
    quoted.replace('%', "%%").replace('\\', "\\\\")
}

/// Writes world-readable (0644), via a temporary file and a rename so readers never see a half-written file.
fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let Some(dir) = path.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path has no parent directory",
        ));
    };
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(contents)?;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o644))?;
    file.persist(path).map_err(|err| err.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use super::{
        APP_ICON_PNG, Plan, XdgDirs, is_enabled, is_generated_by_us, render_desktop_entry,
    };

    const VERSION: &str = "0.99.0";

    fn desired(exe: &str) -> String {
        render_desktop_entry(Path::new(exe), VERSION).unwrap_or_default()
    }

    fn exec_line(exe: &str) -> Option<String> {
        let entry = render_desktop_entry(Path::new(exe), VERSION)?;
        entry
            .lines()
            .find_map(|line| line.strip_prefix("Exec="))
            .map(str::to_owned)
    }

    /// Sandboxed XDG dirs: a data home and a single system data dir, both under `root`.
    fn sandbox_dirs(root: &Path) -> XdgDirs {
        XdgDirs {
            data_home: root.join("home"),
            data_dirs: vec![root.join("sys")],
        }
    }

    fn write(path: &Path, contents: &[u8]) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create dir");
        }
        std::fs::write(path, contents).expect("write file");
    }

    const FOREIGN_ENTRY: &str = "[Desktop Entry]\nType=Application\nName=My Rerun\nExec=rerun\n";

    #[test]
    fn render_golden() {
        assert_eq!(
            render_desktop_entry(Path::new("/opt/rerun/bin/rerun"), "0.99.0").as_deref(),
            Some(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Version=1.5\n\
                 Name=Rerun\n\
                 Comment=Rerun Viewer\n\
                 Exec=/opt/rerun/bin/rerun\n\
                 Icon=rerun\n\
                 StartupWMClass=rerun\n\
                 Terminal=false\n\
                 NoDisplay=true\n\
                 X-Rerun-Generated=true\n\
                 X-Rerun-Version=0.99.0\n"
            )
        );
    }

    #[test]
    fn exec_quoting() {
        assert_eq!(
            exec_line("/opt/my rerun/rerun").as_deref(),
            Some(r#""/opt/my rerun/rerun""#)
        );
        assert_eq!(
            exec_line("/opt/$x/rerun").as_deref(),
            Some(r#""/opt/\\$x/rerun""#)
        );
        assert_eq!(
            exec_line(r#"/opt/a"b/rerun"#).as_deref(),
            Some(r#""/opt/a\\"b/rerun""#)
        );
        assert_eq!(
            exec_line(r"/opt/a\b/rerun").as_deref(),
            Some(r#""/opt/a\\\\b/rerun""#)
        );
        assert_eq!(
            exec_line("/opt/100%/rerun").as_deref(),
            Some("/opt/100%%/rerun")
        );
        assert_eq!(
            exec_line("/opt/100% sure/rerun").as_deref(),
            Some(r#""/opt/100%% sure/rerun""#)
        );
        assert_eq!(exec_line("/opt/a\nb/rerun"), None);
        assert_eq!(exec_line("/opt/a\tb/rerun"), None);
    }

    #[test]
    fn xdg_dirs_from_values() {
        let os = |s: &'static str| Some(OsStr::new(s));
        let defaults = vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ];

        // XDG_DATA_HOME set:
        assert_eq!(
            XdgDirs::from_values(os("/home/u"), os("/xdg"), None),
            Some(XdgDirs {
                data_home: PathBuf::from("/xdg"),
                data_dirs: defaults.clone(),
            })
        );

        // XDG_DATA_HOME empty or relative: fall back to $HOME.
        for xdg_data_home in [os(""), os("relative/xdg"), None] {
            assert_eq!(
                XdgDirs::from_values(os("/home/u"), xdg_data_home, os("")),
                Some(XdgDirs {
                    data_home: PathBuf::from("/home/u/.local/share"),
                    data_dirs: defaults.clone(),
                })
            );
        }

        // XDG_DATA_DIRS with empty and relative entries:
        assert_eq!(
            XdgDirs::from_values(os("/home/u"), None, os(":/a::rel/b:/c:")),
            Some(XdgDirs {
                data_home: PathBuf::from("/home/u/.local/share"),
                data_dirs: vec![PathBuf::from("/a"), PathBuf::from("/c")],
            })
        );

        // No usable data home:
        assert_eq!(XdgDirs::from_values(None, None, None), None);
        assert_eq!(XdgDirs::from_values(os(""), os(""), None), None);
        assert_eq!(XdgDirs::from_values(None, os("relative"), None), None);
    }

    #[test]
    fn row_missing_without_system_entry_writes_both() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let desired = desired("/opt/rerun");

        let plan = dirs.plan(&desired);
        assert_eq!(
            plan,
            Plan {
                desktop_entry: Some(desired.clone()),
                icon: Some(APP_ICON_PNG),
            }
        );

        dirs.apply(plan);
        assert_eq!(
            std::fs::read_to_string(dirs.user_entry_path()).ok(),
            Some(desired)
        );
        assert_eq!(
            std::fs::read(dirs.icon_path()).ok().as_deref(),
            Some(APP_ICON_PNG)
        );
    }

    #[test]
    fn row_missing_without_system_entry_keeps_different_icon() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let desired = desired("/opt/rerun");
        write(&dirs.icon_path(), b"the user's icon");

        let plan = dirs.plan(&desired);
        assert_eq!(
            plan,
            Plan {
                desktop_entry: Some(desired.clone()),
                icon: None,
            }
        );

        dirs.apply(plan);
        assert_eq!(
            std::fs::read_to_string(dirs.user_entry_path()).ok(),
            Some(desired)
        );
        assert_eq!(
            std::fs::read(dirs.icon_path()).ok().as_deref(),
            Some(b"the user's icon".as_slice())
        );
    }

    #[test]
    fn row_missing_with_system_entry_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        write(
            &tmp.path().join("sys/applications/rerun.desktop"),
            FOREIGN_ENTRY.as_bytes(),
        );

        let plan = dirs.plan(&desired("/opt/rerun"));
        assert_eq!(plan, Plan::default());

        dirs.apply(plan);
        assert!(!dirs.data_home.exists());
    }

    #[test]
    fn row_foreign_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        write(&dirs.user_entry_path(), FOREIGN_ENTRY.as_bytes());

        let plan = dirs.plan(&desired("/opt/rerun"));
        assert_eq!(plan, Plan::default());

        dirs.apply(plan);
        assert_eq!(
            std::fs::read_to_string(dirs.user_entry_path())
                .ok()
                .as_deref(),
            Some(FOREIGN_ENTRY)
        );
        assert!(!dirs.icon_path().exists());
    }

    #[test]
    fn generated_marker() {
        assert!(is_generated_by_us(&desired("/opt/rerun")));
        assert!(is_generated_by_us(
            "[Desktop Entry]\nName=Rerun\nX-Rerun-Generated = true\n"
        ));
        assert!(!is_generated_by_us(FOREIGN_ENTRY));
        assert!(!is_generated_by_us(
            "[Desktop Entry]\nName=Rerun\n\n[Desktop Action x]\nX-Rerun-Generated=true\n"
        ));
        assert!(!is_generated_by_us(
            "[Desktop Entry]\nX-Rerun-Generated=false\n"
        ));
    }

    #[test]
    fn row_ours_up_to_date_restores_icon() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let desired = desired("/opt/rerun");
        write(&dirs.user_entry_path(), desired.as_bytes());
        write(&dirs.icon_path(), b"stale icon");

        let plan = dirs.plan(&desired);
        assert_eq!(
            plan,
            Plan {
                desktop_entry: None,
                icon: Some(APP_ICON_PNG),
            }
        );

        dirs.apply(plan);
        assert_eq!(
            std::fs::read(dirs.icon_path()).ok().as_deref(),
            Some(APP_ICON_PNG)
        );
    }

    #[test]
    fn row_ours_outdated_is_rewritten() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let old = render_desktop_entry(Path::new("/opt/rerun"), "0.1.0").unwrap_or_default();
        let desired = desired("/opt/rerun");
        write(&dirs.user_entry_path(), old.as_bytes());

        let plan = dirs.plan(&desired);
        assert_eq!(
            plan,
            Plan {
                desktop_entry: Some(desired.clone()),
                icon: Some(APP_ICON_PNG),
            }
        );

        dirs.apply(plan);
        assert_eq!(
            std::fs::read_to_string(dirs.user_entry_path()).ok(),
            Some(desired)
        );
        assert_eq!(
            std::fs::read(dirs.icon_path()).ok().as_deref(),
            Some(APP_ICON_PNG)
        );
    }

    #[test]
    fn second_run_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let desired = desired("/opt/rerun");
        dirs.apply(dirs.plan(&desired));
        let mtimes = || {
            [dirs.user_entry_path(), dirs.icon_path()]
                .map(|path| std::fs::metadata(path).and_then(|m| m.modified()).ok())
        };
        let before = mtimes();

        let plan = dirs.plan(&desired);
        assert_eq!(plan, Plan::default());

        dirs.apply(plan);
        assert_eq!(mtimes(), before);
    }

    #[test]
    fn opt_out_parsing() {
        for value in ["0", "FALSE", " off ", "no", "Off"] {
            assert!(!is_enabled(Some(OsStr::new(value))), "{value:?}");
        }
        assert!(is_enabled(None));
        for value in ["1", "", "true", "yes"] {
            assert!(is_enabled(Some(OsStr::new(value))), "{value:?}");
        }
    }

    #[test]
    fn icon_write_touches_existing_hicolor_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        let hicolor = dirs.data_home.join("icons/hicolor");
        // The icon's own directory exists already, so writing the icon doesn't create anything directly in `hicolor`.
        std::fs::create_dir_all(hicolor.join("512x512/apps")).expect("create dir");
        let long_ago =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::open(&hicolor)
            .and_then(|dir| dir.set_modified(long_ago))
            .expect("set mtime");
        let hicolor_mtime = || std::fs::metadata(&hicolor).and_then(|m| m.modified()).ok();
        let desired = desired("/opt/rerun");

        dirs.apply(dirs.plan(&desired));
        let after_install = hicolor_mtime();
        assert!(after_install > Some(long_ago), "{after_install:?}");

        dirs.apply(dirs.plan(&desired));
        assert_eq!(hicolor_mtime(), after_install);
    }

    #[test]
    fn written_files_are_world_readable() {
        use std::os::unix::fs::PermissionsExt as _;

        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        dirs.apply(dirs.plan(&desired("/opt/rerun")));

        for path in [dirs.user_entry_path(), dirs.icon_path()] {
            let mode = std::fs::metadata(&path)
                .map(|m| m.permissions().mode() & 0o777)
                .ok();
            assert_eq!(mode, Some(0o644), "{}", path.display());
        }
    }

    #[test]
    fn unwritable_data_home_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write(&tmp.path().join("file"), b"");
        let dirs = XdgDirs {
            data_home: tmp.path().join("file/home"),
            data_dirs: vec![tmp.path().join("sys")],
        };

        assert_eq!(dirs.plan(&desired("/opt/rerun")), Plan::default());

        dirs.apply(Plan {
            desktop_entry: Some(desired("/opt/rerun")),
            icon: Some(APP_ICON_PNG),
        });
        assert!(!dirs.data_home.exists());
    }

    #[test]
    fn unreadable_user_entry_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        std::fs::create_dir_all(dirs.user_entry_path()).expect("create dir");

        let plan = dirs.plan(&desired("/opt/rerun"));
        assert_eq!(plan, Plan::default());

        dirs.apply(plan);
        assert!(dirs.user_entry_path().is_dir());
        assert!(!dirs.icon_path().exists());
    }

    #[test]
    fn failed_icon_write_still_writes_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        std::fs::create_dir_all(dirs.icon_path()).expect("create dir");

        dirs.apply(Plan {
            desktop_entry: Some(desired("/opt/rerun")),
            icon: Some(APP_ICON_PNG),
        });
        assert_eq!(
            std::fs::read_to_string(dirs.user_entry_path()).ok(),
            Some(desired("/opt/rerun"))
        );
        assert!(dirs.icon_path().is_dir());
    }

    /// Runs [`XdgDirs::plan`] on another thread, so that a test fails instead of hanging if it blocks.
    fn plan_with_timeout(dirs: XdgDirs, desired: String) -> Option<Plan> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("desktop_entry_plan".to_owned())
            .spawn(move || tx.send(dirs.plan(&desired)))
            .expect("spawn thread");
        rx.recv_timeout(std::time::Duration::from_secs(5)).ok()
    }

    fn mkfifo(path: &Path) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        let status = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .expect("run mkfifo");
        assert!(status.success(), "mkfifo failed");
    }

    #[test]
    fn fifo_user_entry_does_not_block() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        mkfifo(&dirs.user_entry_path());

        let plan = plan_with_timeout(sandbox_dirs(tmp.path()), desired("/opt/rerun"));
        assert_eq!(plan, Some(Plan::default()));
    }

    #[test]
    fn fifo_icon_does_not_block() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dirs = sandbox_dirs(tmp.path());
        mkfifo(&dirs.icon_path());

        let plan = plan_with_timeout(sandbox_dirs(tmp.path()), desired("/opt/rerun"));
        assert_eq!(
            plan,
            Some(Plan {
                desktop_entry: Some(desired("/opt/rerun")),
                icon: None,
            })
        );
    }
}
