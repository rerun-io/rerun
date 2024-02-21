use super::MediaType;

// TODO(#2388): come up with some DSL in our flatbuffers definitions so that we can declare these
// constants directly in there.
impl MediaType {
    /// Plain text.
    pub const TEXT: &'static str = "text/plain";

    /// Markdown.
    ///
    /// <https://www.iana.org/assignments/media-types/text/markdown>
    pub const MARKDOWN: &'static str = "text/markdown";

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
}

impl MediaType {
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
            // `mime_guess2` considers `.stl` to be a `application/vnd.ms-pki.stl`.
            Some("stl") => {
                return Some(Self::stl());
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
                && buf[3] == b'd'
            // Binary STL is hard to infer since it starts with an 80 byte header that is commonly ignored, see
            // https://en.wikipedia.org/wiki/STL_(file_format)#Binary
        }

        // NOTE:
        // - gltf is simply json, so no magic byte
        //   (also most gltf files contain file:// links, so not much point in sending that to
        //   Rerun for now…)
        // - obj is simply text, so no magic byte

        let mut inferer = infer::Infer::new();
        inferer.add(Self::GLB, "", glb_matcher);
        inferer.add(Self::STL, "", stl_matcher);

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
}

impl std::fmt::Display for MediaType {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
