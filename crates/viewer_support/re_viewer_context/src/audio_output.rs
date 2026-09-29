/// Audio output supplied by the application to views.
pub trait AudioOutput: Sync {
    /// Stage a stream for the current UI pass.
    fn request(&self, request: re_audio::StreamRequest);

    /// The terminal output-device error, if opening or playback failed.
    fn output_error(&self) -> Option<re_audio::OutputError>;
}
