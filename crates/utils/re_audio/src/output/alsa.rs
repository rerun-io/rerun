//! Minimal ALSA playback through `libasound.so.2`, loaded at runtime with `dlopen`.
//!
//! We do this ourselves instead of using `tinyaudio` (as on the other platforms) because
//! `tinyaudio` links `libasound` directly. A binary linked that way cannot even start on a
//! machine without ALSA, such as a minimal container or a headless server: the dynamic loader
//! fails before `main` runs. Loading the library here instead means the viewer starts
//! everywhere and only audio playback reports that no output device is available.
//!
//! Once <https://github.com/mrDIMAS/tinyaudio/pull/25> lands, `tinyaudio` loads ALSA at
//! runtime itself and this module can go away.
//!
//! Only the handful of `snd_pcm_*` calls needed for blocking interleaved playback are bound.

#![expect(unsafe_code, reason = "FFI into libasound")]

use std::ffi::{CStr, c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::Library;

use super::{FailureCallback, FillCallback, OutputDeviceParameters, OutputError};

type SndPcm = c_void;

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_FLOAT_LE: c_int = 14;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

/// Allow ALSA to resample if the hardware cannot do the requested rate.
const SOFT_RESAMPLE: c_int = 1;

/// Number of callbacks worth of audio ALSA may buffer ahead of the hardware.
const LATENCY_IN_CALLBACKS: u64 = 3;

type PcmOpen = unsafe extern "C" fn(*mut *mut SndPcm, *const c_char, c_int, c_int) -> c_int;
type PcmSetParams =
    unsafe extern "C" fn(*mut SndPcm, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
type PcmWritei = unsafe extern "C" fn(*mut SndPcm, *const c_void, c_ulong) -> c_long;
type PcmRecover = unsafe extern "C" fn(*mut SndPcm, c_int, c_int) -> c_int;
type PcmDrain = unsafe extern "C" fn(*mut SndPcm) -> c_int;
type PcmClose = unsafe extern "C" fn(*mut SndPcm) -> c_int;
type Strerror = unsafe extern "C" fn(c_int) -> *const c_char;

/// The loaded library and the function pointers copied out of it.
///
/// The function pointers stay valid for as long as `_library` is alive, which is the whole
/// lifetime of this struct.
struct Alsa {
    _library: Library,
    pcm_open: PcmOpen,
    pcm_set_params: PcmSetParams,
    pcm_writei: PcmWritei,
    pcm_recover: PcmRecover,
    pcm_drain: PcmDrain,
    pcm_close: PcmClose,
    strerror: Strerror,
}

impl Alsa {
    fn load() -> Result<Self, OutputError> {
        // SAFETY: loading libasound runs its initializers, which have no preconditions.
        let library = unsafe { Library::new("libasound.so.2") }
            .map_err(|err| OutputError::LibraryLoad(err.to_string()))?;

        // SAFETY: the signatures below match the ALSA C API.
        unsafe {
            Ok(Self {
                pcm_open: *symbol(&library, b"snd_pcm_open\0")?,
                pcm_set_params: *symbol(&library, b"snd_pcm_set_params\0")?,
                pcm_writei: *symbol(&library, b"snd_pcm_writei\0")?,
                pcm_recover: *symbol(&library, b"snd_pcm_recover\0")?,
                pcm_drain: *symbol(&library, b"snd_pcm_drain\0")?,
                pcm_close: *symbol(&library, b"snd_pcm_close\0")?,
                strerror: *symbol(&library, b"snd_strerror\0")?,
                _library: library,
            })
        }
    }

    fn error(&self, what: &str, code: c_int) -> OutputError {
        // SAFETY: `snd_strerror` returns a static string for any code.
        let message = unsafe { CStr::from_ptr((self.strerror)(code)) };
        OutputError::Device(format!(
            "{what}: {} (ALSA error {code})",
            message.to_string_lossy()
        ))
    }
}

unsafe fn symbol<'a, T>(
    library: &'a Library,
    name: &[u8],
) -> Result<libloading::Symbol<'a, T>, OutputError> {
    // SAFETY: the caller guarantees `T` matches the symbol's real signature.
    unsafe { library.get::<T>(name) }.map_err(|err| OutputError::LibraryLoad(err.to_string()))
}

/// An open PCM handle, closed on drop.
struct Pcm {
    alsa: Alsa,
    handle: *mut SndPcm,
}

// SAFETY: the handle is only ever used from the playback thread that owns the `Pcm`.
unsafe impl Send for Pcm {}

