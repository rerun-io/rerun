use crate::DataCell;

/// Errors from [`data_cells_from_file_path`].
#[derive(thiserror::Error, Debug)]
pub enum FromFileError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    FileRead(#[from] std::io::Error),

    #[error(transparent)]
    DataCellError(#[from] crate::DataCellError),

    #[cfg(feature = "image")]
    #[error(transparent)]
    TensorImageLoad(#[from] re_types::tensor_data::TensorImageLoadError),

    #[error("Unsupported file extension '{extension}' for file {path:?}. To load image files, make sure you compile with the 'image' feature")]
    UnknownExtension {
        extension: String,
        path: std::path::PathBuf,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
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
        "glb" | "gltf" | "obj" => {
            use re_types::{archetypes::Asset3D, AsComponents as _};
            let cells: Result<Vec<_>, _> = Asset3D::from_file(file_path)?
                // TODO(#3414): this should be a method of `Archetype`
                .as_component_batches()
                .into_iter()
                .map(|comp_batch| {
                    let comp_batch = comp_batch.as_ref();
                    Ok(DataCell::from_arrow(
                        comp_batch.name(),
                        comp_batch
                            .to_arrow()
                            .map_err(|err| anyhow::anyhow!("serialization failed: {err}"))?,
                    ))
                })
                .collect();
            cells
        }

        #[cfg(feature = "image")]
        _ => {
            use re_types::{Archetype, AsComponents as _, ResultExt as _};
            let indicator = <re_types::archetypes::Image as Archetype>::indicator().as_ref();
            let indicator_cell = DataCell::from_arrow(
                re_types::archetypes::Image::indicator().name(),
                indicator
                    .try_to_arrow()
                    .map_err(|err| anyhow::anyhow!("serialization failed: {err}"))?,
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
        "glb" | "gltf" | "obj" => {
            use re_types::{archetypes::Asset3D, components::MediaType, AsComponents as _};
            let cells: Result<Vec<_>, _> =
                Asset3D::from_bytes(bytes, MediaType::guess_from_path(file_name))
                    .as_component_batches()
                    .into_iter()
                    .map(|comp_batch| {
                        let comp_batch = comp_batch.as_ref();
                        Ok(DataCell::from_arrow(
                            comp_batch.name(),
                            comp_batch
                                .to_arrow()
                                .map_err(|err| anyhow::anyhow!("serialization failed: {err}"))?,
                        ))
                    })
                    .collect();
            cells
        }

        #[cfg(feature = "image")]
        _ => {
            let format = if let Some(format) = image::ImageFormat::from_extension(extension) {
                format
            } else {
                image::guess_format(&bytes)
                    .map_err(re_types::tensor_data::TensorImageLoadError::from)?
            };

            use re_types::{Archetype, AsComponents as _, ResultExt as _};
            let indicator = <re_types::archetypes::Image as Archetype>::indicator().as_ref();
            let indicator_cell = DataCell::from_arrow(
                re_types::archetypes::Image::indicator().as_ref().name(),
                indicator
                    .try_to_arrow()
                    .map_err(|err| anyhow::anyhow!("serialization failed: {err}"))?,
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
