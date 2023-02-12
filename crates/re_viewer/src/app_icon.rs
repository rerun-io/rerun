/// In which state the app icon is (as far as we know).
#[derive(PartialEq, Eq)]
#[allow(dead_code)]
pub enum AppIconStatus {
    /// We did not set it or failed to do it. In any case we won't try again.
    NotSetIgnored,

    /// We haven't set the icon yet, we should try again next frame.
    ///
    /// This can happen repeatedly due to lazy window creation on some platforms.
    NotSetTryAgain,

    /// We successfully set the icon and it should be visible now.
    Set,
}

/// Sets app icon at runtime.
///
/// By setting the icon at runtime and not via resource files etc. we ensure that we'll get the chance
/// to set the same icon when the process/window is started from python (which sets its own icon ahead of us!).
///
/// Since window creation can be lazy, call this every frame until it's either successfully or gave up.
/// (See [`AppIconStatus`])
pub fn setup_app_icon() -> AppIconStatus {
    crate::profile_function!();

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
    // Also, there is scaling issues, detailed below.

    // TODO(andreas): This does not set the task bar icon for when our application is started from python.
    //      Things tried so far:
    //      * Querying for an owning window and setting icon there (there doesn't seem to be an owning window)
    //      * using undocumented SetConsoleIcon method (successfully queried via GetProcAddress)

    // SAFETY: WinApi function without side-effects.
    let window_handle = unsafe { winuser::GetActiveWindow() };
    if window_handle.is_null() {
        // The Window isn't available yet. Try again later!
        return AppIconStatus::NotSetTryAgain;
    }

    fn create_hicon_with_scale(
        unscaled_image: &image::DynamicImage,
        target_size: i32,
    ) -> winapi::shared::windef::HICON {
        let image_scaled = image::imageops::resize(
            unscaled_image,
            target_size as _,
            target_size as _,
            image::imageops::Lanczos3,
        );

        // Creating transparent icons with WinApi is a huge mess.
        // We'd need to go through CreateIconIndirect's ICONINFO struct which then
        // takes a mask HBITMAP and a color HBITMAP and creating each of these is pain.
        // Instead we workaround this by creating a png which CreateIconFromResourceEx magically understands.
        // This is a pretty horrible hack as we spend a lot of time encoding and decoding, but at least the code is a lot shorter.
        let mut image_scaled_bytes: Vec<u8> = Vec::new();
        if image_scaled
            .write_to(
                &mut std::io::Cursor::new(&mut image_scaled_bytes),
                image::ImageOutputFormat::Png,
            )
            .is_err()
        {
            return std::ptr::null_mut();
        }

        // SAFETY: Creating an HICON which should be readonly on our data.
        unsafe {
            winuser::CreateIconFromResourceEx(
                image_scaled_bytes.as_mut_ptr(),
                image_scaled_bytes.len() as u32,
                1,           // Means this is an icon, not a cursor.
                0x00030000,  // Version number of the HICON
                target_size, // Note that this method can scale, but it does so *very* poorly. So let's avoid that!
                target_size,
                winuser::LR_DEFAULTCOLOR,
            )
        }
    }

    let Ok(unscaled_image) = image::load_from_memory(re_ui::icons::APP_ICON.png_bytes) else {
        re_log::debug!("Failed to decode icon png data.");
        return AppIconStatus::NotSetIgnored;
    };

    // Only setting ICON_BIG with the icon size for big icons (SM_CXICON) works fine
    // but the scaling it does then for the small icon is pretty bad.
    // Instead we set the correct sizes manually and take over the scaling ourselves.
    // For this to work we first need to set the big icon and then the small one.
    //
    // Note that ICON_SMALL may be used even if we don't render a title bar as it may be used in alt+tab!
    {
        // SAFETY: WinAPI getter function with no known side effects.
        let icon_size_big = unsafe { winuser::GetSystemMetrics(winuser::SM_CXICON) };
        let icon_big = create_hicon_with_scale(&unscaled_image, icon_size_big);
        if icon_big.is_null() {
            re_log::debug!("Failed to create HICON (for big icon) from embedded png data.");
            return AppIconStatus::NotSetIgnored; // We could try independently with the small icon but what's the point, it would look bad!
        } else {
            // SAFETY: Unsafe WinApi function, takes objects previously created with WinAPI, all checked for null prior.
            unsafe {
                winuser::SendMessageW(
                    window_handle,
                    winuser::WM_SETICON,
                    winuser::ICON_BIG as usize,
                    icon_big as isize,
                );
            }
        }
    }
    {
        // SAFETY: WinAPI getter function with no known side effects.
        let icon_size_small = unsafe { winuser::GetSystemMetrics(winuser::SM_CXSMICON) };
        let icon_small = create_hicon_with_scale(&unscaled_image, icon_size_small);
        if icon_small.is_null() {
            re_log::debug!("Failed to create HICON (for small icon) from embedded png data.");
            return AppIconStatus::NotSetIgnored;
        } else {
            // SAFETY: Unsafe WinApi function, takes objects previously created with WinAPI, all checked for null prior.
            unsafe {
                winuser::SendMessageW(
                    window_handle,
                    winuser::WM_SETICON,
                    winuser::ICON_SMALL as usize,
                    icon_small as isize,
                );
            }
        }
    }

    // It _probably_ worked out.
    AppIconStatus::Set
}

/// Set icon & app title for `MacOS` applications.
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
        app_menu.setTitle_(NSString::alloc(nil).init_str(crate::APPLICATION_NAME));

        // The title in the Dock apparently can't be changed.
        // At least these people didn't figure it out either:
        // https://stackoverflow.com/questions/69831167/qt-change-application-title-dynamically-on-macos
        // https://stackoverflow.com/questions/28808226/changing-cocoa-app-icon-title-and-menu-labels-at-runtime
    }

    AppIconStatus::Set
}
