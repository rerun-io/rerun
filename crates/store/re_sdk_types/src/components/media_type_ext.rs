use super::MediaType;
use re_rvl::RosRvlMetadata;

// TODO(#2388): come up with some DSL in `re_type_definitions` so that we can declare these
// constants directly in there.
impl MediaType {
    /// Plain text.
    pub const TEXT: &'static str = "text/plain";

    /// Markdown.
    ///
    /// <https://www.iana.org/assignments/media-types/text/markdown>
    pub const MARKDOWN: &'static str = "text/markdown";

    // -------------------------------------------------------
    // Images:

    /// [JPEG image](https://en.wikipedia.org/wiki/JPEG): `image/jpeg`.
    pub const JPEG: &'static str = "image/jpeg";

    /// [PNG image](https://en.wikipedia.org/wiki/PNG): `image/png`.
    ///
    /// <https://www.iana.org/assignments/media-types/image/png>
    pub const PNG: &'static str = "image/png";

    /// [TIFF image](https://en.wikipedia.org/wiki/TIFF): `image/tiff`.
    ///
    /// <https://www.iana.org/assignments/media-types/image/tiff>
    pub const TIFF: &'static str = "image/tiff";

    // -------------------------------------------------------
    // Meshes:

    /// [`glTF`](https://en.wikipedia.org/wiki/GlTF).
    ///
    /// <https://www.iana.org/assignments/media-types/model/gltf+json>
    pub const GLTF: &'static str = "model/gltf+json";

    /// Binary [`glTF`](https://en.wikipedia.org/wiki/GlTF).
    ///
    /// <https://www.iana.org/assignments/media-types/model/gltf-binary>
    pub const GLB: &'static str = "model/gltf-binary";

    /// [Wavefront .obj](https://en.wikipedia.org/wiki/Wavefront_.obj_file).
    ///
    /// <https://www.iana.org/assignments/media-types/model/obj>
    pub const OBJ: &'static str = "model/obj";

    /// [Stereolithography Model `stl`](https://en.wikipedia.org/wiki/STL_(file_format)): `model/stl`.
    ///
    /// Either binary or ASCII.
    /// <https://www.iana.org/assignments/media-types/model/stl>
    pub const STL: &'static str = "model/stl";

    /// [COLLADA `.dae`](https://en.wikipedia.org/wiki/COLLADA): `model/collada+xml`.
    ///
    /// <https://www.iana.org/assignments/media-types/model/vnd.collada+xml>
    pub const DAE: &'static str = "model/vnd.collada+xml";

    // -------------------------------------------------------
    // Compressed Depth Data:

    /// RVL compressed depth: `application/rvl`.
    ///
    /// Run length encoding and Variable Length encoding schemes (RVL) compressed depth data format.
    /// <https://www.microsoft.com/en-us/research/wp-content/uploads/2018/09/p100-wilson.pdf>: `application/rvl`.
    pub const RVL: &'static str = "application/rvl";

    // -------------------------------------------------------
    // Videos:

    /// [MP4 video](https://en.wikipedia.org/wiki/MP4_file_format): `video/mp4`.
    ///
    /// <https://www.iana.org/assignments/media-types/video/mp4>
    pub const MP4: &'static str = "video/mp4";

    // -------------------------------------------------------
    // Audio:

    /// [AAC audio](https://en.wikipedia.org/wiki/Advanced_Audio_Coding) in a raw ADTS stream: `audio/aac`.
    ///
    /// <https://www.iana.org/assignments/media-types/audio/aac>
    pub const AAC: &'static str = "audio/aac";

    /// [FLAC audio](https://en.wikipedia.org/wiki/FLAC): `audio/flac`.
    pub const FLAC: &'static str = "audio/flac";

    /// [M4A audio](https://en.wikipedia.org/wiki/MP4_file_format) (AAC in an MP4 container): `audio/mp4`.
    ///
    /// <https://www.iana.org/assignments/media-types/audio/mp4>
    pub const M4A: &'static str = "audio/mp4";

