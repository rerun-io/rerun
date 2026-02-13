const IMAGE_CHANNELS: u64 = 4;

#[derive(Debug, Clone, clap::Parser)]
pub struct ImageCommand {
    /// Log all images as static?
    #[arg(long = "static")]
    static_: bool,

    #[arg(long = "width", default_value_t = 1024)]
    width: u64,

    #[arg(long = "height", default_value_t = 1024)]
    height: u64,

    /// How many times we log the image.
    ///
    /// Each log call a single pixel changes.
    #[arg(long = "count", default_value_t = 20_000)]
    num_log_calls: usize,
}

impl ImageCommand {
    /// Log a single large image.
    pub fn run(self, rec: &rerun::RecordingStream) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let input = std::hint::black_box(self.prepare());
        self.execute(rec, input)
    }

    fn prepare(&self) -> Vec<u8> {
        re_tracing::profile_function!();

        vec![0u8; (self.width * self.height * IMAGE_CHANNELS) as usize]

        // Skip filling with non-zero values, this adds a bit too much extra overhead.
        // image.resize_with(
        //     (self.width * self.height * IMAGE_CHANNELS) as usize,
        //     || {
        //         i += 1;
        //         i as u8
        //     },
        // );
        // image
    }

    fn execute(
        self,
        rec: &rerun::RecordingStream,
        mut raw_image_data: Vec<u8>,
    ) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let entity_path = if self.static_ {
            "static_test_image"
        } else {
            "test_image"
        };

        for i in 0..self.num_log_calls {
            raw_image_data[i] = 255; // Change a single pixel of the image data, just to make sure we transmit something different each time.

            let image = {
                re_tracing::profile_scope!("rerun::Image::from_rgba32");
                rerun::Image::from_rgba32(
                    // TODO(andreas): We have to copy the image every time since the tensor buffer wants to
                    // take ownership of it.
                    // Note that even though our example here is *very* contrived, it's likely that a user
                    // will want to keep their image, so this copy is definitely part of our API overhead!
                    raw_image_data.clone(),
                    [self.width as _, self.height as _],
                )
            };

            re_tracing::profile_scope!("log");
            rec.log_with_static(entity_path, self.static_, &image)?;
        }

        Ok(())
    }
}
