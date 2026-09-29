use std::sync::{Arc, OnceLock};

use nohash_hasher::IntMap;
use re_audio::{AudioBuffer, AudioDecodeError, WaveformEnvelope};
use re_byte_size::SizeBytes as _;
use re_chunk::RowId;
use re_chunk_store::ChunkStoreEvent;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::MediaType;
use re_viewer_context::{Cache, StoredBlobCacheKey, filter_blob_removed_events};
use web_time::{Duration, Instant};

/// Resolution of the cached waveform overview, in buckets over the whole clip.
const ENVELOPE_BUCKETS: usize = 4096;

/// How long an unused asset stays decoded.
///
/// Long enough that switching tabs or collapsing a container and coming back does not
/// re-decode the whole file, which takes seconds for a long recording.
/// Under memory pressure, [`Cache::purge_memory`] drops unused entries right away.
const KEEP_UNUSED_FOR: Duration = Duration::from_secs(30);

/// A fully decoded audio asset, plus a coarse waveform envelope for drawing.
#[derive(re_byte_size::SizeBytes)]
pub struct DecodedAudio {
    pub buffer: Arc<AudioBuffer>,
    pub envelope: WaveformEnvelope,
}

/// Where a cached asset is in its life cycle.
#[derive(Clone)]
pub enum AudioLoadState {
    /// Decoding on a background thread. Poll again next frame.
    Loading,

    Ready(Arc<DecodedAudio>),

    Failed(AudioDecodeError),
}

type DecodeResult = Result<Arc<DecodedAudio>, AudioDecodeError>;

struct Entry {
    last_used: Instant,

    /// Set exactly once, by the decoding thread. Failures are kept so they are not retried every frame.
    result: Arc<OnceLock<DecodeResult>>,

    debug_name: String,
}

impl Entry {
    fn state(&self) -> AudioLoadState {
        match self.result.get() {
            None => AudioLoadState::Loading,
            Some(Ok(decoded)) => AudioLoadState::Ready(decoded.clone()),
            Some(Err(err)) => AudioLoadState::Failed(err.clone()),
        }
    }
}

impl re_byte_size::SizeBytes for Entry {
    fn heap_size_bytes(&self) -> u64 {
        let decoded = match self.result.get() {
            Some(Ok(decoded)) => decoded.heap_size_bytes(),
            Some(Err(_)) | None => 0,
        };
        decoded + self.debug_name.heap_size_bytes()
    }
}

/// Caches decoded audio assets by the row id of their blob and the media type used to decode it.
///
/// The media type is part of the key because it steers container probing, and a blob can be
/// paired with a new media type later without the blob row changing.
///
/// Decoding happens on a background thread on native, so the first lookups return
/// [`AudioLoadState::Loading`].
#[derive(Default)]
pub struct AudioAssetCache {
    entries: IntMap<StoredBlobCacheKey, IntMap<Hash64, Entry>>,

    /// When [`Cache::begin_frame`] was last called.
    frame_start: Option<Instant>,
}

impl AudioAssetCache {
    pub fn entry(
        &mut self,
        debug_name: String,
        blob_row_id: RowId,
        blob_component: ComponentIdentifier,
        blob: &re_sdk_types::encodings::Blob,
        media_type: Option<&MediaType>,
    ) -> AudioLoadState {
        re_tracing::profile_function!(&debug_name);

        let blob_cache_key = StoredBlobCacheKey::new(blob_row_id, blob_component);
        let media_type_key = Hash64::hash(media_type);

        let now = Instant::now();
        let entry = self
            .entries
            .entry(blob_cache_key)
            .or_default()
            .entry(media_type_key)
            .or_insert_with(|| {
                let result = Arc::new(OnceLock::new());
                start_decoding(
                    debug_name.clone(),
                    blob.0.clone(),
                    media_type.map(|mt| mt.as_str().to_owned()),
                    &result,
                );
                Entry {
                    last_used: now,
                    result,
                    debug_name,
                }
            });

        entry.last_used = now;
        entry.state()
    }
}

fn decode(bytes: &[u8], media_type: Option<&str>) -> DecodeResult {
    // The player only outputs stereo, so fold surround sources down once here.
    let buffer = re_audio::decode(bytes, media_type)?.downmix_to_stereo();
    let envelope = buffer.waveform_envelope(ENVELOPE_BUCKETS);
    Ok(Arc::new(DecodedAudio {
        buffer: Arc::new(buffer),
        envelope,
    }))
}

fn start_decoding(
    debug_name: String,
    bytes: arrow::buffer::ScalarBuffer<u8>,
    media_type: Option<String>,
    result: &Arc<OnceLock<DecodeResult>>,
) {
    let run = {
        let result = result.clone();
        move || {
            re_tracing::profile_scope!("decode_audio", &debug_name);
            result.set(decode(&bytes, media_type.as_deref())).ok();
        }
    };

    cfg_select! {
        target_arch = "wasm32" => {
            // There is no worker infrastructure for this yet, so the whole file is decoded on
            // the main thread. Long recordings stall the UI once, on first use.
            run();
            drop(debug_name); // Only used by the profiler, which is compiled out on web.
        }
        _ => {
            if let Err(err) = std::thread::Builder::new()
                .name("audio_decoder".to_owned())
                .spawn(run)
            {
                // Record the failure, or the entry would report `Loading` forever.
                result
                    .set(Err(AudioDecodeError::Decode(format!(
                        "Failed to spawn audio decoding thread: {err}"
                    ))))
                    .ok();
            }
        }
    }
}

impl Cache for AudioAssetCache {
    fn name(&self) -> &'static str {
        "AudioAssetCache"
    }

    fn begin_frame(&mut self) {
        re_tracing::profile_function!();

        let now = Instant::now();
        self.frame_start = Some(now);
        self.entries.retain(|_, per_media_type| {
            per_media_type.retain(|_, entry| now.duration_since(entry.last_used) < KEEP_UNUSED_FOR);
            !per_media_type.is_empty()
        });
    }

    fn purge_memory(&mut self) {
        re_tracing::profile_function!();

        // Keep what was used this frame: dropping it would only cause an immediate, slow re-decode.
        let Some(frame_start) = self.frame_start else {
            return;
        };
        self.entries.retain(|_, per_media_type| {
            per_media_type.retain(|_, entry| frame_start <= entry.last_used);
            !per_media_type.is_empty()
        });
    }

    fn on_store_events(&mut self, events: &[&ChunkStoreEvent], _entity_db: &EntityDb) {
        re_tracing::profile_function!();

        let cache_key_removed = filter_blob_removed_events(events);
        self.entries
            .retain(|cache_key, _| !cache_key_removed.contains(cache_key));
    }
}

impl re_byte_size::MemUsageTreeCapture for AudioAssetCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let mut node = re_byte_size::MemUsageNode::new();

        let mut items: Vec<_> = self
            .entries
            .values()
            .flat_map(|per_media_type| per_media_type.values())
            .map(|entry| (entry.debug_name.as_str(), entry.heap_size_bytes()))
            .collect();
        items.sort_by(|a, b| a.0.cmp(b.0));

        for (debug_name, size) in items {
            node.add(debug_name, re_byte_size::MemUsageTree::Bytes(size));
        }

        node.with_total_size_bytes(self.entries.total_size_bytes())
    }
}
