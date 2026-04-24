use std::collections::BTreeMap;

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

/// Describes one output group of a lens.
#[pyclass(
    frozen,
    name = "LensOutputInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyLensOutputInternal {
    components: Vec<(ComponentDescriptor, Selector<DynExpr>)>,
    times: Vec<(String, re_log_types::TimeType, Selector<DynExpr>)>,
}

#[pymethods]
impl PyLensOutputInternal {
    #[new]
    #[pyo3(text_signature = "(self)")]
    fn new() -> Self {
        Self {
            components: Vec::new(),
            times: Vec::new(),
        }
    }

    /// Add a component output column. Returns a new LensOutput with the component added.
    fn to_component(
        &self,
        component: PyComponentDescriptor,
        selector: &PySelectorInternal,
    ) -> Self {
        let descr = component.0;
        let mut components = self.components.clone();
        components.push((descr, selector.selector().clone()));
        Self {
            components,
            times: self.times.clone(),
        }
    }

    /// Add a time extraction column. Returns a new LensOutput with the time added.
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
        signature = (input_component, output = None, *, to_entity = None),
        text_signature = "(self, input_component, output=None, *, to_entity=None)"
    )]
    #[expect(clippy::needless_pass_by_value)] // PyO3 requires owned arguments
    fn new(
        py: Python<'_>,
        input_component: &str,
        output: Option<PyRef<'_, PyLensOutputInternal>>,
        to_entity: Option<BTreeMap<String, Py<PyLensOutputInternal>>>,
    ) -> PyResult<Self> {
        if output.is_none() && to_entity.as_ref().is_none_or(BTreeMap::is_empty) {
            return Err(PyValueError::new_err(
                "At least one of `output` or `to_entity` must be provided",
            ));
        }

        let component: ComponentIdentifier = input_component.into();
        let mut builder = Lens::for_input_column(component);

        if let Some(ref out) = output {
            builder = build_output(builder, out, None)?;
        }

        if let Some(ref to_entity) = to_entity {
            for (entity_path, out) in to_entity {
                let out = out.borrow(py);
                builder = build_output(builder, &out, Some(entity_path.as_str()))?;
            }
        }

        Ok(Self {
            inner: builder.build(),
        })
    }
}

/// Build one output group from its description, appending it to the lens builder.
fn build_output(
    builder: LensBuilder,
    desc: &PyLensOutputInternal,
    target_entity: Option<&str>,
) -> PyResult<LensBuilder> {
    let build_fn = |mut out: re_lenses_core::OutputBuilder| {
        for (descr, selector) in &desc.components {
            out = out.component(descr.clone(), selector.clone())?;
        }
        for (name, timeline_type, selector) in &desc.times {
            out = out.time(name.as_str(), *timeline_type, selector.clone())?;
        }
        Ok(out)
    };

    let result = match target_entity {
        None => builder.output_columns(build_fn),
        Some(target) => builder.output_columns_at(target, build_fn),
    };

    result.map_err(|err| PyValueError::new_err(err.to_string()))
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
