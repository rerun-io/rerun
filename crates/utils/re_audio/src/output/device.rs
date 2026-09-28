//! [`OutputDevice`]: the platform device, opened without blocking the caller.

use std::sync::Arc;

use re_mutex::Mutex;

use super::{FailureCallback, FillCallback, OutputDeviceParameters, OutputError, backend};

/// Where an [`OutputDevice`] is in its life.
#[derive(Clone, Debug)]
pub enum DeviceStatus {
    /// Still being opened; `fill` is not called yet.
    Opening,

    /// Playing, calling `fill` for samples.
    Running,

    /// Could not be opened, or stopped after an unrecoverable error. Terminal.
    Failed(OutputError),
}

/// The default output stream. Dropping it stops playback.
///
/// Opening a device can take a while on some systems, so on native it happens on a
/// background thread and [`Self::open`] returns at once. Callers can queue audio right
/// away; it starts playing once the device is up. See [`Self::status`] for the outcome.
pub struct OutputDevice {
    status: Arc<Mutex<DeviceStatus>>,
    _opener: Opener,
}

impl OutputDevice {
    /// Starts opening the default output device. `fill` is called for samples once it runs.
    pub fn open(params: OutputDeviceParameters, fill: FillCallback) -> Self {
        let status = Arc::new(Mutex::new(DeviceStatus::Opening));
        let opener = Opener::start(params, fill, &status);
        Self {
            status,
            _opener: opener,
        }
    }

    pub fn status(&self) -> DeviceStatus {
        self.status.lock().clone()
    }
}

/// Opens the backend device and reports how it went to the shared status.
fn open_backend(
    params: OutputDeviceParameters,
    fill: FillCallback,
    status: &Arc<Mutex<DeviceStatus>>,
) -> Option<backend::Device> {
    re_tracing::profile_function!();
    let failure_status = status.clone();
    let on_failure: FailureCallback = Box::new(move |err| {
        re_log::warn!("{err}");
        *failure_status.lock() = DeviceStatus::Failed(err);
    });
    match backend::Device::new(params, fill, on_failure) {
        Ok(device) => {
            re_log::debug!("Opened audio output device");
            let mut status = status.lock();
            if matches!(*status, DeviceStatus::Opening) {
                *status = DeviceStatus::Running;
            }
            Some(device)
        }
        Err(err) => {
            re_log::warn!("Failed to open audio output device: {err}");
            *status.lock() = DeviceStatus::Failed(err);
            None
        }
    }
}

cfg_select! {
    target_arch = "wasm32" => {
        /// Owns the backend device. There are no threads on the web, so it opens synchronously.
        struct Opener {
            _device: Option<backend::Device>,
        }

        impl Opener {
            fn start(
                params: OutputDeviceParameters,
                fill: FillCallback,
                status: &Arc<Mutex<DeviceStatus>>,
            ) -> Self {
                Self {
                    _device: open_backend(params, fill, status),
                }
            }

        }
    }
    _ => {
        use std::sync::mpsc;

        /// A thread that opens the backend device and then owns it until dropped,
        /// so the device never has to cross threads.
        ///
        /// Opening a device can be slow (hundreds of ms) which is why we do it in a background thread.
        struct Opener {
            stop: Option<mpsc::SyncSender<()>>,
            thread: Option<std::thread::JoinHandle<()>>,
        }

        impl Opener {
            fn start(
                params: OutputDeviceParameters,
                fill: FillCallback,
                status: &Arc<Mutex<DeviceStatus>>,
            ) -> Self {
                let (stop, wait_for_drop) = mpsc::sync_channel(0);
                let thread = std::thread::Builder::new()
                    .name("audio_output_device".to_owned())
                    .spawn({
                        let status = status.clone();
                        move || {
                            let Some(_device) = open_backend(params, fill, &status) else {
                                return;
                            };
                            wait_for_drop.recv().ok();
                        }
                    });
                let thread = match thread {
                    Ok(thread) => Some(thread),
                    Err(err) => {
                        *status.lock() = DeviceStatus::Failed(OutputError::Device(format!(
                            "Failed to spawn the audio device thread: {err}"
                        )));
                        None
                    }
                };
                Self {
                    stop: Some(stop),
                    thread,
                }
            }
        }

        impl Drop for Opener {
            fn drop(&mut self) {
                re_tracing::profile_function!();
                self.stop.take();
                if let Some(thread) = self.thread.take() {
                    thread.join().ok();
                }
            }
        }
    }
}