    /// [MP3 audio](https://en.wikipedia.org/wiki/MP3): `audio/mpeg`.
    ///
    /// <https://www.iana.org/assignments/media-types/audio/mpeg>
    pub const MP3: &'static str = "audio/mpeg";

    /// [Ogg audio](https://en.wikipedia.org/wiki/Ogg) (Vorbis or Opus): `audio/ogg`.
    ///
    /// <https://www.iana.org/assignments/media-types/audio/ogg>
    pub const OGG: &'static str = "audio/ogg";

    /// [WAV audio](https://en.wikipedia.org/wiki/WAV): `audio/wav`.
    pub const WAV: &'static str = "audio/wav";

    // -------------------------------------------------------
    // Robotics formats:

    /// Rerun recording data: `application/x-rerun`.
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    pub const RRD: &'static str = "application/x-rerun";

    /// [MCAP](https://mcap.dev/) recording: `application/x-mcap`.
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    pub const MCAP: &'static str = "application/x-mcap";

    // -------------------------------------------------------
    // Geometry:

    /// [PLY (Polygon File Format)](https://en.wikipedia.org/wiki/PLY_(file_format)): `application/x-ply`.
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    pub const PLY: &'static str = "application/x-ply";
}

impl MediaType {
    /// `text/plain`
    #[inline]
    pub fn plain_text() -> Self {
        Self(Self::TEXT.into())
    }

    /// `text/markdown`
    #[inline]
    pub fn markdown() -> Self {
        Self(Self::MARKDOWN.into())
    }

    // -------------------------------------------------------
    // Images:

    /// `image/jpeg`
    #[inline]
    pub fn jpeg() -> Self {
        Self(Self::JPEG.into())
    }

    /// `image/png`
    #[inline]
    pub fn png() -> Self {
        Self(Self::PNG.into())
    }

    /// `image/tiff`
    #[inline]
    pub fn tiff() -> Self {
        Self(Self::TIFF.into())
    }

    // -------------------------------------------------------
    // Meshes:

    /// `model/gltf+json`
    #[inline]
    pub fn gltf() -> Self {
        Self(Self::GLTF.into())
    }

    /// `model/gltf-binary`
    #[inline]
    pub fn glb() -> Self {
        Self(Self::GLB.into())
    }

    /// `model/obj`
    #[inline]
    pub fn obj() -> Self {
        Self(Self::OBJ.into())
    }

    /// `model/stl`
    #[inline]
    pub fn stl() -> Self {
        Self(Self::STL.into())
    }

    /// `model/vnd.collada+xml`
    #[inline]
    pub fn dae() -> Self {
        Self(Self::DAE.into())
    }

    // -------------------------------------------------------
    // Compressed Depth Data:

    /// `application/rvl`
    #[inline]
    pub fn rvl() -> Self {
        Self(Self::RVL.into())
    }

    // -------------------------------------------------------
    // Video:

    /// `video/mp4`
    #[inline]
    pub fn mp4() -> Self {
        Self(Self::MP4.into())
    }

    // -------------------------------------------------------
    // Audio:

    /// `audio/aac`
    #[inline]
    pub fn aac() -> Self {
        Self(Self::AAC.into())
    }

    /// `audio/flac`
    #[inline]
    pub fn flac() -> Self {
        Self(Self::FLAC.into())
    }

    /// `audio/mp4`
    #[inline]
    pub fn m4a() -> Self {
        Self(Self::M4A.into())
    }

    /// `audio/mpeg`
    #[inline]
    pub fn mp3() -> Self {
        Self(Self::MP3.into())
    }

    /// `audio/ogg`
    #[inline]
    pub fn ogg() -> Self {
        Self(Self::OGG.into())
    }

    /// `audio/wav`
    #[inline]
    pub fn wav() -> Self {
        Self(Self::WAV.into())
    }

    // -------------------------------------------------------
    // Robotics formats:

    /// `application/x-rerun`
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    #[inline]
    pub fn rrd() -> Self {
        Self(Self::RRD.into())
    }

    /// `application/x-mcap`
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    #[inline]
    pub fn mcap() -> Self {
        Self(Self::MCAP.into())
    }

