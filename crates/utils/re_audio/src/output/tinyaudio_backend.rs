use super::{FailureCallback, FillCallback, OutputDeviceParameters, OutputError};

/// A running `tinyaudio` output stream. Dropping it stops playback.
pub struct Device(#[expect(dead_code)] tinyaudio::OutputDevice);

impl Device {
    /// Opens the default output device and starts calling `fill` for samples.
    pub fn new(
        params: OutputDeviceParameters,
        mut fill: FillCallback,
        _on_failure: FailureCallback,
    ) -> Result<Self, OutputError> {
        re_tracing::profile_function!();
        let OutputDeviceParameters {
            sample_rate,
            num_channels,
            frames_per_callback,
        } = params;
        let backend_params = tinyaudio::OutputDeviceParameters {
            channels_count: num_channels,
            sample_rate: sample_rate as usize,
            channel_sample_count: frames_per_callback,
        };
        tinyaudio::run_output_device(backend_params, move |out| fill(&params, out))
            .map(Self)
            .map_err(|err| OutputError::Device(err.to_string()))
    }
}
