use static_assertions::assert_not_impl_any;

/// A token that (almost) proves we are on the main thread.
///
/// Certain operations are only allowed on the main thread.
/// These operations should require this token.
/// For instance, any function using file dialogs (e.g. using [`rfd`](https://docs.rs/rfd/latest/rfd/)) should require this token.
///
/// The token should only be constructed in `fn main`, using [`MainThreadToken::i_promise_i_am_on_the_main_thread`],
/// and then be passed down the call tree to where it is needed.
/// [`MainThreadToken`] is neither `Send` nor `Sync`,
/// thus guaranteeing that it cannot be found in other threads.
///
/// Of course, there is nothing stopping you from calling [`MainThreadToken::i_promise_i_am_on_the_main_thread`] from a background thread,
/// but PLEASE DON'T DO THAT.
/// In other words, don't use this as a guarantee for unsafe code.
///
/// There is also [`MainThreadToken::from_egui_ui`] which uses the implicit guarantee of egui
/// (which _usually_ is run on the main thread) to construct a [`MainThreadToken`].
/// Use this only in a code base where you are sure that egui is running only on the main thread.
#[derive(Clone, Copy)]
pub struct MainThreadToken {
    /// Prevent from being sent between threads.
    ///
    /// Workaround until `impl !Send for X {}` is stable.
    _dont_send_me: std::marker::PhantomData<*const ()>,
}

impl MainThreadToken {
    /// Only call this from `fn main`, or you may get weird runtime errors!
    pub fn i_promise_i_am_on_the_main_thread() -> Self {
        // On web there is no thread name.
        // On native the thread-name is always "main" in Rust,
        // but there is nothing preventing a user from also naming another thread "main".
        // In any case, since `MainThreadToken` is just best-effort, we only check this in debug builds.
        #[cfg(not(target_arch = "wasm32"))]
        debug_assert_eq!(std::thread::current().name(), Some("main"),
            "DEBUG ASSERT: Trying to construct a MainThreadToken on a thread that is not the main thread!"
        );

        Self {
            _dont_send_me: std::marker::PhantomData,
        }
    }

    /// We _should_ only create an [`egui::Ui`] on the main thread,
    /// so having it is good enough to "prove" that we are on the main thread.
    ///
    /// Use this only in a code base where you are sure that egui is running only on the main thread.
    ///
    /// In theory there is nothing preventing anyone from creating a [`egui::Ui`] on another thread,
    /// but practice that is unlikely (or intentionally malicious).
    #[cfg(feature = "egui")]
    pub fn from_egui_ui(_ui: &egui::Ui) -> Self {
        Self::i_promise_i_am_on_the_main_thread()
    }
}

assert_not_impl_any!(MainThreadToken: Send, Sync);
assert_not_impl_any!(&MainThreadToken: Send, Sync);
