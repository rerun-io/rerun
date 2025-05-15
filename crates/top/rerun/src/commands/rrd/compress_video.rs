use std::time::Duration;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use indicatif::ProgressBar;
use itertools::Itertools as _;
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};

use re_entity_db::EntityDb;
use re_sdk::StoreId;

use crate::commands::{read_rrd_streams_from_file_or_stdin, save_entity_dbs_to_rrd};

#[derive(Debug, Clone, clap::Parser)]
pub struct CompressVideo {
    /// Path to rrd files to migrate
    // TODO: allow folders
    path_to_input_rrds: Vec<Utf8PathBuf>,
}

impl CompressVideo {
    pub fn run(&self) -> anyhow::Result<()> {
        let Self {
            mut path_to_input_rrds,
        } = self.clone();

        let num_files_before = path_to_input_rrds.len();

        path_to_input_rrds.retain(|f| !f.to_string().ends_with(".backup.rrd"));

        let num_files = path_to_input_rrds.len();

        if num_files < num_files_before {
            eprintln!(
                "Ignored {} file(s) that are called .backup.rrd, and are therefore assumed to already have been compressed",
                num_files_before - num_files
            );
        }

        // Sanity-check input:
        for path in &path_to_input_rrds {
            anyhow::ensure!(path.exists(), "No such file: {path}");
        }

        eprintln!("Compressing images in {num_files} .rrd file(s) to videos…");

        let progress =
            ProgressBar::new(path_to_input_rrds.len() as u64).with_message("Migrating rrd:s");
        progress.enable_steady_tick(Duration::from_millis(500));

        let failures: Vec<(Utf8PathBuf, anyhow::Error)> = path_to_input_rrds
            .par_iter()
            .filter_map(|original_path| {
                if let Err(err) = video_compress_file_at(original_path) {
                    progress.inc(1);
                    Some((original_path.clone(), err))
                } else {
                    progress.inc(1);
                    None
                }
            })
            .collect();

        progress.finish_and_clear();

        if failures.is_empty() {
            eprintln!(
                // TODO: report how many files were just untouched and remove backups for those.
                "✅ {} file(s) successfully compressed.",
                path_to_input_rrds.len()
            );
            Ok(())
        } else {
            let num_failures = failures.len();
            eprintln!("❌ Failed to compress {num_failures}/{num_files} file(s):");
            eprintln!();
            for (path, err) in &failures {
                eprintln!("  {path}: {}\n", re_error::format(err));
            }
            anyhow::bail!("Failed to compress {num_failures}/{num_files} file(s)");
        }
    }
}

fn video_compress_file_at(original_path: &Utf8PathBuf) -> anyhow::Result<()> {
    // Rename `old_name.rrd` to `old_name.backup.rrd`:
    let backup_path = original_path.with_extension("backup.rrd");

    if backup_path.exists() {
        eprintln!("Ignoring compression of {original_path}: {backup_path} already exists");
        return Ok(());
    }

    // TODO: only do this if there's actually anything to compress.
    std::fs::rename(original_path, &backup_path)
        .with_context(|| format!("Couldn't rename {original_path:?} to {backup_path:?}"))?;

    if let Err(err) = video_compress_from_to(&backup_path, original_path) {
        // Restore:
        std::fs::rename(&backup_path, original_path).ok();
        Err(err)
    } else {
        Ok(())
    }
}

/// Stream-convert an rrd file
fn video_compress_from_to(from_path: &Utf8PathBuf, to_path: &Utf8PathBuf) -> anyhow::Result<()> {
    let (rx, _rrd_in_size) = read_rrd_streams_from_file_or_stdin(&[from_path.clone()]);

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();

    let mut errors = indexmap::IndexSet::new();

    for (_source, res) in rx {
        match res {
            Ok(msg) => {
                if let Err(err) = entity_dbs
                    .entry(msg.store_id().clone())
                    .or_insert_with(|| re_entity_db::EntityDb::new(msg.store_id().clone()))
                    .add(&msg)
                {
                    errors.insert(err.to_string());
                }
            }

            Err(err) => {
                errors.insert(err.to_string());
            }
        }
    }

    for entity_db in entity_dbs.values_mut() {
        video_compress_entity_db(entity_db, &mut errors);
    }

    // TODO: report size difference
    let _rrd_out_size = save_entity_dbs_to_rrd(Some(to_path), entity_dbs)?;

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.iter().join("\n"))
    }
}

