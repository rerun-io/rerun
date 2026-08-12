use arrow::datatypes::DataType;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::{Borrowed, FromPyObject};

use re_lenses_core::{CastTo, DynExpr, Lens, OutputMode, Selector};
use re_types_core::{ComponentDescriptor, ComponentIdentifier, TimelineName};

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
    components: Vec<(ComponentDescriptor, Selector<DynExpr>, Option<CastTo>)>,
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
    fn new(input_component: &str, output_entity: Option<String>, scatter: bool) -> PyResult<Self> {
        Ok(Self {
            components: Vec::new(),
            times: Vec::new(),
            input_component: ComponentIdentifier::try_new(input_component)
                .map_err(|err| PyValueError::new_err(err.to_string()))?,
            output_entity,
            scatter,
        })
    }

    /// Add a component output column. Returns a new instance with the component added.
    ///
    /// `cast_to` is `None` (no cast), the string `"auto"` (cast to the component's
    /// canonical type), or a pyarrow `DataType` (cast to that explicit type).
    #[pyo3(signature = (component, selector, cast_to = None))]
    fn to_component(
        &self,
        component: PyComponentDescriptor,
        selector: &PySelectorInternal,
        cast_to: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let cast = parse_cast_to(cast_to)?;
        let mut components = self.components.clone();
        components.push((component.0, selector.selector().clone(), cast));
        Ok(Self {
            components,
            times: self.times.clone(),
            input_component: self.input_component,
            output_entity: self.output_entity.clone(),
            scatter: self.scatter,
        })
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
        for (descr, selector, cast) in &self.components {
            builder = match cast {
                Some(cast) => {
                    builder.to_component_with_cast(descr.clone(), selector.clone(), cast.clone())
                }
                None => builder.to_component(descr.clone(), selector.clone()),
            };
        }
        for (name, timeline_type, selector) in &self.times {
            let timeline_name = TimelineName::try_new(name.as_str())
                .map_err(|err| PyValueError::new_err(err.to_string()))?;
            builder = builder.to_timeline(timeline_name, *timeline_type, selector.clone());
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
    fn new(
        input_component: &str,
        selector: &PySelectorInternal,
        keep_row_ids: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            input_component: ComponentIdentifier::try_new(input_component)
                .map_err(|err| PyValueError::new_err(err.to_string()))?,
            selector: selector.selector().clone(),
            keep_row_ids,
        })
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
pub enum PyLens {
    Derive(Py<PyDeriveLensInternal>),
    Mutate(Py<PyMutateLensInternal>),
}

impl PyLens {
    pub fn build(&self, py: Python<'_>) -> PyResult<Lens> {
        match self {
            Self::Derive(d) => d.borrow(py).build(),
            Self::Mutate(i) => Ok(i.borrow(py).build()),
        }
    }
}

impl<'py> FromPyObject<'_, 'py> for PyLens {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(d) = ob.cast::<PyDeriveLensInternal>() {
            Ok(Self::Derive(d.to_owned().unbind()))
        } else if let Ok(i) = ob.cast::<PyMutateLensInternal>() {
            Ok(Self::Mutate(i.to_owned().unbind()))
        } else {
            Err(PyValueError::new_err(
                "Expected a DeriveLensInternal or MutateLensInternal instance",
            ))
        }
    }
}

/// Parse the Python `cast_to` argument: `None`, the string `"auto"`, or a pyarrow `DataType`.
fn parse_cast_to(cast_to: Option<Bound<'_, PyAny>>) -> PyResult<Option<CastTo>> {
    let Some(obj) = cast_to else {
        return Ok(None);
    };
    if let Ok(s) = obj.extract::<String>() {
        return match s.as_str() {
            "auto" => Ok(Some(CastTo::Auto)),
            other => Err(PyValueError::new_err(format!(
                "Unknown cast_to '{other}', expected 'auto' or a pyarrow DataType"
            ))),
        };
    }
    let PyArrowType(datatype) = obj.extract::<PyArrowType<DataType>>()?;
    Ok(Some(CastTo::Type(datatype)))
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
