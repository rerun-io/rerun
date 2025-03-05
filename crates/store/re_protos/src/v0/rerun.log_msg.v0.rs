// This file is @generated by prost-build.
/// TODO(#8631): Remove `LogMsg`
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogMsg {
    #[prost(oneof = "log_msg::Msg", tags = "1, 2, 3")]
    pub msg: ::core::option::Option<log_msg::Msg>,
}
/// Nested message and enum types in `LogMsg`.
pub mod log_msg {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Msg {
        /// A message that contains a new store info.
        #[prost(message, tag = "1")]
        SetStoreInfo(super::SetStoreInfo),
        /// A message that contains an Arrow-IPC encoded message.
        #[prost(message, tag = "2")]
        ArrowMsg(super::ArrowMsg),
        /// A message that contains a blueprint activation command.
        #[prost(message, tag = "3")]
        BlueprintActivationCommand(super::BlueprintActivationCommand),
    }
}
impl ::prost::Name for LogMsg {
    const NAME: &'static str = "LogMsg";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.LogMsg".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.LogMsg".into()
    }
}
/// Corresponds to `LogMsg::SetStoreInfo`. Used to identify a recording.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetStoreInfo {
    /// A time-based UID that is used to determine how a `StoreInfo` fits in the global ordering of events.
    #[prost(message, optional, tag = "1")]
    pub row_id: ::core::option::Option<super::super::common::v0::Tuid>,
    /// The new store info.
    #[prost(message, optional, tag = "2")]
    pub info: ::core::option::Option<StoreInfo>,
}
impl ::prost::Name for SetStoreInfo {
    const NAME: &'static str = "SetStoreInfo";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.SetStoreInfo".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.SetStoreInfo".into()
    }
}
/// Corresponds to `LogMsg::ArrowMsg`. Used to transmit actual data.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ArrowMsg {
    /// The ID of the store that this message is for.
    #[prost(message, optional, tag = "1")]
    pub store_id: ::core::option::Option<super::super::common::v0::StoreId>,
    /// Compression algorithm used.
    #[prost(enumeration = "Compression", tag = "2")]
    pub compression: i32,
    #[prost(int32, tag = "3")]
    pub uncompressed_size: i32,
    /// Encoding of the payload.
    #[prost(enumeration = "Encoding", tag = "4")]
    pub encoding: i32,
    /// Arrow-IPC encoded schema and chunk, compressed according to the `compression` field.
    #[prost(bytes = "vec", tag = "1000")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