    // -------------------------------------------------------
    // Geometry:

    /// `application/x-ply`
    ///
    /// This is not a standardized format and mostly exists here to accommodate file type detection
    /// in the data loader.
    #[inline]
    pub fn ply() -> Self {
        Self(Self::PLY.into())
    }
}

impl MediaType {
    /// Returns the media type as a string slice, e.g. "text/plain".
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl MediaType {
    /// Tries to guess the media type of the file at `path` based on its extension.
    #[inline]
    pub fn guess_from_path(path: impl AsRef<std::path::Path>) -> Option<Self> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str().map(|s| s.to_lowercase()));

        match extension.as_deref() {
            // `mime_guess2` considers `.obj` to be a tgif… but really it's way more likely to be an obj.
            Some("obj") => {
                return Some(Self::obj());
            }
            // `mime_guess2` doesn't know about PLY, but we use `application/x-ply`.
            Some("ply") => {
                return Some(Self::ply());
            }
            // `mime_guess2` considers `.stl` to be a `application/vnd.ms-pki.stl`.
            Some("stl") => {
                return Some(Self::stl());
            }
            // `mime_guess2` considers `.m4a` to be the non-standard `audio/m4a`.
            Some("m4a") => {
                return Some(Self::m4a());
            }
            _ => {}
        }

