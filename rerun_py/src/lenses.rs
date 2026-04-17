use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use re_lenses_core::{DynExpr, Lens, LensBuilder, OutputMode, Selector};
use re_types_core::{ComponentDescriptor, ComponentIdentifier};

use crate::python_bridge::PyComponentDescriptor;
use crate::selector::PySelectorInternal;

/// Register lens classes.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLensOutputInternal>()?;
    m.add_class::<PyLensInternal>()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// LensOutputInternal
// ---------------------------------------------------------------------------

/// Describes one output group: either 1:1 (columns) or 1:N (scatter columns).
#[pyclass(
    frozen,
    name = "LensOutputInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLensOutputInternal {
    scatter: bool,
    target_entity: Option<String>,
    components: Vec<(ComponentDescriptor, Selector<DynExpr>)>,
    times: Vec<(String, re_log_types::TimeType, Selector<DynExpr>)>,
}

#[pymethods]
impl PyLensOutputInternal {
    #[new]
    #[pyo3(
        signature = (*, scatter = false, target_entity = None),
        text_signature = "(self, *, scatter=False, target_entity=None)"
    )]
    fn new(scatter: bool, target_entity: Option<String>) -> Self {
        Self {
            scatter,
            target_entity,
            components: Vec::new(),
            times: Vec::new(),
        }
    }

    /// Add a component output column. Returns a new LensOutput with the component added.
    fn component(&self, component: PyComponentDescriptor, selector: &PySelectorInternal) -> Self {
        let descr = component.0;
        let mut components = self.components.clone();
        components.push((descr, selector.selector().clone()));
        Self {
            scatter: self.scatter,
            target_entity: self.target_entity.clone(),
            components,
            times: self.times.clone(),
        }
    }

    /// Add a time extraction column. Returns a new LensOutput with the time added.
    fn time(
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
            scatter: self.scatter,
            target_entity: self.target_entity.clone(),
            components: self.components.clone(),
            times,
        })
    }
}

// ---------------------------------------------------------------------------
// LensInternal
// ---------------------------------------------------------------------------

#[pyclass(
    frozen,
    name = "LensInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLensInternal {
    inner: Lens,
}

impl PyLensInternal {
    pub fn inner(&self) -> &Lens {
        &self.inner
    }
}

#[pymethods]
impl PyLensInternal {
    #[new]
    #[pyo3(
        signature = (input_component, *, outputs),
        text_signature = "(self, input_component, *, outputs)"
    )]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned arguments
    fn new(input_component: &str, outputs: Vec<PyRef<'_, PyLensOutputInternal>>) -> PyResult<Self> {
        let component: ComponentIdentifier = input_component.into();

        let mut builder = Lens::for_input_column(component);

        for output in &outputs {
            builder = build_output(builder, output)?;
        }

        Ok(Self {
            inner: builder.build(),
        })
    }
}

/// Build one output group from its description, appending it to the lens builder.
fn build_output(builder: LensBuilder, desc: &PyLensOutputInternal) -> PyResult<LensBuilder> {
    builder
        .output(desc.scatter, |mut out| {
            if let Some(ref target) = desc.target_entity {
                out = out.at_entity(target.as_str());
            }
            for (descr, selector) in &desc.components {
                out = out.component(descr.clone(), selector.clone())?;
            }
            for (name, timeline_type, selector) in &desc.times {
                out = out.time(name.as_str(), *timeline_type, selector.clone())?;
            }
            Ok(out)
        })
        .map_err(|err| PyValueError::new_err(err.to_string()))
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
