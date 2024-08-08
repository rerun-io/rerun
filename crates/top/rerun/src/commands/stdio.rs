use std::path::PathBuf;

use anyhow::Context;
use crossbeam::channel;
use itertools::Itertools as _;

use re_chunk::external::crossbeam;
use re_log_types::LogMsg;

// ---

/// Asynchronously decodes potentially multiplexed RRD streams from the given `paths`, or standard
/// input if none are specified.
///
/// The returned channel contains both the successfully decoded data, if any, as well as any
/// errors faced during processing.
///
/// This function is best-effort: it will try to make progress even in the face of errors.
/// It is up to the user to decide whether and when to stop.
///
/// This function is capable of decoding multiple independent recordings from a single stream.
pub fn read_rrd_streams_from_file_or_stdin(
    version_policy: re_log_encoding::decoder::VersionPolicy,
    paths: &[String],
) -> channel::Receiver<anyhow::Result<LogMsg>> {
    let path_to_input_rrds = paths.iter().map(PathBuf::from).collect_vec();

    // TODO(cmc): might want to make this configurable at some point.
    let (tx, rx) = crossbeam::channel::bounded(100);

    _ = std::thread::Builder::new()
        .name("rerun-cli-stdin".to_owned())
        .spawn(move || {
            if path_to_input_rrds.is_empty() {
                // stdin

                let stdin = std::io::BufReader::new(std::io::stdin().lock());

                let decoder = match re_log_encoding::decoder::Decoder::new_concatenated(
                    version_policy,
                    stdin,
                )
                .context("couldn't decode stdin stream -- skipping")
                {
                    Ok(decoder) => decoder,
                    Err(err) => {
                        tx.send(Err(err)).ok();
                        return;
                    }
                };

                for res in decoder {
                    let res = res.context("couldn't decode message from stdin -- skipping");
                    tx.send(res).ok();
                }
            } else {
                // file(s)

                for rrd_path in path_to_input_rrds {
                    let rrd_file = match std::fs::File::open(&rrd_path)
                        .with_context(|| format!("couldn't open {rrd_path:?} -- skipping"))
                    {
                        Ok(file) => file,
                        Err(err) => {
                            tx.send(Err(err)).ok();
                            continue;
                        }
                    };

                    let decoder =
                        match re_log_encoding::decoder::Decoder::new(version_policy, rrd_file)
                            .with_context(|| format!("couldn't decode {rrd_path:?} -- skipping"))
                        {
                            Ok(decoder) => decoder,
                            Err(err) => {
                                tx.send(Err(err)).ok();
                                continue;
                            }
                        };

                    for res in decoder {
                        let res = res.context("decode rrd message").with_context(|| {
                            format!("couldn't decode message {rrd_path:?} -- skipping")
                        });
                        tx.send(res).ok();
                    }
                }
            }
        });

    rx
}
