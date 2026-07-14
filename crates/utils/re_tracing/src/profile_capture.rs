use std::sync::Arc;

use parking_lot::Mutex;

/// Captures puffin profile data in memory for a fixed number of frames.
///
/// Register with [`Self::start`], poll [`Self::is_done`] once per frame, then
/// call [`Self::finish`] to recover the collected frames as a [`puffin::FrameView`].
pub struct ProfileCapture {
    frames: Arc<Mutex<Vec<Arc<puffin::FrameData>>>>,
    sink_id: puffin::FrameSinkId,
    target_frames: usize,
}

impl ProfileCapture {
    /// Start capturing puffin profile data.
    ///
    /// Registers an in-memory sink with the global profiler that accumulates frames,
    /// until [`Self::is_done`] returns `true`.
    pub fn start(target_frames: usize) -> Self {
        puffin::set_scopes_on(true);

        let frames: Arc<Mutex<Vec<Arc<puffin::FrameData>>>> =
            Arc::new(Mutex::new(Vec::with_capacity(target_frames)));

        let sink_frames = frames.clone();
        let sink_id = puffin::GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            sink_frames.lock().push(frame);
        }));

        Self {
            frames,
            sink_id,
            target_frames,
        }
    }

    /// Whether enough frames have been collected.
    pub fn is_done(&self) -> bool {
        self.frames.lock().len() >= self.target_frames
    }

    /// Remove the sink and return the collected frames as a [`puffin::FrameView`].
    pub fn finish(self) -> puffin::FrameView {
        let Self {
            frames,
            sink_id,
            target_frames: _,
        } = self;

        puffin::GlobalProfiler::lock().remove_sink(sink_id);

        let mut view = puffin::FrameView::default();
        for frame in frames.lock().drain(..) {
            view.add_frame(frame);
        }
        view
    }
}
