use std::io::Write as _;

use camino::{Utf8Path, Utf8PathBuf};
use re_log_channel::{UiCallback, ViewerControlError};
use re_protos::viewer_control::v1alpha1::{
    CaptureProfileTraceRequest, CaptureProfileTraceResponse, ViewerControlResponse,
};
use re_tracing::reexports::puffin::FrameView;

use super::App;

/// How many frames a profile capture records by default.
const DEFAULT_PROFILE_CAPTURE_FRAMES: u32 = 5;

/// The most frames a viewer-control request may ask for.
///
/// At 60 FPS this records for 5 seconds, which fits within the 10 second deadline of a
/// viewer-control request.
const MAX_PROFILE_CAPTURE_FRAMES: u32 = 300;

impl App {
    /// Start recording a profile of the next frames, and answer `on_done` once it has been written.
    pub(super) fn begin_profile_capture(
        &mut self,
        request: CaptureProfileTraceRequest,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    ) {
        let CaptureProfileTraceRequest {
            file_path,
            num_frames,
        } = request;

        if self.profile_capture.is_some() {
            on_done.call(Err(ViewerControlError::failed_precondition(
                "Another profile capture is still running",
            )));
            return;
        }

        if let Some(num_frames) = num_frames
            && !(1..=MAX_PROFILE_CAPTURE_FRAMES).contains(&num_frames)
        {
            on_done.call(Err(ViewerControlError::invalid_argument(format!(
                "`num_frames` is {num_frames}, but has to be between 1 and {MAX_PROFILE_CAPTURE_FRAMES}"
            ))));
            return;
        }

        self.profile_capture = Some(PendingProfileCapture::to_path(
            num_frames,
            file_path.into(),
            on_done,
        ));
    }
}

/// An in-memory profile capture, and where its result goes once enough frames are recorded.
pub struct PendingProfileCapture {
    capture: re_tracing::ProfileCapture,
    target: ProfileTraceTarget,
}

impl PendingProfileCapture {
    /// Record the default number of frames, then ask the user where to save them.
    pub fn to_save_dialog() -> Self {
        Self {
            capture: re_tracing::ProfileCapture::start(DEFAULT_PROFILE_CAPTURE_FRAMES as usize),
            target: ProfileTraceTarget::SaveDialog,
        }
    }

    /// Record `num_frames` frames, or the default number, then write them to `path` and answer
    /// `on_done`.
    fn to_path(
        num_frames: Option<u32>,
        path: Utf8PathBuf,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    ) -> Self {
        Self {
            capture: re_tracing::ProfileCapture::start(
                num_frames.unwrap_or(DEFAULT_PROFILE_CAPTURE_FRAMES) as usize,
            ),
            target: ProfileTraceTarget::SaveToPath { path, on_done },
        }
    }

    /// Save the capture in `slot` once it has enough frames, and request another frame until then.
    pub fn poll(slot: &mut Option<Self>, egui_ctx: &egui::Context) {
        let Some(pending) = slot else {
            return;
        };

        if !pending.capture.is_done() {
            egui_ctx.request_repaint();
            return;
        }

        if let Some(Self { capture, target }) = slot.take() {
            target.save(&capture.finish());
        }
    }
}

/// Where a finished profile capture is written.
enum ProfileTraceTarget {
    /// Ask the user for a path with a save dialog.
    SaveDialog,

    /// Write to this path, then answer the viewer-control request that asked for it.
    SaveToPath {
        path: Utf8PathBuf,
        on_done: UiCallback<Result<ViewerControlResponse, ViewerControlError>>,
    },
}

impl ProfileTraceTarget {
    fn save(self, view: &FrameView) {
        match self {
            Self::SaveDialog => {
                let Some(path) = rfd::FileDialog::new()
                    .set_file_name("rerun.puffin")
                    .set_title("Save profile trace")
                    .add_filter("Puffin profile", &["puffin"])
                    .save_file()
                else {
                    re_log::info!("Profile trace capture cancelled by user.");
                    return;
                };
                let result = Utf8PathBuf::try_from(path)
                    .map_err(anyhow::Error::from)
                    .and_then(|path| write_profile_trace(view, &path));
                if let Err(err) = result {
                    re_log::error!("Failed to save profile trace: {err}");
                }
            }
            Self::SaveToPath { path, on_done } => {
                let result = write_profile_trace(view, &path)
                    .map(|file_path| {
                        CaptureProfileTraceResponse {
                            num_frames: view.recent_frames().count() as u32,
                            file_path: file_path.into_string(),
                        }
                        .into()
                    })
                    .map_err(|err| {
                        ViewerControlError::internal(format!(
                            "Failed to save profile trace: {err}\nFile path: {path}"
                        ))
                    });
                on_done.call(result);
            }
        }
    }
}

/// Write `view` to `path`, and return the absolute path of the written file.
fn write_profile_trace(view: &FrameView, path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    view.write(&mut writer)?;
    writer.flush()?;

    let path = path.canonicalize_utf8()?;
    re_log::info!("Saved profile trace to {path}");
    Ok(path)
}
