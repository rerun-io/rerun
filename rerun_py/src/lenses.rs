use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use re_lenses_core::{DynExpr, Lens, OutputMode, Selector};
use re_types_core::{ComponentDescriptor, ComponentIdentifier};

use crate::python_bridge::PyComponentDescriptor;
use crate::selector::PySelectorInternal;

/// Register lens classes.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDeriveLensInternal>()?;
    m.add_class::<PyMutateLensInternal>()?;
    Ok(())
}

/// A derive lens that creates new component/time columns from an input component.
///
/// In Python, `scatter=True` maps to `Lens::Scatter` internally.
#[pyclass(
    frozen,
    name = "DeriveLensInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyDeriveLensInternal {
    components: Vec<(ComponentDescriptor, Selector<DynExpr>)>,
    times: Vec<(String, re_log_types::TimeType, Selector<DynExpr>)>,
    input_component: ComponentIdentifier,
    output_entity: Option<String>,
    scatter: bool,
}

#[pymethods]
impl PyDeriveLensInternal {
    #[new]
    #[pyo3(
        signature = (input_component, *, output_entity = None, scatter = false),
        text_signature = "(self, input_component, *, output_entity=None, scatter=False)"
    )]
    fn new(input_component: &str, output_entity: Option<String>, scatter: bool) -> Self {
        Self {
            components: Vec::new(),
            times: Vec::new(),
            input_component: input_component.into(),
            output_entity,
            scatter,
        }
    }

    /// Add a component output column. Returns a new instance with the component added.
    fn to_component(
        &self,
        component: PyComponentDescriptor,
        selector: &PySelectorInternal,
    ) -> Self {
        let mut components = self.components.clone();
        components.push((component.0, selector.selector().clone()));
        Self {
            components,
            times: self.times.clone(),
            input_component: self.input_component,
            output_entity: self.output_entity.clone(),
            scatter: self.scatter,
        }
    }

    /// Add a time extraction column. Returns a new instance with the time added.
    fn to_timeline(
        &self,
        timeline_name: &str,
        timeline_type: &str,
        selector: &PySelectorInternal,
    ) -> PyResult<Self> {
        let parsed_type = parse_timeline_type(timeline_type)?;
        let mut times = self.times.clone();
        times.push((
            timeline_name.to_owned(),
            parsed_type,
            selector.selector().clone(),
        ));
        Ok(Self {
            components: self.components.clone(),
            times,
            input_component: self.input_component,
            output_entity: self.output_entity.clone(),
            scatter: self.scatter,
        })
    }
}

impl PyDeriveLensInternal {
    /// Build the Rust `Lens` from this internal representation.
    pub fn build(&self) -> PyResult<Lens> {
        let mut builder = if self.scatter {
            Lens::scatter(self.input_component)
        } else {
            Lens::derive(self.input_component)
        };
        if let Some(ref entity) = self.output_entity {
            builder = builder.output_entity(entity.as_str());
        }
        for (descr, selector) in &self.components {
            builder = builder.to_component(descr.clone(), selector.clone());
        }
        for (name, timeline_type, selector) in &self.times {
            builder = builder.to_timeline(name.as_str(), *timeline_type, selector.clone());
        }
        builder
            .build()
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }
}

/// A mutate lens that modifies the input component in-place.
#[pyclass(
    frozen,
    name = "MutateLensInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyMutateLensInternal {
    input_component: ComponentIdentifier,
    selector: Selector<DynExpr>,
    keep_row_ids: bool,
}

#[pymethods]
impl PyMutateLensInternal {
    #[new]
    #[pyo3(
        signature = (input_component, selector, *, keep_row_ids = false),
        text_signature = "(self, input_component, selector, *, keep_row_ids=False)"
    )]
    fn new(input_component: &str, selector: &PySelectorInternal, keep_row_ids: bool) -> Self {
        Self {
            input_component: input_component.into(),
            selector: selector.selector().clone(),
            keep_row_ids,
        }
    }
}

impl PyMutateLensInternal {
    /// Build the Rust `Lens` from this internal representation.
    pub fn build(&self) -> Lens {
        let mut builder = Lens::mutate(self.input_component, self.selector.clone());
        if self.keep_row_ids {
            builder = builder.keep_row_ids();
        }
        builder.build()
    }
}

/// Extracts a `Lens` from either derive or mutate Python lens types.
pub enum PyLens<'py> {
    Derive(PyRef<'py, PyDeriveLensInternal>),
    Mutate(PyRef<'py, PyMutateLensInternal>),
}

impl PyLens<'_> {
    pub fn build(&self) -> PyResult<Lens> {
        match self {
            Self::Derive(d) => d.build(),
            Self::Mutate(i) => Ok(i.build()),
        }
    }
}

impl<'py> FromPyObject<'py> for PyLens<'py> {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        if let Ok(d) = ob.downcast::<PyDeriveLensInternal>() {
            Ok(Self::Derive(d.borrow()))
        } else if let Ok(i) = ob.downcast::<PyMutateLensInternal>() {
            Ok(Self::Mutate(i.borrow()))
        } else {
            Err(PyValueError::new_err(
                "Expected a DeriveLensInternal or MutateLensInternal instance",
            ))
        }
    }
}

fn parse_timeline_type(s: &str) -> PyResult<re_log_types::TimeType> {
    match s {
        "sequence" => Ok(re_log_types::TimeType::Sequence),
        "duration_ns" => Ok(re_log_types::TimeType::DurationNs),
        "timestamp_ns" => Ok(re_log_types::TimeType::TimestampNs),
        _ => Err(PyValueError::new_err(format!(
            "Unknown timeline type '{s}', expected 'sequence', 'duration_ns', or 'timestamp_ns'"
        ))),
    }
}

/// Parse an output mode string from Python.
pub fn parse_output_mode(s: &str) -> PyResult<OutputMode> {
    match s {
        "forward_all" => Ok(OutputMode::ForwardAll),
        "forward_unmatched" => Ok(OutputMode::ForwardUnmatched),
        "drop_unmatched" => Ok(OutputMode::DropUnmatched),
        _ => Err(PyValueError::new_err(format!(
            "Unknown output_mode '{s}', expected 'forward_all', 'forward_unmatched', or 'drop_unmatched'"
        ))),
    }
}
