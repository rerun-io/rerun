//! Implementation of filesystem operations based on [OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system).
//!
//! The signatures loosely mirror [`tokio::fs`](https://docs.rs/tokio/latest/tokio/fs/index.html)
//! for familiarity.

use std::io;
use std::path::{Component, Path};
use std::sync::Arc;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    DomException, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemWritableFileStream,
};

pub struct Metadata {
    is_file: bool,
    len: u64,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.is_file
    }

    #[expect(clippy::len_without_is_empty, reason = "mirrors std::fs::Metadata")]
    pub fn len(&self) -> u64 {
        self.len
    }
}

pub async fn metadata(path: &Path) -> io::Result<Metadata> {
    let path = path.to_owned();
    re_async::spawn_local_with_result(async move {
        match open_file(&path).await {
            Ok(file_handle) => {
                let file: web_sys::File = await_js(file_handle.get_file()).await?;
                let blob: &web_sys::Blob = file.as_ref();
                Ok(Metadata {
                    is_file: true,
                    len: blob.size() as u64,
                })
            }
            Err(err) if err.kind() == io::ErrorKind::InvalidInput => Ok(Metadata {
                is_file: false,
                len: 0,
            }),
            Err(err) => Err(err),
        }
    })
    .await
    .map_err(io::Error::other)?
}

/// Read the entire file into Wasm linear memory.
pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
    let path = path.to_owned();
    re_async::spawn_local_with_result(async move {
        let file_handle = open_file(&path).await?;
        let file: web_sys::File = await_js(file_handle.get_file()).await?;
        read_blob(file.as_ref()).await
    })
    .await
    .map_err(io::Error::other)?
}

/// Read an entire browser file into Wasm linear memory.
pub async fn read_file(file: web_sys::File) -> io::Result<Vec<u8>> {
    re_async::spawn_local_with_result(async move { read_blob(file.as_ref()).await })
        .await
        .map_err(io::Error::other)?
}

async fn read_blob(blob: &web_sys::Blob) -> io::Result<Vec<u8>> {
    if blob.size() > f64::from(u32::MAX) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot read files larger than u32::MAX bytes into Wasm memory",
        ));
    }
    let buffer: js_sys::ArrayBuffer = await_js(blob.array_buffer()).await?;

    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// A positional-read handle to an OPFS file.
///
/// Owns the browser [`web_sys::File`] snapshot returned when opened. Reads slice the backing
/// [`web_sys::Blob`], so only the requested range crosses the JS/Wasm boundary — unlike [`read`],
/// which copies the whole file.
#[derive(Debug)]
pub struct File {
    file: web_sys::File,
}

impl From<web_sys::File> for File {
    fn from(file: web_sys::File) -> Self {
        Self { file }
    }
}

impl File {
    /// Opens an existing OPFS file, failing with [`io::ErrorKind::NotFound`] if it is absent.
    pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let file = re_async::spawn_local_with_result(async move {
            let file_handle = open_file(&path).await?;
            await_js(file_handle.get_file()).await
        })
        .await
        .map_err(io::Error::other)??;
        Ok(Self { file })
    }
}

#[async_trait::async_trait]
impl re_async::AsyncReadAt for File {
    async fn read_exact_at(&self, offset: u64, len: usize) -> io::Result<bytes::Bytes> {
        let file = self.file.clone();
        re_async::spawn_local_with_result(async move {
            let end = offset.checked_add(len as u64).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "read range overflows u64")
            })?;

            let blob: &web_sys::Blob = file.as_ref();
            // `Blob::slice` clamps to the file end, so a short result means EOF was reached.
            let slice = blob
                .slice_with_f64_and_f64(offset as f64, end as f64)
                .map_err(|err| js_to_io_error(&err))?;
            let buffer: js_sys::ArrayBuffer = await_js(slice.array_buffer()).await?;
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();

            if bytes.len() < len {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "read past end of file",
                ));
            }
            Ok(bytes.into())
        })
        .await
        .map_err(io::Error::other)?
    }

    async fn size(&self) -> io::Result<u64> {
        let file = self.file.clone();
        re_async::spawn_local_with_result(async move {
            let blob: &web_sys::Blob = file.as_ref();
            Ok(blob.size() as u64)
        })
        .await
        .map_err(io::Error::other)?
    }
}

/// Write `contents` to `path`, creating any missing parent directories.
///
/// Takes `contents` by value so callers that already own the bytes avoid a copy; the whole
/// buffer would otherwise be duplicated on the Wasm heap for large uploads.
pub async fn write(path: impl AsRef<Path>, contents: Arc<[u8]>) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    re_async::spawn_local_with_result(async move {
        let file_handle = create_file(&path).await?;
        let writer: FileSystemWritableFileStream = await_js(file_handle.create_writable()).await?;

        if let Err(err) = write_all(&writer, &contents).await {
            // `createWritable` commits atomically on close. Aborting preserves any previous file
            // and prevents a partial write from becoming visible.
            let writable_stream: &web_sys::WritableStream = writer.as_ref();
            writable_stream.abort().await.ok();
            return Err(err);
        }

        Ok(())
    })
    .await
    .map_err(io::Error::other)?
}