        mime_guess2::from_path(path)
            .first_raw()
            .map(ToOwned::to_owned)
            .map(Into::into)
    }

    /// Tries to guess the media type of the file at `path` based on its contents (magic bytes).
    #[inline]
    pub fn guess_from_data(data: &[u8]) -> Option<Self> {
        fn glb_matcher(buf: &[u8]) -> bool {
            buf.len() >= 4 && buf[0] == b'g' && buf[1] == b'l' && buf[2] == b'T' && buf[3] == b'F'
        }

        fn stl_matcher(buf: &[u8]) -> bool {
            // ASCII STL
            buf.len() >= 5
                && buf[0] == b's'
                && buf[1] == b'o'
                && buf[2] == b'l'
                && buf[3] == b'i'
                && buf[4] == b'd'
            // Binary STL is hard to infer since it starts with an 80 byte header that is commonly ignored, see
            // https://en.wikipedia.org/wiki/STL_(file_format)#Binary
        }

        fn dae_matcher(buf: &[u8]) -> bool {
            // COLLADA .dae files are XML, so we can look for the <COLLADA> tag.
            buf.starts_with(b"<COLLADA>")
        }

        fn rvl_matcher(buf: &[u8]) -> bool {
            const MAX_REASONABLE_DIMENSION: u32 = 65_536;

            let Ok(metadata) = RosRvlMetadata::parse(buf) else {
                return false;
            };

            let quant_a = metadata.depth_quant_a;
            let quant_b = metadata.depth_quant_b;

            if !quant_a.is_finite() || !quant_b.is_finite() {
                return false;
            }

            // Reject unreasonable values to reduce false positives.
            if !(0.0..=1e4).contains(&quant_a) || quant_b.abs() > 1e4 {
                return false;
            }

            metadata.width <= MAX_REASONABLE_DIMENSION
                && metadata.height <= MAX_REASONABLE_DIMENSION
        }

        fn rrd_matcher(buf: &[u8]) -> bool {
            // "RRF2" (current) or "RRF1"/"RRF0" (legacy)
            buf.starts_with(b"RRF2") || buf.starts_with(b"RRF1") || buf.starts_with(b"RRF0")
        }

        fn mcap_matcher(buf: &[u8]) -> bool {
            // MCAP magic
            buf.starts_with(b"\x89MCAP0\r\n")
        }

        fn ply_matcher(buf: &[u8]) -> bool {
            buf.starts_with(b"ply\n") || buf.starts_with(b"ply\r\n")
        }

        // NOTE:
        // - gltf is simply json, so no magic byte
        //   (also most gltf files contain file:// links, so not much point in sending that to
        //   Rerun for now…)
        // - obj is simply text, so no magic byte

        let mut inferer = infer::Infer::new();
        // Custom matchers take precedence over the built-in ones, so these
        // normalize `infer`'s `audio/x-wav`, `audio/x-flac`, and `audio/m4a`
        // to the media types we use everywhere else:
        inferer.add(Self::FLAC, "flac", infer::audio::is_flac);
        inferer.add(Self::M4A, "m4a", infer::audio::is_m4a);
        inferer.add(Self::WAV, "wav", infer::audio::is_wav);
        inferer.add(Self::RRD, "rrd", rrd_matcher);
        inferer.add(Self::MCAP, "mcap", mcap_matcher);
        inferer.add(Self::PLY, "ply", ply_matcher);
        inferer.add(Self::GLB, "glb", glb_matcher);
        inferer.add(Self::STL, "stl", stl_matcher);
        inferer.add(Self::DAE, "dae", dae_matcher);
        inferer.add(Self::RVL, "rvl", rvl_matcher);

        inferer
            .get(data)
            .map(|v| v.mime_type())
            .map(ToOwned::to_owned)
            .map(Into::into)
    }

    /// Returns `opt` if not `None`, otherwise tries to guess a media type using [`Self::guess_from_path`].
    #[inline]
    pub fn or_guess_from_path(
        opt: Option<Self>,
        path: impl AsRef<std::path::Path>,
    ) -> Option<Self> {
        opt.or_else(|| Self::guess_from_path(path))
    }

    /// Returns `opt` if not `None`, otherwise tries to guess a media type using [`Self::guess_from_data`].
    #[inline]
    pub fn or_guess_from_data(opt: Option<Self>, data: &[u8]) -> Option<Self> {
        opt.or_else(|| Self::guess_from_data(data))
    }

    /// Return e.g. "jpg" for `image/jpeg`.
    pub fn file_extension(&self) -> Option<&'static str> {
        match self.as_str() {
            // Special-case some where there are multiple extensions:
            Self::DAE => Some("dae"),
            Self::JPEG => Some("jpg"),
            Self::M4A => Some("m4a"),
            Self::MARKDOWN => Some("md"),
            Self::MP3 => Some("mp3"),
            Self::OGG => Some("ogg"),
            Self::RVL => Some("rvl"),
            Self::STL => Some("stl"),
            Self::TEXT => Some("txt"),

            // Custom MIME types not known to mime_guess2:
            Self::MCAP => Some("mcap"),
            Self::PLY => Some("ply"),
            Self::RRD => Some("rrd"),

            _ => {
                let alternatives = mime_guess2::get_mime_extensions_str(&self.0)?;

                // Return shortest alternative:
                alternatives.iter().min_by_key(|s| s.len()).copied()
            }
        }
    }

    /// Returns `true` if this is an image media type.
    pub fn is_image(&self) -> bool {
        self.as_str().starts_with("image/")
    }

    /// Returns `true` if this is an video media type.
    pub fn is_video(&self) -> bool {
        self.as_str().starts_with("video/")
    }

    /// Returns `true` if this is an audio media type.
    pub fn is_audio(&self) -> bool {
        self.as_str().starts_with("audio/")
    }
}

impl std::fmt::Display for MediaType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for MediaType {
    #[inline]
    fn default() -> Self {
        // https://www.rfc-editor.org/rfc/rfc2046.txt
        // "The "octet-stream" subtype is used to indicate that a body contains arbitrary binary data."
        Self("application/octet-stream".into())
    }
}

