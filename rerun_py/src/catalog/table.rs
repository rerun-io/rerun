use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::{
    exceptions::PyRuntimeError,
    pyclass, pymethods,
    types::{PyAnyMethods, PyCapsule},
    Bound, PyAny, PyRef, PyRefMut, PyResult,
};

use re_datafusion::TableEntryTableProvider;

use crate::{
    catalog::PyEntry,
    utils::{get_tokio_runtime, wait_for_future},
};

#[pyclass(name = "Table", extends=PyEntry)]
#[derive(Default)]
pub struct PyTable {
    lazy_provider: Option<Arc<dyn TableProvider + Send>>,
}

#[pymethods]
impl PyTable {
    fn __datafusion_table_provider__<'py>(
        mut self_: PyRefMut<'py, Self>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let py = self_.py();
        if self_.lazy_provider.is_none() {
            let super_ = self_.as_mut();

            let id = super_.id.borrow(py).id;

            let connection = super_.client.borrow_mut(py).connection().clone();

            self_.lazy_provider = Some(
                wait_for_future(
                    py,
                    TableEntryTableProvider::new(connection.client(), id).into_provider(),
                )
                .map_err(|err| {
                    PyRuntimeError::new_err(format!("Error creating TableProvider: {err}"))
                })?,
            );
        }

        let provider = self_
            .lazy_provider
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Missing TableProvider".to_owned()))?
            .clone();

        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let provider = FFI_TableProvider::new(provider, false, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }

    fn df<'py>(self_: PyRef<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();

        let super_ = self_.as_super();
        let client = super_.client.borrow(py);
        let table_name = super_.name().clone();
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);

        drop(client);

        // We're fine with this failing.
        ctx.call_method1("deregister_table", (table_name.clone(),))?;

        ctx.call_method1("register_table_provider", (table_name.clone(), self_))?;

        let df = ctx.call_method1("table", (table_name,))?;

        Ok(df)
    }
}
