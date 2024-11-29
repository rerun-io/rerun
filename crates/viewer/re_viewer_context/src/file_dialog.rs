use crate::ViewerContext;

impl ViewerContext<'_> {
    /// Save some bytes to disk, after first showing a save dialog.
    #[allow(clippy::unused_self)] // Not used on Wasm
    pub fn save_file_dialog(&self, file_name: String, title: String, data: Vec<u8>) {
        re_tracing::profile_function!();

        #[cfg(target_arch = "wasm32")]
        {
            // Web
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = async_save_dialog(&file_name, &title, data).await {
                    re_log::error!("File saving failed: {err}");
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Native
            let path = {
                re_tracing::profile_scope!("file_dialog");
                rfd::FileDialog::new()
                    .set_file_name(file_name)
                    .set_title(title)
                    .save_file()
            };
            if let Some(path) = path {
                use crate::SystemCommandSender as _;
                self.command_sender
                    .send_system(crate::SystemCommand::FileSaver(Box::new(move || {
                        std::fs::write(&path, &data)?;
                        Ok(path)
                    })));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn async_save_dialog(file_name: &str, title: &str, data: Vec<u8>) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let file_handle = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .set_title(title)
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
