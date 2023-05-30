use re_log_types::DataCell;

/// Errors from [`DataCell::data_cell_from_file_path`] and [`data_cell_from_mesh_file_path`].
#[derive(thiserror::Error, Debug)]
pub enum FromFileError {
    #[error(transparent)]
    FileRead(#[from] std::io::Error),

    #[error(transparent)]
    DataCellError(#[from] re_log_types::DataCellError),

    #[cfg(feature = "image")]
    #[error(transparent)]
    TensorImageLoad(#[from] crate::TensorImageLoadError),

    #[error("Unsupported file extension '{extension}' for file {path:?}. To load image files, make sure you compile with the 'image' feature")]
    UnknownExtension {
        extension: String,
        path: std::path::PathBuf,
    },
}

/// Read the file at the given path.
///
/// Supported file extensions are:
///  * `glb`, `gltf`, `obj`: encoded meshes, leaving it to the viewer to decode
///  * `jpg`, `jpeg`: encoded JPEG, leaving it to the viewer to decode. Requires the `image` feature.
///  * `png` and other image formats: decoded here. Requires the `image` feature.
///
/// All other extensions will return an error.
pub fn data_cell_from_file_path(file_path: &std::path::Path) -> Result<DataCell, FromFileError> {
    let extension = file_path
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();

    match extension.as_str() {
        "glb" => data_cell_from_mesh_file_path(file_path, crate::MeshFormat::Glb),
        "glft" => data_cell_from_mesh_file_path(file_path, crate::MeshFormat::Gltf),
        "obj" => data_cell_from_mesh_file_path(file_path, crate::MeshFormat::Obj),

        #[cfg(feature = "image")]
        _ => {
            // Assume an image (there are so many image extensions):
            let tensor = crate::Tensor::from_image_file(file_path)?;
            Ok(DataCell::try_from_native(std::iter::once(&tensor))?)
        }

        #[cfg(not(feature = "image"))]
        _ => Err(FromFileError::UnknownExtension {
            extension,
            path: file_path.to_owned(),
        }),
    }
}

/// Read the mesh file at the given path.
///
/// Supported file extensions are:
///  * `glb`, `gltf`, `obj`: encoded meshes, leaving it to the viewer to decode
///
/// All other extensions will return an error.
pub fn data_cell_from_mesh_file_path(
    file_path: &std::path::Path,
    format: crate::MeshFormat,
) -> Result<DataCell, FromFileError> {
    let mesh = crate::EncodedMesh3D {
        mesh_id: crate::MeshId::random(),
        format,
        bytes: std::fs::read(file_path)?.into(),
        transform: [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
        ],
    };
    let mesh = crate::Mesh3D::Encoded(mesh);
    Ok(DataCell::try_from_native(std::iter::once(&mesh))?)
}
