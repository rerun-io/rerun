fn main() {
    cfg_aliases::cfg_aliases! {
        native: { not(target_arch = "wasm32") },
        with_dav1d: { all(feature = "av1", native) },
        with_ffmpeg: { all(feature= "ffmpeg", native) }
    }
}