fn video_compress_entity_db(entity_db: &EntityDb, errors: &mut indexmap::IndexSet<String>) {
    let storage_engine = entity_db.storage_engine();
    let store = storage_engine.store();

    let blob_descriptor = re_types::archetypes::EncodedImage::descriptor_blob();

    //let mut rewritten_chunks: Vec<re_chunk::Chunk> = Vec::new();

    for entity_path in &store.all_entities() {
        if !store.entity_has_component(entity_path, &blob_descriptor) {
            continue;
        }

        let Some((timeline, _)) = store
            .timelines()
            .into_iter()
            .find(|(_name, timeline)| timeline.typ() != re_log_types::TimeType::Sequence)
        else {
            // TODO: report if there's no time timeline at all.
            continue;
        };

        // TODO: handle multiple timelines proper.

        let chunks = store.range_relevant_chunks(
            &re_chunk::RangeQuery::everything(timeline),
            entity_path,
            &blob_descriptor,
        );

        let num_encoded_images: usize = chunks
            .iter()
            .map(|chunk| {
                chunk
                    .iter_component::<re_types::components::Blob>(&blob_descriptor)
                    .map(|blobs| blobs.iter().count())
                    .count()
            })
            .sum();
        if num_encoded_images < 4 {
            re_log::info!(
                "Found {num_encoded_images} at {entity_path:?}: skipping since it's too few images"
            );
            continue;
        }
        re_log::info!("Found {num_encoded_images} at {entity_path:?}, compressing to to a video…");

        let blob_component_iters: Vec<_> = chunks
            .iter()
            .flat_map(|c| c.iter_component::<re_types::components::Blob>(&blob_descriptor))
            .collect();
        let blobs = blob_component_iters
            .iter()
            .flat_map(|blobs| blobs.iter().map(|b| b.as_ref()));

        if let Err(err) = encode_jpeg_blobs_to_mp4(blobs) {
            errors.insert(format!(
                "Failed to compress images on {entity_path:?}: {err}"
            ));
        } else {
            re_log::info!("Compressed images on {entity_path:?} to a video.");
        }
    }
}

fn encode_jpeg_blobs_to_mp4<'a>(jpeg_blobs: impl Iterator<Item = &'a [u8]>) -> anyhow::Result<()> {
    use gstreamer as gst;
    use gstreamer::prelude::*;

    // Initialize GStreamer
    gst::init()?;

    let pipeline = gst::Pipeline::new();
    let appsrc;

    // Configure pipeline
    // TODO: expose some paraemters
    {
        // Configure appsrc
        let caps = gst::Caps::builder("image/jpeg")
            .field("framerate", gst::Fraction::new(30, 1))
            .build();

        appsrc = gst::ElementFactory::make("appsrc")
            //  .set_property("stream-type", AppStreamType::Stream) // TODO: ????
            //.set_property("caps", &caps) // TODO:????
            .build()?;
        let jpegdec = gst::ElementFactory::make("jpegdec").build()?;
        let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
        let x264enc = gst::ElementFactory::make("x264enc").build()?;
        let mp4mux = gst::ElementFactory::make("mp4mux").build()?;
        let sink = gst::ElementFactory::make("filesink").build()?;

        // TODO: output to binary?
        sink.set_property("location", "test.mp4");

        // Add elements to pipeline
        pipeline.add_many(&[&appsrc, &jpegdec, &videoconvert, &x264enc, &mp4mux, &sink])?;

        // Link elements
        appsrc.link(&jpegdec)?;
        jpegdec.link(&videoconvert)?;
        videoconvert.link(&x264enc)?;
        x264enc.link(&mp4mux)?;
        mp4mux.link(&sink)?;
    }

    // Start playing
    pipeline.set_state(gst::State::Playing)?;

    // Get appsrc pad
    let mut appsrc_pad = appsrc.static_pad("src").unwrap();

    // Send each JPEG blob
    for (i, blob) in jpeg_blobs.enumerate() {
        // TODO: safety yadayada
        // SAFTEY: stuff
        #[allow(unsafe_code)]
        let static_lifetime_blob = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(blob) };
        let mut buffer = gst::Buffer::from_slice(static_lifetime_blob);

        // Set timestamp. TODO: Claude is bullshitting me here again!
        // TODO: ignore this since we're "referencing" into the stream?
        // let timestamp = gst::ClockTime::from_seconds(i as u64) / 30; // 30 fps
        // buffer.set_pts(timestamp);
        // buffer.set_dts(timestamp);
        // buffer.set_duration(gst::ClockTime::from_seconds(1) / 30);

        // Push buffer
        appsrc_pad.push(buffer)?;
    }

    // Send EOS
    if !appsrc_pad.push_event(gst::event::Eos::new()) {
        anyhow::bail!("Failed to send EOS event");
    }

    // TODO: progress bar?

    // Wait until EOS or error
    let bus = pipeline.bus().unwrap(); // TODO? what?
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null)?;
                anyhow::bail!(
                    "Error from {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
            }
            _ => (),
        }
    }

    // Clean up
    pipeline.set_state(gst::State::Null)?;

    Ok(())
}