#[test]
fn test_media_type_extension() {
    assert_eq!(MediaType::glb().file_extension(), Some("glb"));
    assert_eq!(MediaType::gltf().file_extension(), Some("gltf"));
    assert_eq!(MediaType::jpeg().file_extension(), Some("jpg"));
    assert_eq!(MediaType::mp4().file_extension(), Some("mp4"));
    assert_eq!(MediaType::markdown().file_extension(), Some("md"));
    assert_eq!(MediaType::plain_text().file_extension(), Some("txt"));
    assert_eq!(MediaType::png().file_extension(), Some("png"));
    assert_eq!(MediaType::ply().file_extension(), Some("ply"));
    assert_eq!(MediaType::rvl().file_extension(), Some("rvl"));
    assert_eq!(MediaType::stl().file_extension(), Some("stl"));
    assert_eq!(MediaType::aac().file_extension(), Some("aac"));
    assert_eq!(MediaType::flac().file_extension(), Some("flac"));
    assert_eq!(MediaType::m4a().file_extension(), Some("m4a"));
    assert_eq!(MediaType::mp3().file_extension(), Some("mp3"));
    assert_eq!(MediaType::ogg().file_extension(), Some("ogg"));
    assert_eq!(MediaType::wav().file_extension(), Some("wav"));
}

#[test]
fn test_guess_audio() {
    for (ext, expected) in [
        ("aac", MediaType::aac()),
        ("flac", MediaType::flac()),
        ("m4a", MediaType::m4a()),
        ("mp3", MediaType::mp3()),
        ("ogg", MediaType::ogg()),
        ("wav", MediaType::wav()),
        ("WAV", MediaType::wav()),
    ] {
        assert_eq!(
            MediaType::guess_from_path(format!("sound.{ext}")),
            Some(expected),
            "extension {ext}"
        );
    }

    let wav = include_bytes!("../../../../../tests/assets/audio/sine_440hz_2s.wav");
    assert_eq!(MediaType::guess_from_data(wav), Some(MediaType::wav()));

    let aac = include_bytes!("../../../../../tests/assets/audio/sine_440hz_2s.aac");
    assert_eq!(MediaType::guess_from_data(aac), Some(MediaType::aac()));

    assert_eq!(
        MediaType::guess_from_data(b"fLaC\0\0\0\x22"),
        Some(MediaType::flac())
    );
    assert_eq!(
        MediaType::guess_from_data(b"OggS\0\x02"),
        Some(MediaType::ogg())
    );
    assert_eq!(
        MediaType::guess_from_data(b"\xff\xfb\x90\x64"),
        Some(MediaType::mp3())
    );

    assert!(MediaType::wav().is_audio());
    assert!(!MediaType::mp4().is_audio());
}

#[test]
fn test_guess_from_path_ply() {
    assert_eq!(
        MediaType::guess_from_path("mesh.ply"),
        Some(MediaType::ply())
    );
}

#[test]
fn test_guess_from_data_rvl() {
    assert_eq!(MediaType::guess_from_data(&[]), None);

    let valid_rvl = build_rvl_header(640, 480, 1.0, 0.0);
    assert_eq!(
        MediaType::guess_from_data(&valid_rvl),
        Some(MediaType::rvl())
    );

    let mut invalid_quant = valid_rvl.clone();
    invalid_quant[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(MediaType::guess_from_data(&invalid_quant), None);

    let mut invalid_quant_magnitude = valid_rvl.clone();
    invalid_quant_magnitude[8..12].copy_from_slice(&(20_000.0f32).to_le_bytes());
    assert_eq!(MediaType::guess_from_data(&invalid_quant_magnitude), None);

    let mut zero_width = valid_rvl.clone();
    zero_width[12..16].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(MediaType::guess_from_data(&zero_width), None);

    let mut huge_height = valid_rvl.clone();
    huge_height[16..20].copy_from_slice(&(100_000u32).to_le_bytes());
    assert_eq!(MediaType::guess_from_data(&huge_height), None);

    assert_eq!(MediaType::guess_from_data(b"Hello, World!"), None);
}

#[cfg(test)]
fn build_rvl_header(width: u32, height: u32, depth_quant_a: f32, depth_quant_b: f32) -> Vec<u8> {
    // 12 bytes config header + 8 bytes resolution + a few payload bytes.
    let mut data = vec![0u8; 24];
    data[4..8].copy_from_slice(&depth_quant_a.to_le_bytes());
    data[8..12].copy_from_slice(&depth_quant_b.to_le_bytes());
    data[12..16].copy_from_slice(&width.to_le_bytes());
    data[16..20].copy_from_slice(&height.to_le_bytes());
    data
}
