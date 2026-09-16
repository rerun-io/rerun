use crate::decode::sync_decoder_wrapper::SyncDecoder;

pub struct RvlDecoder;

impl SyncDecoder for RvlDecoder {
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: super::Chunk,
        output_sender: &re_quota_channel::Sender<super::FrameResult>,
    ) {
        if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        let meta = match re_rvl::RosRvlMetadata::parse(&chunk.data) {
            Ok(meta) => meta,
            Err(err) => {
                let _send_error = output_sender.send(Err(super::DecodeError::RvlDecoder(err)));
                return;
            }
        };

        let data = match re_rvl::decode_rvl_with_quantization(&chunk.data, &meta) {
            Ok(data) => data,
            Err(err) => {
                let _send_error = output_sender.send(Err(super::DecodeError::RvlDecoder(err)));
                return;
            }
        };

        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_ne_bytes()).collect();

        let decoded = super::DecodedFrameContent {
            data: bytes,
            width: meta.width,
            height: meta.height,
            format: super::PixelFormat::R32Float,
        };

        let content = cfg_select! {
            target_arch = "wasm32" => super::FrameContent::Decoded(decoded),
            _ => decoded,
        };

        let _send_error = output_sender.send(Ok(super::Frame {
            content,
            info: super::FrameInfo {
                // Every image decodes on its own.
                is_sync: Some(true),
                ..chunk.frame_info()
            },
        }));
    }

    fn reset(&mut self, _video_data_description: &crate::VideoDataDescription) {}
}
