//! Implementation of filesystem operations based on [OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system).
//!
//! The API follows [`tokio::fs`](https://docs.rs/tokio/latest/tokio/fs/index.html),
//! so that we can use it as a drop-in replacement.

// TODO(grtlr): Maybe move this to a `re_opfs` crate.

use std::io;
use std::path::{Component, Path};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{DomException, FileSystemDirectoryHandle, FileSystemFileHandle};

pub struct Metadata {
    is_file: bool,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.is_file
    }
}

pub async fn metadata(path: &Path) -> io::Result<Metadata> {
    let path = path.to_owned();
    run_local(async move {
        match open_file(&path).await {
            Ok(file_handle) => {
                let _file: web_sys::File = await_js(file_handle.get_file()).await?;
                Ok(Metadata { is_file: true })
            }
            Err(err) if err.kind() == io::ErrorKind::InvalidInput => {
                Ok(Metadata { is_file: false })
            }
            Err(err) => Err(err),
        }
    })
    .await
}

// TODO(RR-5154): Replace this with something akin to `read_exact_at`, to avoid
// copying all of the bytes via `to_vec`.
pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
    let path = path.to_owned();
    run_local(async move {
        let file_handle = open_file(&path).await?;
        let file: web_sys::File = await_js(file_handle.get_file()).await?;
        let blob: &web_sys::Blob = file.as_ref();
        let buffer: js_sys::ArrayBuffer = await_js(blob.array_buffer()).await?;

        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    })
    .await
}

async fn open_file(path: &Path) -> io::Result<FileSystemFileHandle> {
    let components = opfs_components(path)?;
    let Some((file_name, directory_names)) = components.split_last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OPFS path must contain a file component",
        ));
    };

    let navigator = web_sys::window()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::Unsupported, "OPFS requires a browser Window")
        })?
        .navigator();
    let mut directory: FileSystemDirectoryHandle =
        await_js(navigator.storage().get_directory()).await?;

    for directory_name in directory_names {
        directory = await_js(directory.get_directory_handle(directory_name)).await?;
    }

    await_js(directory.get_file_handle(file_name)).await
}

fn opfs_components(path: &Path) -> io::Result<Vec<String>> {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(component) => components.push(
                component
                    .to_str()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "OPFS path is not UTF-8")
                    })?
                    .to_owned(),
            ),
            Component::ParentDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "OPFS paths must not contain parent-directory or prefix components",
                ));
            }
        }
    }

    Ok(components)
}

/// Converts a [`js_sys::Promise`] to a Rust `future`.
async fn await_js<T>(promise: js_sys::Promise) -> io::Result<T>
where
    T: JsCast,
{
    JsFuture::from(promise)
        .await
        .map_err(js_to_io_error)?
        .dyn_into()
        .map_err(js_to_io_error)
}

/// `tonic` service futures are `Send`, while `JsFuture` is not.
/// Run browser API work on the local Wasm executor and await only the `Send` oneshot receiver.
fn run_local<T>(
    future: impl std::future::Future<Output = io::Result<T>> + 'static,
) -> impl std::future::Future<Output = io::Result<T>> + Send
where
    T: Send + 'static,
{
    let (tx, rx) = futures::channel::oneshot::channel();

    wasm_bindgen_futures::spawn_local(async move {
        let result = future.await;
        tx.send(result).ok();
    });

    async move {
        rx.await.map_err(|_err| {
            io::Error::new(
                io::ErrorKind::Interrupted,
                "OPFS browser task was canceled before completion",
            )
        })?
    }
}

fn js_to_io_error(value: JsValue) -> io::Error {
    if let Some(exception) = value.dyn_ref::<DomException>() {
        return err_from_dom_exception(exception);
    }

    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        return err_from_js(error);
    }

    io::Error::other(value.as_string().unwrap_or_else(|| format!("{value:?}")))
}

fn err_from_dom_exception(exception: &DomException) -> io::Error {
    let kind = match exception.code() {
        DomException::NOT_FOUND_ERR => io::ErrorKind::NotFound,
        DomException::SECURITY_ERR => io::ErrorKind::PermissionDenied,
        DomException::TYPE_MISMATCH_ERR => io::ErrorKind::InvalidInput,
        _ => io::ErrorKind::Other,
    };

    io::Error::new(kind, exception.message())
}

fn err_from_js(error: &js_sys::Error) -> io::Error {
    let name = String::from(error.name());
    let raw_message = String::from(error.message());
    let message = if raw_message.is_empty() {
        name
    } else {
        format!("{name}: {raw_message}")
    };

    io::Error::other(message)
}
