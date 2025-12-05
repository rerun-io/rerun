use std::any::Any;
use std::path::PathBuf;

use ahash::HashMap;
use poll_promise::Promise;

const FILE_SAVER_PROMISE: &str = "file_saver";

/// Pending background tasks, e.g. files being saved.
#[derive(Default)]
pub struct BackgroundTasks {
    /// Pending background tasks, using `poll_promise`.
    promises: HashMap<String, Promise<Box<dyn Any + Send>>>,
}

impl BackgroundTasks {
    /// Creates a promise with the specified name that will run `f` on a background
    /// thread using the `poll_promise` crate.
    ///
    /// Names can only be re-used once the promise with that name has finished running,
    /// otherwise an other is returned.
    // TODO(cmc): offer `spawn_async_promise` once we open save_file to the web
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_threaded_promise<F, T>(
        &mut self,
        name: impl Into<String>,
        f: F,
    ) -> anyhow::Result<()>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let name = name.into();

        if self.promises.contains_key(&name) {
            anyhow::bail!("there's already a promise {name:?} running!");
        }

        let f = move || Box::new(f()) as Box<dyn Any + Send>; // erase it
        let promise = Promise::spawn_thread(&name, f);

        self.promises.insert(name, promise);

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_file_saver<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce() -> anyhow::Result<PathBuf> + Send + 'static,
    {
        self.spawn_threaded_promise(FILE_SAVER_PROMISE, f)
    }

    /// Polls the promise with the given name.
    ///
    /// Returns `Some<T>` it it's ready, or `None` otherwise.
    ///
    /// Panics if `T` does not match the actual return value of the promise.
    pub fn poll_promise<T: Any>(&mut self, name: impl AsRef<str>) -> Option<T> {
        let promise = self.promises.remove(name.as_ref())?;
        match promise.try_take() {
            Ok(any) => Some(
                *any.downcast::<T>()
                    .expect("Downcast failure in poll_promise"),
            ),
            Err(promise) => {
                self.promises.insert(name.as_ref().to_owned(), promise);
                None
            }
        }
    }

    pub fn poll_file_saver_promise(&mut self) -> Option<anyhow::Result<PathBuf>> {
        self.poll_promise(FILE_SAVER_PROMISE)
    }

    pub fn is_file_save_in_progress(&self) -> bool {
        self.promises.contains_key(FILE_SAVER_PROMISE)
    }
}
