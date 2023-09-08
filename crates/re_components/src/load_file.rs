use re_log_types::DataCell;

/// Errors from [`data_cells_from_file_path`] and [`data_cells_from_mesh_file_path`].
#[derive(thiserror::Error, Debug)]
pub enum FromFileError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    FileRead(#[from] std::io::Error),

    #[error(transparent)]
    DataCellError(#[from] re_log_types::DataCellError),

    #[cfg(feature = "image")]
    #[error(transparent)]
    TensorImageLoad(#[from] re_types::tensor_data::TensorImageLoadError),

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
#[cfg(not(target_arch = "wasm32"))]
pub fn data_cells_from_file_path(
    file_path: &std::path::Path,
) -> Result<Vec<DataCell>, FromFileError> {
    let extension = file_path
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();

    match extension.as_str() {
        "glb" => data_cells_from_mesh_file_path(file_path, crate::MeshFormat::Glb),
        "glft" => data_cells_from_mesh_file_path(file_path, crate::MeshFormat::Gltf),
        "obj" => data_cells_from_mesh_file_path(file_path, crate::MeshFormat::Obj),

        #[cfg(feature = "image")]
        _ => {
            use re_types::Archetype;
            let indicator = <re_types::archetypes::Image as Archetype>::Indicator::new_list(1);
            let indicator_cell = DataCell::from_arrow(
                re_types::archetypes::Image::indicator_component(),
                indicator.to_arrow(),
            );

            // Assume an image (there are so many image extensions):
            let tensor = re_types::components::TensorData(
                re_types::datatypes::TensorData::from_image_file(file_path)?,
            );
            Ok(vec![
                indicator_cell,
                DataCell::try_from_native(std::iter::once(&tensor))?,
            ])
        }

        #[cfg(not(feature = "image"))]
        _ => Err(FromFileError::UnknownExtension {
            extension,
            path: file_path.to_owned(),
        }),
    }
}

pub fn data_cells_from_file_contents(
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<Vec<DataCell>, FromFileError> {
    re_tracing::profile_function!(file_name);

    let extension = std::path::Path::new(file_name)
        .extension()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();

    match extension.as_str() {
        "glb" => data_cells_from_mesh_file_contents(bytes, crate::MeshFormat::Glb),
        "glft" => data_cells_from_mesh_file_contents(bytes, crate::MeshFormat::Gltf),
        "obj" => data_cells_from_mesh_file_contents(bytes, crate::MeshFormat::Obj),

        #[cfg(feature = "image")]
        _ => {
            let format = if let Some(format) = image::ImageFormat::from_extension(extension) {
                format
            } else {
                image::guess_format(&bytes)
                    .map_err(re_types::tensor_data::TensorImageLoadError::from)?
            };

            use re_types::Archetype;
            let indicator = <re_types::archetypes::Image as Archetype>::Indicator::new_list(1);
            let indicator_cell = DataCell::from_arrow(
                re_types::archetypes::Image::indicator_component(),
                indicator.to_arrow(),
            );

            // Assume an image (there are so many image extensions):
            let tensor = re_types::components::TensorData(
                re_types::datatypes::TensorData::from_image_bytes(bytes, format)?,
            );
            Ok(vec![
                indicator_cell,
                DataCell::try_from_native(std::iter::once(&tensor))?,
            ])
        }

        #[cfg(not(feature = "image"))]
        _ => Err(FromFileError::UnknownExtension {
            extension,
            path: file_name.to_owned().into(),
        }),
    }
}

/// Read the mesh file at the given path.
///
/// Supported file extensions are:
///  * `glb`, `gltf`, `obj`: encoded meshes, leaving it to the viewer to decode
///
/// All other extensions will return an error.
#[cfg(not(target_arch = "wasm32"))]
pub fn data_cells_from_mesh_file_path(
    file_path: &std::path::Path,
    format: crate::MeshFormat,
) -> Result<Vec<DataCell>, FromFileError> {
    let bytes = std::fs::read(file_path)?;
    data_cells_from_mesh_file_contents(bytes, format)
}

pub fn data_cells_from_mesh_file_contents(
    bytes: Vec<u8>,
    format: crate::MeshFormat,
) -> Result<Vec<DataCell>, FromFileError> {
    // TODO(#2788): mesh indicator
    let mesh = crate::EncodedMesh3D {
        format,
        bytes: bytes.into(),
        transform: [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
        ],
    };
    let mesh = crate::Mesh3D::Encoded(mesh);
    Ok(vec![DataCell::try_from_native(std::iter::once(&mesh))?])
}
