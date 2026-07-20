//! Implementation of filesystem operations based on [OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system).
//!
//! The signatures loosely mirror [`tokio::fs`](https://docs.rs/tokio/latest/tokio/fs/index.html)
//! for familiarity.

// TODO(grtlr): Maybe move this to a `re_opfs` crate.

use std::io;
use std::path::{Component, Path};
use std::sync::Arc;

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    DomException, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemWritableFileStream,
};

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

/// Write `contents` to `path`, creating any missing parent directories.
///
/// Takes `contents` by value so callers that already own the bytes avoid a copy; the whole
/// buffer would otherwise be duplicated on the Wasm heap for large uploads.
pub async fn write(path: impl AsRef<Path>, contents: Arc<[u8]>) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    run_local(async move {
        let file_handle = create_file(&path).await?;
        let writer: FileSystemWritableFileStream = await_js(file_handle.create_writable()).await?;

        if let Err(err) = write_all(&writer, &contents).await {
            // Discard the partially-written file so a later read fails cleanly rather than
            // returning truncated contents (e.g. when the quota is exceeded mid-write).
            let writable_stream: &web_sys::WritableStream = writer.as_ref();
            JsFuture::from(writable_stream.abort()).await.ok();
            return Err(err);
        }

        Ok(())
    })
    .await
}

async fn write_all(writer: &FileSystemWritableFileStream, contents: &[u8]) -> io::Result<()> {
    let _: JsValue = await_js(
        writer
            .write_with_u8_array(contents)
            .map_err(|err| js_to_io_error(&err))?,
    )
    .await?;

    let writable_stream: &web_sys::WritableStream = writer.as_ref();
    let _: JsValue = await_js(writable_stream.close()).await?;
    Ok(())
}

/// Recursively remove the directory at `path` and everything under it.
///
/// A missing `path` is treated as success, so this is an idempotent "clear".
pub async fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    run_local(async move {
        let (directory, name) = match parent_directory_and_file_name(&path, false).await {
            Ok(directory_and_name) => directory_and_name,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };

        let options = web_sys::FileSystemRemoveOptions::new();
        options.set_recursive(true);

        match await_js::<JsValue>(directory.remove_entry_with_options(&name, &options)).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    })
    .await
}

async fn open_file(path: &Path) -> io::Result<FileSystemFileHandle> {
    let (directory, file_name) = parent_directory_and_file_name(path, false).await?;
    await_js(directory.get_file_handle(&file_name)).await
}

async fn create_file(path: &Path) -> io::Result<FileSystemFileHandle> {
    let (directory, file_name) = parent_directory_and_file_name(path, true).await?;
    let options = web_sys::FileSystemGetFileOptions::new();
    options.set_create(true);
    await_js(directory.get_file_handle_with_options(&file_name, &options)).await
}

/// The OPFS root directory handle.
async fn opfs_root() -> io::Result<FileSystemDirectoryHandle> {
    let navigator = web_sys::window()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::Unsupported, "OPFS requires a browser Window")
        })?
        .navigator();
    await_js(navigator.storage().get_directory()).await
}

/// Resolve the parent directory of `path`, walking (and, when `create`, creating) each component.
async fn parent_directory_and_file_name(
    path: &Path,
    create: bool,
) -> io::Result<(FileSystemDirectoryHandle, String)> {
    let components = opfs_components(path)?;
    let Some((file_name, directory_names)) = components.split_last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OPFS path must contain a file component",
        ));
    };

    let mut directory = opfs_root().await?;
    for directory_name in directory_names {
        directory = if create {
            let options = web_sys::FileSystemGetDirectoryOptions::new();
            options.set_create(true);
            await_js(directory.get_directory_handle_with_options(directory_name, &options)).await?
        } else {
            await_js(directory.get_directory_handle(directory_name)).await?
        };
    }

    Ok((directory, file_name.clone()))
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
        .map_err(|err| js_to_io_error(&err))?
        .dyn_into()
        .map_err(|err| js_to_io_error(&err))
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

fn js_to_io_error(value: &JsValue) -> io::Error {
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

#[cfg(test)]
mod test {
    use super::*;

    use std::io;

    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    fn unique_opfs_test_dir() -> String {
        format!("opfs-test-{}", re_tuid::Tuid::new())
    }

    #[wasm_bindgen_test]
    async fn write_read_metadata_and_overwrite_nested_file() {
        let test_dir = unique_opfs_test_dir();
        let file_path = format!("/{test_dir}/./nested/file.bin");

        write(&file_path, Vec::from(b"first write").into())
            .await
            .expect("initial write should succeed");

        let metadata = metadata(file_path.as_ref())
            .await
            .expect("metadata should succeed for an OPFS file");
        assert!(metadata.is_file());
        assert_eq!(
            read(file_path.as_ref())
                .await
                .expect("read should return the bytes that were written"),
            b"first write",
        );

        write(&file_path, Vec::from(b"second").into())
            .await
            .expect("overwriting an OPFS file should succeed");
        assert_eq!(
            read(file_path.as_ref())
                .await
                .expect("read should return the overwritten bytes"),
            b"second",
        );

        remove_dir_all(test_dir)
            .await
            .expect("test cleanup should remove the OPFS directory");
    }

    #[wasm_bindgen_test]
    async fn remove_dir_all_is_recursive_and_idempotent() {
        let test_dir = unique_opfs_test_dir();
        let first_file = format!("{test_dir}/a.bin");
        let second_file = format!("{test_dir}/nested/b.bin");

        write(&first_file, Vec::from(b"a").into())
            .await
            .expect("writing first OPFS file should succeed");
        write(&second_file, Vec::from(b"b").into())
            .await
            .expect("writing nested OPFS file should succeed");

        remove_dir_all(&test_dir)
            .await
            .expect("recursive remove should succeed");
        remove_dir_all(&test_dir)
            .await
            .expect("removing a missing OPFS directory should be a no-op");
        remove_dir_all(format!("{test_dir}/nested"))
            .await
            .expect("removing below a missing OPFS directory should be a no-op");

        let err = read(first_file.as_ref())
            .await
            .expect_err("removed OPFS file should not be readable");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);

        let err = read(second_file.as_ref())
            .await
            .expect_err("recursively removed OPFS file should not be readable");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[wasm_bindgen_test]
    async fn rejects_parent_directory_paths() {
        let err = write("opfs-test/../escape.bin", Vec::from(b"x").into())
            .await
            .expect_err("OPFS paths must not allow parent-directory traversal");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

        let err = read("../escape.bin".as_ref())
            .await
            .expect_err("OPFS paths must not allow parent-directory traversal");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

        let err = remove_dir_all("../escape")
            .await
            .expect_err("OPFS paths must not allow parent-directory traversal");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