/// Copy a browser file into OPFS without moving its payload through Wasm linear memory.
pub async fn write_file(path: impl AsRef<Path>, file: web_sys::File) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    let blob: web_sys::Blob = file.unchecked_into();
    let readable_stream = blob.stream();
    re_async::spawn_local_with_result(async move {
        let file_handle = create_file(&path).await?;
        let writer: FileSystemWritableFileStream = await_js(file_handle.create_writable()).await?;
        let writable_stream: &web_sys::WritableStream = writer.as_ref();

        if let Err(err) = await_js::<JsValue>(readable_stream.pipe_to(writable_stream)).await {
            // `pipeTo` normally aborts its destination on failure. Abort explicitly as well so
            // this remains transactional if browser defaults change.
            writable_stream.abort().await.ok();
            return Err(err);
        }

        Ok(())
    })
    .await
    .map_err(io::Error::other)?
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
    re_async::spawn_local_with_result(async move {
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
    .map_err(io::Error::other)?
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

async fn await_js<T>(promise: js_sys::Promise) -> io::Result<T>
where
    T: JsCast,
{
    promise
        .await
        .map_err(|err| js_to_io_error(&err))?
        .dyn_into()
        .map_err(|err| js_to_io_error(&err))
}

fn js_to_io_error(value: &JsValue) -> io::Error {
    if let Some(exception) = value.dyn_ref::<DomException>() {
        return err_from_dom_exception(exception);
    }

    io::Error::other(crate::Error::from(value))
}

fn err_from_dom_exception(exception: &DomException) -> io::Error {
    let kind = match exception.code() {
        DomException::NOT_FOUND_ERR => io::ErrorKind::NotFound,
        DomException::SECURITY_ERR => io::ErrorKind::PermissionDenied,
        DomException::TYPE_MISMATCH_ERR => io::ErrorKind::InvalidInput,
        DomException::QUOTA_EXCEEDED_ERR => io::ErrorKind::StorageFull,
        _ => io::ErrorKind::Other,
    };

    io::Error::new(kind, exception.message())
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io;

    use re_async::AsyncReadAt as _;
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
        assert_eq!(metadata.len(), b"first write".len() as u64);
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
    async fn write_file_overwrites_and_preserves_contents() {
        let test_dir = unique_opfs_test_dir();
        let file_path = format!("/{test_dir}/streamed.bin");

        write_file(&file_path, file(b"streamed contents"))
            .await
            .expect("streamed write should succeed");
        assert_eq!(
            read(file_path.as_ref()).await.expect("read should succeed"),
            b"streamed contents",
        );
        assert_eq!(
            metadata(file_path.as_ref())
                .await
                .expect("metadata should succeed")
                .len(),
            b"streamed contents".len() as u64,
        );

        write_file(&file_path, file(b"short"))
            .await
            .expect("streamed overwrite should succeed");
        assert_eq!(
            read(file_path.as_ref()).await.expect("read should succeed"),
            b"short",
        );
        assert_eq!(
            metadata(file_path.as_ref())
                .await
                .expect("metadata should succeed")
                .len(),
            b"short".len() as u64,
        );

        remove_dir_all(test_dir)
            .await
            .expect("test cleanup should remove the OPFS directory");
    }

    fn file(contents: &[u8]) -> web_sys::File {
        let bytes = js_sys::Uint8Array::from(contents);
        let parts = js_sys::Array::new();
        parts.push(&bytes);
        web_sys::File::new_with_u8_array_sequence(&parts, "source.bin")
            .expect("File creation should succeed")
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

    #[wasm_bindgen_test]
    async fn file_reads_file() {
        let test_dir = unique_opfs_test_dir();
        let file_path = format!("/{test_dir}/data.bin");
        let contents = b"0123456789";

        write(&file_path, Vec::from(contents).into())
            .await
            .expect("write should succeed");

        let file = File::open(&file_path).await.expect("open should succeed");

        assert_eq!(
            file.size().await.expect("size should succeed"),
            contents.len() as u64,
        );

        assert_eq!(
            file.read_exact_at(3, 4)
                .await
                .expect("mid-file read should succeed"),
            b"3456".as_slice(),
        );

        // A read ending exactly at EOF returns all requested bytes.
        assert_eq!(
            file.read_exact_at(6, 4)
                .await
                .expect("tail read should succeed"),
            b"6789".as_slice(),
        );

        let err = file
            .read_exact_at(8, 4)
            .await
            .expect_err("reading past the end should fail");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);

        let err = File::open(format!("/{test_dir}/missing.bin"))
            .await
            .expect_err("opening a missing OPFS file should fail");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);

        remove_dir_all(test_dir)
            .await
            .expect("test cleanup should remove the OPFS directory");
    }
}
