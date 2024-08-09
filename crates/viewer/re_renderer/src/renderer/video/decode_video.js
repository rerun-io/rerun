// https://w3c.github.io/webcodecs/samples/video-decode-display/

// Wraps an MP4Box File as a WritableStream underlying sink.
class MP4FileSink {
  #file = null;
  #offset = 0;

  constructor(file) {
    this.#file = file;
  }

  write(chunk) {
    // MP4Box.js requires buffers to be ArrayBuffers, but we have a Uint8Array.
    const buffer = chunk.buffer;

    // Inform MP4Box where in the file this chunk is from.
    buffer.fileStart = this.#offset;
    this.#offset += buffer.byteLength;

    // Append chunk.
    this.#file.appendBuffer(buffer);
  }

  close() {
    this.#file.flush();
  }
}

// Demuxes the first video track of an MP4 file using MP4Box, calling
// `onConfig()` and `onChunk()` with appropriate WebCodecs objects.
class MP4Demuxer {
  #onConfig = null;
  #onChunk = null;
  #file = null;

  constructor(uri, { onConfig, onChunk, onError, onClose }) {
    this.#onConfig = onConfig;
    this.#onChunk = onChunk;

    // Configure an MP4Box File for demuxing.
    this.#file = __rerun_mp4box.createFile();
    this.#file.onError = (error) => onError(error);
    this.#file.onReady = this.#onReady.bind(this);
    this.#file.onSamples = this.#onSamples.bind(this);

    // Fetch the file and pipe the data through.
    const fileSink = new MP4FileSink(this.#file, onClose);
    fetch(uri).then(async (response) => {
      // highWaterMark should be large enough for smooth streaming, but lower is
      // better for memory usage.
      await response.body.pipeTo(
        new WritableStream(fileSink, { highWaterMark: 2 }),
      );

      await onClose();
    });
  }

  // Get the appropriate `description` for a specific track. Assumes that the
  // track is H.264, H.265, VP8, VP9, or AV1.
  #description(track) {
    const trak = this.#file.getTrackById(track.id);
    for (const entry of trak.mdia.minf.stbl.stsd.entries) {
      const box = entry.avcC || entry.hvcC || entry.vpcC || entry.av1C;
      if (box) {
        const stream = new DataStream(undefined, 0, DataStream.BIG_ENDIAN);
        box.write(stream);
        return new Uint8Array(stream.buffer, 8); // Remove the box header.
      }
    }
    throw new Error("avcC, hvcC, vpcC, or av1C box not found");
  }

  #onReady(info) {
    const track = info.videoTracks[0];

    // Generate and emit an appropriate VideoDecoderConfig.
    this.#onConfig({
      // Browser doesn't support parsing full vp8 codec (eg: `vp08.00.41.08`),
      // they only support `vp8`.
      codec: track.codec.startsWith("vp08") ? "vp8" : track.codec,
      codedHeight: track.video.height,
      codedWidth: track.video.width,
      description: this.#description(track),
    });

    // Start demuxing.
    this.#file.setExtractionOptions(track.id);
    this.#file.start();
  }

  #onSamples(track_id, ref, samples) {
    // Generate and emit an EncodedVideoChunk for each demuxed sample.
    for (const sample of samples) {
      this.#onChunk(
        new EncodedVideoChunk({
          type: sample.is_sync ? "key" : "delta",
          timestamp: (1e6 * sample.cts) / sample.timescale,
          duration: (1e6 * sample.duration) / sample.timescale,
          data: sample.data,
        }),
      );
    }
  }
}

function decodeVideo(url) {
  return new Promise((resolve, reject) => {
    let config = { duration: 0 };
    let frames = [];

    let decoder = new VideoDecoder({
      output(frame) {
        frames.push(frame);

        const end = frame.timestamp + (frame.duration ?? 0);
        if (end >= config.duration) {
          config.duration = end;
        }
        config.format = frame.format;
      },
      error(error) {
        reject(error);
      },
    });

    // Fetch and demux the media data.
    let demuxer = new MP4Demuxer(url, {
      onConfig(v) {
        config.codec = v.codec;
        config.height = v.codedHeight;
        config.width = v.codedWidth;
        decoder.configure(v);
      },
      onChunk(chunk) {
        decoder.decode(chunk);
      },
      onError(error) {
        reject(error);
      },
      async onClose() {
        await decoder.flush();
        resolve({ config, frames });
      },
    });
  });
}

window.__rerun_decode_video = decodeVideo;
