use pyo3::{pyclass, pymethods};

use re_sorbet::{ComponentColumnDescriptor, ComponentColumnSelector};

/// The descriptor of a component column.
///
/// Component columns contain the data for a specific component of an entity.
///
/// Column descriptors are used to describe the columns in a
/// [`Schema`][rerun.dataframe.Schema]. They are read-only. To select a component
/// column, use [`ComponentColumnSelector`][rerun.dataframe.ComponentColumnSelector].
#[pyclass(frozen, name = "ComponentColumnDescriptor")]
#[derive(Clone)]
pub struct PyComponentColumnDescriptor(pub ComponentColumnDescriptor);

impl From<ComponentColumnDescriptor> for PyComponentColumnDescriptor {
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self(desc)
    }
}

#[pymethods]
impl PyComponentColumnDescriptor {
    pub fn __repr__(&self) -> String {
        format!(
            "Column name: {col}\n\
             \tEntity path: {path}\n\
             \tArchetype: {arch}\n\
             \tComponent type: {ctype}\n\
             \tComponent: {comp}",
            col = self.0.column_name(re_sorbet::BatchType::Dataframe),
            path = self.entity_path(),
            arch = self.archetype().unwrap_or("None"),
            ctype = self.component_type().unwrap_or(""),
            comp = self.component(),
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    /// The entity path.
    ///
    /// This property is read-only.
    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    /// The component.
    ///
    /// This property is read-only.
    #[getter]
    fn component(&self) -> &str {
        &self.0.component
    }

    /// The component type, if any.
    ///
    /// This property is read-only.
    #[getter]
    fn component_type(&self) -> Option<&str> {
        self.0.component_type.map(|c| c.as_str())
    }

    /// The archetype name, if any.
    ///
    /// This property is read-only.
    #[getter]
    fn archetype(&self) -> Option<&str> {
        self.0.archetype.map(|c| c.as_str())
    }

    /// Whether the column is static.
    ///
    /// This property is read-only.
    #[getter]
    fn is_static(&self) -> bool {
        self.0.is_static
    }

    /// Whether the column is an indicator column.
    ///
    /// This property is read-only.
    #[getter]
    fn is_indicator(&self) -> bool {
        self.0.component_descriptor().is_indicator_component()
    }
}

impl From<PyComponentColumnDescriptor> for ComponentColumnDescriptor {
    fn from(desc: PyComponentColumnDescriptor) -> Self {
        desc.0
    }
}

/// A selector for a component column.
///
/// Component columns contain the data for a specific component of an entity.
///
/// Parameters
/// ----------
/// entity_path : str
///     The entity path to select.
/// component : str
///     The component to select
#[pyclass(frozen, name = "ComponentColumnSelector")]
#[derive(Clone)]
pub struct PyComponentColumnSelector(pub ComponentColumnSelector);

#[pymethods]
impl PyComponentColumnSelector {
    /// Create a new `ComponentColumnSelector`.
    // Note: the `Parameters` section goes into the class docstring.
    #[new]
    #[pyo3(text_signature = "(self, entity_path: str, component: str)")]
    fn new(entity_path: &str, component: &str) -> Self {
        Self(ComponentColumnSelector {
            entity_path: entity_path.into(),
            component: component.to_owned(),
        })
    }

    fn __repr__(&self) -> String {
        format!("{}", self.0)
    }

    /// The entity path.
    ///
    /// This property is read-only.
    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    /// The component.
    ///
    /// This property is read-only.
    #[getter]
    fn component(&self) -> &str {
        &self.0.component
    }
}

impl From<PyComponentColumnSelector> for ComponentColumnSelector {
    fn from(selector: PyComponentColumnSelector) -> Self {
        selector.0
    }
}
