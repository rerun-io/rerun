/// In which state the app icon is (as far as we know).
#[derive(PartialEq, Eq)]
pub enum AppIconStatus {
    /// We did not set it or failed to do it. In any case we won't try again.
    NotSetIgnored,

    /// We haven't set the icon yet, we should try again next frame.
    /// This can happen due to lazy window creation.
    NotSetTryAgain,

    /// We successfully set the icon and it should be visible now.
    Success,
}

/// Sets app icon at runtime.
///
/// By setting the icon at runtime and not via resource files etc. we ensure that we'll get the chance
/// to set the same icon when the process/window is started from python (which sets its own icon ahead of us!).
///
/// Since window creation can be lazy, call this every frame until it's either succesfull or gave up.
/// (See [`AppIconStatus`])
pub fn setup_app_icon() -> AppIconStatus {
    #[cfg(target_os = "windows")]
    return setup_app_icon_windows();

    #[cfg(target_os = "macos")]
    return setup_app_icon_mac();

    #[allow(unreachable_code)]
    AppIconStatus::NotSetIgnored
}

/// Set icon for Windows applications.
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn setup_app_icon_windows() -> AppIconStatus {
    use winapi::um::winuser;

    // We would get fairly far already with winit's `set_window_icon` (which is exposed to eframe) actually!
    // However, it only sets ICON_SMALL, i.e. doesn't allow us to set a higher resolution icon for the task bar.

    // TODO(andreas): This does not set the task bar icon for when our application is started from python.
    //      Things tried so far:
    //      * Querying for an owning window and setting icon there (there doesn't seem to be an owning window)
    //      * using undocumented SetConsoleIcon method (successfully queried via GetProcAddress)

    let icon_data = &re_ui::icons::APP_ICON.png_bytes;

    // SAFETY: Accessing raw data from icon in a read-only manner. Icon data is static.
    unsafe {
        let hwnd = winuser::GetActiveWindow();
        if hwnd.is_null() {
            // The Window isn't available yet. Try again later!
            return AppIconStatus::NotSetTryAgain;
        }

        // Different sources say on the web say different things on what to do with ICON_SMALL when ICON_BIG is set.
        // Tried around a bit myself: Only setting ICON_BIG with the icon size for big icons (SM_CXICON) seems to be best,
        // both for the big taskbar icon and the small window/alt-tab icon.
        // The scaling algorithm doesn't seem to be great, so we shouldn't feed in a too large icon.

        let icon_size_big = winuser::GetSystemMetrics(winuser::SM_CXICON);
        #[allow(clippy::as_ptr_cast_mut)] // as_mut_ptr is a compile error here
        let hicon_big = winuser::CreateIconFromResourceEx(
            icon_data.as_ptr() as winapi::shared::minwindef::PBYTE,
            icon_data.len() as u32,
            1,             // Means this is an icon, not a cursor.
            0x00030000,    // Version number of the HICON
            icon_size_big, // This is the *desired* size. This method will scale for us
            icon_size_big,
            winuser::LR_DEFAULTCOLOR,
        );
        if hicon_big.is_null() {
            re_log::debug!("Failed to create HICON (for big icon) from embedded png data.");
        } else {
            winuser::SendMessageW(
                hwnd,
                winuser::WM_SETICON,
                winuser::ICON_BIG as usize,
                hicon_big as isize,
            );
        }
    }

    AppIconStatus::Success
}

/// Set icon & app title for MacOS applications.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn setup_app_icon_mac() -> AppIconStatus {
    use cocoa::{
        appkit::{NSApp, NSApplication, NSImage, NSMenu, NSWindow},
        base::{id, nil},
        foundation::{NSData, NSString},
    };
    use objc::{msg_send, sel, sel_impl};

    let icon_data = &re_ui::icons::APP_ICON.png_bytes;

    // SAFETY: Accessing raw data from icon in a read-only manner. Icon data is static!
    unsafe {
        let app = NSApp();
        let data = NSData::dataWithBytes_length_(
            nil,
            icon_data.as_ptr().cast::<std::ffi::c_void>(),
            icon_data.len() as u64,
        );
        let app_icon = NSImage::initWithData_(NSImage::alloc(nil), data);
        app.setApplicationIconImage_(app_icon);

        // Change the title in the top bar - for python processes this would be again "python" otherwise.
        let main_menu = app.mainMenu();
        let app_menu: id = msg_send![main_menu.itemAtIndex_(0), submenu];
        app_menu.setTitle_(NSString::alloc(nil).init_str(APPLICATION_NAME));

        // The title in the Dock apparently can't be changed.
        // At least these people didn't figure it out either:
        // https://stackoverflow.com/questions/69831167/qt-change-application-title-dynamically-on-macos
        // https://stackoverflow.com/questions/28808226/changing-cocoa-app-icon-title-and-menu-labels-at-runtime
    }

    AppIconStatus::Success
}
