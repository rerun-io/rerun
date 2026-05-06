use crate::CommandSender;

fn is_safe_filename_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')
}

/// Replace "dangerous" characters by a safe one.
pub fn sanitize_file_name(file_name: &str) -> String {
    file_name.replace(|c: char| !is_safe_filename_char(c), "-")
}

impl CommandSender {
    /// Save some bytes to disk, after first showing a save dialog.
    ///
    /// [This may only be called on the main thread](https://docs.rs/rfd/latest/rfd/#macos-non-windowed-applications-async-and-threading).
    #[allow(clippy::allow_attributes, clippy::unused_self)] // Not used on Wasm
    pub fn save_files_dialog(
        &self,
        _: re_capabilities::MainThreadToken,
        title: &str,
        files: Vec<(String, Vec<u8>)>, // (file_name, data)
    ) {
        re_tracing::profile_function!();

        if files.is_empty() {
            return;
        }

        // Web
        #[cfg(target_arch = "wasm32")]
        {
            let title = title.to_owned();
            let len = files.len();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = async_save_files_dialog_wasm(title, files).await {
                    re_log::error!("File saving failed: {err}");
                } else {
                    re_log::info!("{} saved.", re_format::format_plural_s(len, "file"));
                }
            });
        }

        // Native
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = {
                re_tracing::profile_scope!("file_dialog");
                rfd::FileDialog::new().set_title(title).pick_folder()
            };

            if let Some(dir) = dir {
                use crate::SystemCommandSender as _;

                let files = files
                    .into_iter()
                    .map(|(name, data)| (dir.join(sanitize_file_name(&name)), data))
                    .collect::<Vec<_>>();

                self.send_system(crate::SystemCommand::FileSaver(Box::new(move || {
                    let mut last_path = None;
                    for (path, data) in files {
                        std::fs::write(&path, &data)?;
                        last_path = Some(path);
                    }
                    last_path.ok_or_else(|| anyhow::anyhow!("No files to save"))
                })));
            }
        }
    }
}

/// Save multiple files to disk, after first showing a save dialog in the browser.
///
/// Browser can save single file only, so we zip multiple files into a single zip file.
#[cfg(target_arch = "wasm32")]
pub async fn async_save_files_dialog_wasm(
    title: String,
    mut files: Vec<(String, Vec<u8>)>,
) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use std::io::Write as _;

    if files.is_empty() {
        return Ok(());
    }

    // save single file as is
    let (file_name, data) = if files.len() == 1 {
        std::mem::take(&mut files[0])
    }
    // zip multiple files into a single zip file
    else {
        let file_name = "rerun.zip".to_owned();
        let mut data = Vec::new();

        let cursor = std::io::Cursor::new(&mut data);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (file_name, data) in files {
            let file_name = sanitize_file_name(&file_name);
            zip.start_file(file_name, options)
                .context("Failed to add file to zip")?;
            zip.write_all(&data).context("Failed to write file data")?;
        }
        zip.finish().context("Failed to finalize zip")?;

        (file_name, data)
    };

    let file_handle = rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(file_name)
        .save_file()
        .await;

    let Some(file_handle) = file_handle else {
        return Ok(()); // aborted
    };

    file_handle
        .write(data.as_slice())
        .await
        .context("Failed to save")
}