impl Pcm {
    fn open(params: &OutputDeviceParameters) -> Result<Self, OutputError> {
        re_tracing::profile_function!();

        re_log::debug!(
            "Opening ALSA playback device 'default': sample_rate={}, channels={}, frames_per_callback={}",
            params.sample_rate,
            params.num_channels,
            params.frames_per_callback,
        );
        let alsa = Alsa::load()?;

        let device_name = c"default";
        let mut handle: *mut SndPcm = std::ptr::null_mut();
        // SAFETY: valid out-pointer and nul-terminated name.
        let code = unsafe {
            (alsa.pcm_open)(
                &raw mut handle,
                device_name.as_ptr(),
                SND_PCM_STREAM_PLAYBACK,
                0,
            )
        };
        if code < 0 {
            let env_vars = [
                "ALSA_CARD",
                "ALSA_CONFIG_DIR",
                "ALSA_CONFIG_PATH",
                "ALSA_PCM_CARD",
                "ALSA_PCM_DEVICE",
                "ALSA_PLUGIN_DIR",
                "HOME",
                "PIPEWIRE_REMOTE",
                "PULSE_SERVER",
                "XDG_RUNTIME_DIR",
            ];
            let env_vars = env_vars
                .into_iter()
                .map(|name| format!("{name}={:?}", std::env::var_os(name)))
                .collect::<Vec<_>>()
                .join(", ");
            re_log::debug!("ALSA playback environment: {env_vars}");

            for path in ["/proc/asound/cards", "/proc/asound/pcm"] {
                re_log::debug!(
                    "ALSA kernel devices: {:?}; path={path}",
                    std::fs::read_to_string(path),
                );
            }
            return Err(alsa.error("Failed to open the default playback device", code));
        }
        let pcm = Self { alsa, handle };

        let callback_us =
            params.frames_per_callback as u64 * 1_000_000 / params.sample_rate.max(1) as u64;
        let latency_us = (LATENCY_IN_CALLBACKS * callback_us) as c_uint;
        re_log::debug!(
            "Setting ALSA playback parameters: format=FLOAT_LE, access=RW_INTERLEAVED, sample_rate={}, channels={}, soft_resample={SOFT_RESAMPLE}, latency_us={latency_us}",
            params.sample_rate,
            params.num_channels,
        );
        // SAFETY: `handle` was just opened successfully.
        let code = unsafe {
            (pcm.alsa.pcm_set_params)(
                pcm.handle,
                SND_PCM_FORMAT_FLOAT_LE,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                params.num_channels as c_uint,
                params.sample_rate,
                SOFT_RESAMPLE,
                latency_us,
            )
        };
        if code < 0 {
            return Err(pcm.alsa.error("Failed to set playback parameters", code));
        }

        Ok(pcm)
    }

    /// Writes all of `interleaved`, recovering from underruns.
    fn write_all(&self, interleaved: &[f32], num_channels: usize) -> Result<(), OutputError> {
        let mut frames_written = 0;
        let num_frames = interleaved.len() / num_channels;

        while frames_written < num_frames {
            let remaining = &interleaved[frames_written * num_channels..];
            // SAFETY: `remaining` holds at least `(num_frames - frames_written)` whole frames.
            let result = unsafe {
                (self.alsa.pcm_writei)(
                    self.handle,
                    remaining.as_ptr().cast(),
                    (num_frames - frames_written) as c_ulong,
                )
            };
            if 0 <= result {
                frames_written += result as usize;
            } else {
                // SAFETY: recovering from an error code returned by the same handle.
                let recovered = unsafe { (self.alsa.pcm_recover)(self.handle, result as c_int, 1) };
                if recovered < 0 {
                    return Err(self.alsa.error("Audio playback failed", recovered));
                }
            }
        }

        Ok(())
    }
}

impl Drop for Pcm {
    fn drop(&mut self) {
        // SAFETY: the handle is open and not used after this.
        unsafe {
            (self.alsa.pcm_drain)(self.handle);
            (self.alsa.pcm_close)(self.handle);
        }
    }
}

/// The playback thread. Dropping it asks the thread to stop and waits for it.
pub struct Device {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Device {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

impl Device {
    /// Opens the default ALSA playback device and starts a thread that calls `fill` for samples.
    pub fn new(
        params: OutputDeviceParameters,
        mut fill: FillCallback,
        on_failure: FailureCallback,
    ) -> Result<Self, OutputError> {
        re_tracing::profile_function!();

        // Open on the calling thread so setup errors are reported synchronously.
        let pcm = Pcm::open(&params)?;

        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("audio_output".to_owned())
            .spawn({
                let stop = stop.clone();
                move || {
                    let mut buffer =
                        vec![0.0_f32; params.frames_per_callback * params.num_channels];
                    while !stop.load(Ordering::Acquire) {
                        fill(&params, &mut buffer);
                        if let Err(err) = pcm.write_all(&buffer, params.num_channels) {
                            on_failure(err);
                            break;
                        }
                    }
                }
            })
            .map_err(|err| {
                OutputError::Device(format!("Failed to spawn playback thread: {err}"))
            })?;

        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}
