use std::path::PathBuf;

use anyhow::Context as _;
use crossbeam::channel;
use itertools::Itertools as _;

use re_chunk::external::crossbeam;

// ---

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InputSource {
    Stdin,
    File(PathBuf),
}

impl std::fmt::Display for InputSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdin => write!(f, "stdin"),
            Self::File(path) => write!(f, "{path:?}"),
        }
    }
}

/// Asynchronously decodes potentially multiplexed RRD streams from the given `paths`, or standard
/// input if none are specified.
///
/// This function returns 2 channels:
/// * The first channel contains both the successfully decoded data, if any, as well as any
///   errors faced during processing.
/// * The second channel, which will fire only once, after all processing is done, indicates the
///   total number of bytes processed.
///
/// This function is best-effort: it will try to make progress even in the face of errors.
/// It is up to the user to decide whether and when to stop.
///
/// This function is capable of decoding multiple independent recordings from a single stream.
pub fn read_rrd_streams_from_file_or_stdin(
    paths: &[String],
) -> (
    channel::Receiver<(InputSource, anyhow::Result<re_log_types::LogMsg>)>,
    channel::Receiver<u64>,
) {
    read_any_rrd_streams_from_file_or_stdin::<re_log_types::LogMsg>(paths)
}

/// Asynchronously decodes potentially multiplexed RRD streams from the given `paths`, or standard
/// input if none are specified.
///
/// This function returns 2 channels:
/// * The first channel contains both the successfully decoded data, if any, as well as any
///   errors faced during processing.
/// * The second channel, which will fire only once, after all processing is done, indicates the
///   total number of bytes processed.
///
/// This function is best-effort: it will try to make progress even in the face of errors.
/// It is up to the user to decide whether and when to stop.
///
/// This function is capable of decoding multiple independent recordings from a single stream.
pub fn read_raw_rrd_streams_from_file_or_stdin(
    paths: &[String],
) -> (
    channel::Receiver<(
        InputSource,
        anyhow::Result<re_protos::log_msg::v1alpha1::log_msg::Msg>,
    )>,
    channel::Receiver<u64>,
) {
    read_any_rrd_streams_from_file_or_stdin::<re_protos::log_msg::v1alpha1::log_msg::Msg>(paths)
}

fn read_any_rrd_streams_from_file_or_stdin<
    T: re_log_encoding::decoder::stream::FileEncoded + Send + 'static,
>(
    paths: &[String],
) -> (
    channel::Receiver<(InputSource, anyhow::Result<T>)>,
    channel::Receiver<u64>,
) {
    let path_to_input_rrds = paths
        .iter()
        .filter(|s| !s.is_empty()) // Avoid a problem with `pixi run check-backwards-compatibility`
        .map(PathBuf::from)
        .collect_vec();

    // TODO(cmc): might want to make this configurable at some point.
    let (tx, rx) = crossbeam::channel::bounded(100);
    let (tx_size_bytes, rx_size_bytes) = crossbeam::channel::bounded(1);

    _ = std::thread::Builder::new()
        .name("rerun-rrd-in".to_owned())
        .spawn(move || {
            let mut size_bytes = 0;

            if path_to_input_rrds.is_empty() {
                // stdin

                let stdin = std::io::BufReader::new(std::io::stdin().lock());
                let wait_for_eos = true;
                let mut decoder =
                    re_log_encoding::decoder::stream::StreamDecoder::decode_lazy_with_opts(
                        stdin,
                        wait_for_eos,
                    );

                for res in &mut decoder {
                    let res = res.context("couldn't decode message from stdin -- skipping");
                    tx.send((InputSource::Stdin, res)).ok();
                }

                size_bytes += decoder.num_bytes_processed();
            } else {
                // file(s)

                for rrd_path in path_to_input_rrds {
                    let rrd_file = match std::fs::File::open(&rrd_path)
                        .with_context(|| format!("couldn't open {rrd_path:?} -- skipping"))
                    {
                        Ok(file) => file,
                        Err(err) => {
                            tx.send((InputSource::File(rrd_path.clone()), Err(err)))
                                .ok();
                            continue;
                        }
                    };

                    let rrd_file = std::io::BufReader::new(rrd_file);
                    let mut messages =
                        re_log_encoding::decoder::stream::StreamDecoder::decode_lazy(rrd_file);

                    for res in &mut messages {
                        let res = res.context("decode rrd message").with_context(|| {
                            format!("couldn't decode message {rrd_path:?} -- skipping")
                        });
                        tx.send((InputSource::File(rrd_path.clone()), res)).ok();
                    }

                    size_bytes += messages.num_bytes_processed();
                }
            }

            tx_size_bytes.send(size_bytes).ok();
        });

    (rx, rx_size_bytes)
}
