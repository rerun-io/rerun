use re_log_types::DataSourceMessage;
use re_smart_channel::Sender;

/// Asynchronously loads RRD data streaming in from standard input.
///
/// This fails synchronously iff the standard input stream could not be opened, otherwise errors
/// are handled asynchronously (as in: they're logged).
pub fn load_stdin(tx: Sender<DataSourceMessage>) -> anyhow::Result<()> {
    let stdin = std::io::BufReader::new(std::io::stdin());

    let wait_for_eos = true;
    let decoder = re_log_encoding::decoder::stream::StreamDecoderApp::decode_eager_with_opts(
        stdin,
        wait_for_eos,
    )?;

    rayon::spawn(move || {
        re_tracing::profile_scope!("stdin");

        for msg in decoder {
            let msg = match msg {
                Ok(msg) => msg,
                Err(err) => {
                    re_log::warn_once!("Failed to decode message in stdin: {err}");
                    continue;
                }
            };
            if tx.send(msg.into()).is_err() {
                break; // The other end has decided to hang up, not our problem.
            }
        }

        tx.quit(None).ok(); // The other end has decided to hang up, not our problem.
    });

    Ok(())
}
