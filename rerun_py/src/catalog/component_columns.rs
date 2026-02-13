use pyo3::{pyclass, pymethods};
use re_sorbet::{ComponentColumnDescriptor, ComponentColumnSelector};

/// The descriptor of a component column.
///
/// Component columns contain the data for a specific component of an entity.
///
/// Column descriptors are used to describe the columns in a
/// [`Schema`][rerun.catalog.Schema]. They are read-only. To select a component
/// column, use [`ComponentColumnSelector`][rerun.catalog.ComponentColumnSelector].
#[pyclass(
    frozen,
    hash,
    eq,
    name = "ComponentColumnDescriptor",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyComponentColumnDescriptor(pub ComponentColumnDescriptor);

impl From<ComponentColumnDescriptor> for PyComponentColumnDescriptor {
    fn from(desc: ComponentColumnDescriptor) -> Self {
        Self(desc)
    }
}

#[pymethods]
impl PyComponentColumnDescriptor {
    pub fn __repr__(&self) -> String {
        // We could print static state all the time
        // but in schema non-static print out with IndexColumnDescriptors
        // so it looks a bit noisy.
        let static_info = if self.is_static() {
            "\n\tStatic: true"
        } else {
            ""
        };

        format!(
            "Column name: {col}\n\
             \tEntity path: {path}\n\
             \tArchetype: {arch}\n\
             \tComponent type: {ctype}\n\
             \tComponent: {comp}{static_info}",
            col = self.name(),
            path = self.entity_path(),
            arch = self.archetype().unwrap_or("None"),
            ctype = self.component_type().unwrap_or(""),
            comp = self.component(),
            static_info = static_info,
        )
    }

    /// The entity path.
    ///
    /// This property is read-only.
    #[getter]
    fn entity_path(&self) -> String {
        self.0.entity_path.to_string()
    }

    /// Is this column a property?
    #[getter]
    fn is_property(&self) -> bool {
        self.0.entity_path.is_property()
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

    /// The name of this column.
    ///
    /// This property is read-only.
    #[getter]
    fn name(&self) -> String {
        self.0.column_name(re_sorbet::BatchType::Dataframe)
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
#[pyclass(
    frozen,
    eq,
    name = "ComponentColumnSelector",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, PartialEq, Eq)]
pub struct PyComponentColumnSelector(pub ComponentColumnSelector);

impl std::fmt::Display for PyComponentColumnSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(selector) = self;
        f.write_fmt(format_args!("{selector}"))
    }
}

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
        self.to_string()
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
