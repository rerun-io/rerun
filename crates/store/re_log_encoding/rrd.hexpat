// This is an incomplete Pattern Language definition for the ImHex editor.
//
// To use it, load any RRD file in the ImHex editor, and import this file as a pattern definition.
//
// Unfortunately this merely covers the very outer layers of the Rerun RRD file encoding.
// The RRD protocol relies on several layers of encodings that require code generation as well as third
// party libraries. To provide a proper pattern definition for the RRD protocol, not only a full Rerun
// ImHex plugin would be required, but the existing codebase would also need custom Arrow and Protobuf
// decoders to map byte spans to actual column names, etc.
// Still, it's a start. May it serves as breadcrumbs for the next soul venturing into this.
//
// ImHex: https://imhex.werwolv.net/

#pragma author Rerun Technologies AB <opensource@rerun.io>
#pragma description Rerun's RRD encoding (.rrd, .rbl)

// Refer to the `re_log_encoding` crate [1] for the ground truth.
//
// [1]: <https://github.com/rerun-io/rerun/tree/main/crates/store/re_log_encoding>

#pragma MIME application/rrd
#pragma magic [ 52 52 46 ?? ] @ 0x00

#pragma endian little

import std.io;
import std.sys;

// --- Definitions ---

// TODO(cmc): maybe the hover experience would be better if I made a u32 or something? (and remove the struct type)
struct Version {
    u8 major;
    u8 minor;
    u8 patch;
    u8 meta;
} [[comment("Binary-encoded semver"), format("Version::fmt")]];

namespace Version {
    fn fmt(Version v) {
        // TODO(cmc): handle the meta part (e.g. `+dev`)
        return std::format("{}.{}.{}", v.major, v.minor, v.patch);
    };
}

enum Compression: u8 {
    Off = 0,
    LZ4 = 1,
};

enum Serializer: u8 {
    Protobuf = 2,
};

struct EncodingOptions {
    Compression compression;
    Serializer serializer;
    u8 _r0 [[hidden, comment("reserved")]];
    u8 _r1 [[hidden, comment("reserved")]];
} [[format("EncodingOptions::fmt")]];

namespace EncodingOptions {
    fn fmt(EncodingOptions eo) {
        return std::format("{} {}", eo.compression, eo.serializer);
    };
}

struct StreamHeader {
    char magic[4] [[comment("RRF0/RRF1/RRF2")]];
    Version version;
    EncodingOptions opts;
} [[single_color, color("E6194B"), format("StreamHeader::fmt"), comment("Custom binary encoding")]];

namespace StreamHeader {
    fn fmt(StreamHeader fh) {
        return std::format("Rerun RRD v{} [{}]", fh.version, fh.opts);
    };
}

enum MessageKind : u64 {
    End = 0,
    SetStoreInfo = 1,
    ArrowMsg = 2,
    BlueprintActivationCommand = 3,
};

struct MessageHeader {
    MessageKind kind;
    u64 len;
} [[single_color, color("3CB44B"), format("MessageHeader::fmt"), comment("Custom binary encoding")]];

namespace MessageHeader {
    fn fmt(MessageHeader mh) {
        return std::format("MessageHeader<{}>", mh.kind);
    };
}

struct Message {
    MessageHeader header;

    match (header.kind) {
        (MessageKind::End): {
            break;
        }

        (MessageKind::SetStoreInfo): {}

        (MessageKind::ArrowMsg): {}

        (MessageKind::BlueprintActivationCommand): {}
    }

    // TODO(cmc): To properly see what's in there would require not only a full Rerun ImHex plugin, but also
    // custom Protobuf and Arrow decoders in the Rerun codebase so that we could map spans of bytes to actual
    // Arrow columns etc.
    u8 data[header.len] [[color("4363D8"), comment("Protobuf encoded data which embeds Arrow IPC encoded blobs")]];
    // $ += header.len;
} [[format("Message::fmt")]];

namespace Message {
    fn fmt(Message m) {
        return std::format("Message<{}>", m.header.kind);
    };
}

// --- Entrypoint ---

StreamHeader stream_header @ 0;

// TODO(cmc): These older ones were MsgPack so we don't actually support them anyhow…
std::assert_warn(
       stream_header.magic == "RRF0"
    || stream_header.magic == "RRF1"
    || stream_header.magic == "RRF2",
    std::format("invalid magic bytes: expected RRF[0|1|2], got {} instead", stream_header.magic)
);

Message messages[while (true)] @ $;

// TODO(cmc): and then here there's the whole mess where you might of might not have an EOS marker, and
// might or might not have a StreamHeader that follows.
// I.e. if parsing fails, we need to try and start again.


// --- Random notes ---

// Not useful to us at the moment, but interesting nonetheless:
import type.magic;
using FourCC = type::Magic<0x52524632>;

// TODO(cmc): pick colors from here:
// F58231
// 911EB4
// 46F0F0
// F032E6
// BCF60C
// FABEBE
// 008080
// E6BEFF
// 9A6324