impl ::prost::Name for ArrowMsg {
    const NAME: &'static str = "ArrowMsg";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.ArrowMsg".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.ArrowMsg".into()
    }
}
/// Corresponds to `LogMsg::BlueprintActivationCommand`.
///
/// Used for activating a blueprint once it has been fully transmitted,
/// because showing a blueprint before it is fully transmitted can lead to
/// a confusing user experience, or inconsistent results due to heuristics.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlueprintActivationCommand {
    /// The ID of the blueprint to activate.
    #[prost(message, optional, tag = "1")]
    pub blueprint_id: ::core::option::Option<super::super::common::v0::StoreId>,
    /// Whether to make the blueprint active immediately.
    #[prost(bool, tag = "2")]
    pub make_active: bool,
    /// Whether to make the blueprint the default.
    #[prost(bool, tag = "3")]
    pub make_default: bool,
}
impl ::prost::Name for BlueprintActivationCommand {
    const NAME: &'static str = "BlueprintActivationCommand";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.BlueprintActivationCommand".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.BlueprintActivationCommand".into()
    }
}
/// Information about a recording or blueprint.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreInfo {
    /// Unique ID of the recording.
    #[prost(message, optional, tag = "1")]
    pub store_id: ::core::option::Option<super::super::common::v0::StoreId>,
    /// Where the recording came from.
    /// TODO(grtlr): Will be removed after #9178.
    #[deprecated]
    #[prost(message, optional, tag = "2")]
    pub store_source: ::core::option::Option<StoreSource>,
    /// Version of the store crate.
    /// TODO(grtlr): Will be removed after #9178.
    #[deprecated]
    #[prost(message, optional, tag = "3")]
    pub store_version: ::core::option::Option<StoreVersion>,
}
impl ::prost::Name for StoreInfo {
    const NAME: &'static str = "StoreInfo";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.StoreInfo".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.StoreInfo".into()
    }
}
/// The source of a recording or blueprint.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreSource {
    /// Determines what is encoded in `extra`.
    #[prost(enumeration = "StoreSourceKind", tag = "1")]
    pub kind: i32,
    /// Store source payload. See `StoreSourceKind` for what exactly is encoded here.
    #[prost(message, optional, tag = "2")]
    pub extra: ::core::option::Option<StoreSourceExtra>,
}
impl ::prost::Name for StoreSource {
    const NAME: &'static str = "StoreSource";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.StoreSource".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.StoreSource".into()
    }
}
/// A newtype for `StoreSource` payload.
///
/// This exists to that we can implement conversions on the newtype for convenience.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StoreSourceExtra {
    #[prost(bytes = "vec", tag = "1")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
}
impl ::prost::Name for StoreSourceExtra {
    const NAME: &'static str = "StoreSourceExtra";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.StoreSourceExtra".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.StoreSourceExtra".into()
    }
}
/// Version of the Python SDK that created the recording.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PythonVersion {
    #[prost(int32, tag = "1")]
    pub major: i32,
    #[prost(int32, tag = "2")]
    pub minor: i32,
    #[prost(int32, tag = "3")]
    pub patch: i32,
    #[prost(string, tag = "4")]
    pub suffix: ::prost::alloc::string::String,
}
impl ::prost::Name for PythonVersion {
    const NAME: &'static str = "PythonVersion";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.PythonVersion".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.PythonVersion".into()
    }
}
/// Information about the Rust SDK that created the recording.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CrateInfo {
    /// Version of the Rust compiler used to compile the SDK.
    #[prost(string, tag = "1")]
    pub rustc_version: ::prost::alloc::string::String,
    /// Version of LLVM used by the Rust compiler.
    #[prost(string, tag = "2")]
    pub llvm_version: ::prost::alloc::string::String,
}
impl ::prost::Name for CrateInfo {
    const NAME: &'static str = "CrateInfo";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.CrateInfo".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.CrateInfo".into()
    }
}
/// A recording which came from a file.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct FileSource {
    #[prost(enumeration = "FileSourceKind", tag = "1")]
    pub kind: i32,
}
impl ::prost::Name for FileSource {
    const NAME: &'static str = "FileSource";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.FileSource".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.FileSource".into()
    }
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct StoreVersion {
    /// Crate version encoded using our custom scheme.
    ///
    /// See `CrateVersion` in `re_build_info`.
    #[prost(int32, tag = "1")]
    pub crate_version_bits: i32,
}
impl ::prost::Name for StoreVersion {
    const NAME: &'static str = "StoreVersion";
    const PACKAGE: &'static str = "rerun.log_msg.v0";
    fn full_name() -> ::prost::alloc::string::String {
        "rerun.log_msg.v0.StoreVersion".into()
    }
    fn type_url() -> ::prost::alloc::string::String {
        "/rerun.log_msg.v0.StoreVersion".into()
    }
}
/// The type of compression used on the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Compression {
    /// No compression.
    None = 0,
    /// LZ4 block compression.
    Lz4 = 1,
}
impl Compression {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Lz4 => "LZ4",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "NONE" => Some(Self::None),
            "LZ4" => Some(Self::Lz4),
            _ => None,
        }
    }
}
/// The encoding of the message payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Encoding {
    /// We don't know what encoding the payload is in.
    Unknown = 0,
    /// The payload is encoded as Arrow-IPC.
    ArrowIpc = 1,
}
impl Encoding {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::ArrowIpc => "ARROW_IPC",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "ARROW_IPC" => Some(Self::ArrowIpc),
            _ => None,
        }
    }
}
/// What kind of source a recording comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StoreSourceKind {
    /// We don't know anything about the source of this recording.
    ///
    /// `extra` is unused.
    UnknownKind = 0,
    /// The recording came from the C++ SDK.
    ///
    /// `extra` is unused.
    CSdk = 1,
    /// The recording came from the Python SDK.
    ///
    /// `extra` is `PythonVersion`.
    PythonSdk = 2,
    /// The recording came from the Rust SDK.
    ///
    /// `extra` is `CrateInfo`.
    RustSdk = 3,
    /// The recording came from a file.
    ///
    /// `extra` is `FileSource`.
    File = 4,
    /// The recording came from some action in the viewer.
    ///
    /// `extra` is unused.
    Viewer = 5,
    /// The recording came from some other source.
    ///
    /// `extra` is a string.
    Other = 6,
}
impl StoreSourceKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::UnknownKind => "UNKNOWN_KIND",
            Self::CSdk => "C_SDK",
            Self::PythonSdk => "PYTHON_SDK",
            Self::RustSdk => "RUST_SDK",
            Self::File => "FILE",
            Self::Viewer => "VIEWER",
            Self::Other => "OTHER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN_KIND" => Some(Self::UnknownKind),
            "C_SDK" => Some(Self::CSdk),
            "PYTHON_SDK" => Some(Self::PythonSdk),
            "RUST_SDK" => Some(Self::RustSdk),
            "FILE" => Some(Self::File),
            "VIEWER" => Some(Self::Viewer),
            "OTHER" => Some(Self::Other),
            _ => None,
        }
    }
}
/// Determines where the file came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FileSourceKind {
    /// We don't know where the file came from.
    UnknownSource = 0,
    /// The file came from the command line.
    Cli = 1,
    /// The file was served over HTTP.
    Uri = 2,
    /// The file was dragged into the viewer.
    DragAndDrop = 3,
    /// The file was opened using a file dialog.
    FileDialog = 4,
    /// The recording was produced using a data loader, such as when logging a mesh file.
    Sdk = 5,
}
impl FileSourceKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::UnknownSource => "UNKNOWN_SOURCE",
            Self::Cli => "CLI",
            Self::Uri => "URI",
            Self::DragAndDrop => "DRAG_AND_DROP",
            Self::FileDialog => "FILE_DIALOG",
            Self::Sdk => "SDK",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN_SOURCE" => Some(Self::UnknownSource),
            "CLI" => Some(Self::Cli),
            "URI" => Some(Self::Uri),
            "DRAG_AND_DROP" => Some(Self::DragAndDrop),
            "FILE_DIALOG" => Some(Self::FileDialog),
            "SDK" => Some(Self::Sdk),
            _ => None,
        }
    }
}
