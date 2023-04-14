mod tensor_slice_to_gpu;

mod scene;
pub(crate) use self::scene::SceneTensor;

mod ui;
pub(crate) use self::ui::{view_tensor, ViewTensorState};

mod tensor_dimension_mapper;
pub use self::tensor_dimension_mapper::dimension_mapping_ui;
