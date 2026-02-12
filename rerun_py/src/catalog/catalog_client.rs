use std::collections::HashSet;
use std::sync::Arc;

use arrow::datatypes::Schema;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::{PyLookupError, PyRuntimeError, PyValueError};
use pyo3::types::PyAnyMethods as _;
use pyo3::{Py, PyAny, PyErr, PyResult, Python, pyclass, pymethods};
use re_datafusion::{DEFAULT_CATALOG_NAME, get_all_catalog_names};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};

use crate::catalog::datafusion_catalog::PyDataFusionCatalogProvider;
use crate::catalog::{
    ConnectionHandle, PyDatasetEntryInternal, PyEntryId, PyRerunHtmlTable, PyTableEntryInternal,
    to_py_err,
};
use crate::utils::{get_tokio_runtime, wait_for_future};

/// Client for a remote Rerun catalog server.
#[pyclass(  // NOLINT: ignore[py-cls-eq] non-trivial implementation
    name = "CatalogClientInternal",
    module = "rerun_bindings.rerun_bindings"
)]

pub struct PyCatalogClientInternal {
    origin: re_uri::Origin,

    connection: ConnectionHandle,

    // If this isn't set, it means datafusion wasn't found
    datafusion_ctx: Option<Py<PyAny>>,
}

impl PyCatalogClientInternal {
    pub fn connection(&self) -> &ConnectionHandle {
        &self.connection
    }
}

#[pymethods]
impl PyCatalogClientInternal {
    #[staticmethod]
    pub fn datafusion_major_version() -> u64 {
        datafusion_ffi::version()
    }

    /// Create a new catalog client object.
    #[new]
    #[pyo3(text_signature = "(self, url, token=None)")]
    fn new(py: Python<'_>, url: String, token: Option<String>) -> PyResult<Self> {
        // NOTE: The entire TLS stack expects this global variable to be set. It doesn't matter
        // what we set it to. But we have to set it, or we will crash at runtime, as soon as
        // anything tries to do anything TLS-related.
        // This used to be implicitly done by `object_store`, just by virtue of depending on it,
        // but we removed that unused dependency, so now we must do it ourselves.
        _ = rustls::crypto::ring::default_provider().install_default();

        let origin = url.as_str().parse::<re_uri::Origin>().map_err(to_py_err)?;

        let connection_registry =
            re_redap_client::ConnectionRegistry::new_with_stored_credentials();

        let credentials = match token
            .map(TryFrom::try_from)
            .transpose()
            .map_err(to_py_err)?
        {
            Some(token) => re_redap_client::Credentials::Token(token),
            None => re_redap_client::Credentials::Stored,
        };
        connection_registry.set_credentials(&origin, credentials);

        let connection = ConnectionHandle::new(connection_registry, origin.clone());

        let datafusion_ctx = py
            .import("datafusion")
            .and_then(|datafusion| Ok(datafusion.getattr("SessionContext")?.call0()?.unbind()))
            .ok();

        // Set up our renderer by default since we've already established datafusion
        // is installed.
        let html_renderer = PyRerunHtmlTable::new(None, None);

        let format_fn = py
            .import("datafusion")
            .and_then(|datafusion| datafusion.getattr("dataframe_formatter"))
            .and_then(|df_formatter| df_formatter.getattr("set_formatter"));

        if let Ok(format_fn) = format_fn {
            let _ = format_fn.call1((html_renderer,))?;
        }

        let ret = Self {
            origin,
            connection,
            datafusion_ctx,
        };

        ret.update_catalog_providers(py, true)?;

        Ok(ret)
    }

    /// Get the URL of the catalog (a `rerun+http` URL).
    #[getter]
    pub fn url(&self) -> String {
        self.origin.to_string()
    }

    /// Get a list of all dataset entries in the catalog.
    fn datasets(
        self_: Py<Self>,
        py: Python<'_>,
        include_hidden: bool,
    ) -> PyResult<Vec<Py<PyDatasetEntryInternal>>> {
        let connection = self_.borrow(py).connection.clone();

        let mut entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Dataset))?;

        if include_hidden {
            entry_details.extend(connection.find_entries(
                py,
                EntryFilter::new().with_entry_kind(EntryKind::BlueprintDataset),
            )?);
        }

        entry_details
            .into_iter()
            .map(|details| {
                let dataset_entry = connection.read_dataset(py, details.id)?;
                Py::new(
                    py,
                    PyDatasetEntryInternal::new(self_.clone_ref(py), dataset_entry),
                )
            })
            .collect()
    }

    /// Get a list of all table entries in the catalog.
    fn tables(
        self_: Py<Self>,
        py: Python<'_>,
        include_hidden: bool,
    ) -> PyResult<Vec<Py<PyTableEntryInternal>>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details =
            connection.find_entries(py, EntryFilter::new().with_entry_kind(EntryKind::Table))?;

        entry_details
            .into_iter()
            .filter(|details| !details.name.starts_with("__") || include_hidden)
            .map(|details| {
                let table_entry = connection.read_table(py, details.id)?;
                let table = PyTableEntryInternal::new(self_.clone_ref(py), table_entry);

                Py::new(py, table)
            })
            .collect()
    }

    // ---

    /// Get a dataset by name or id.
    fn get_dataset(
        self_: Py<Self>,
        id: Py<PyEntryId>,
        py: Python<'_>,
    ) -> PyResult<Py<PyDatasetEntryInternal>> {
        let connection = self_.borrow(py).connection.clone();
        let dataset_entry = connection.read_dataset(py, id.borrow(py).id)?;

        Py::new(
            py,
            PyDatasetEntryInternal::new(self_.clone_ref(py), dataset_entry),
        )
    }

    /// Get a table by name or id.
    ///
    /// Note: the entry table is named `__entries`.
    fn get_table(
        self_: Py<Self>,
        py: Python<'_>,
        id: Py<PyEntryId>,
    ) -> PyResult<Py<PyTableEntryInternal>> {
        let connection = self_.borrow(py).connection.clone();
        let table_entry = connection.read_table(py, id.borrow(py).id)?;

        Py::new(
            py,
            PyTableEntryInternal::new(self_.clone_ref(py), table_entry),
        )
    }

    // ---

    /// Create a new dataset with the provided name.
    fn create_dataset(
        self_: Py<Self>,
        py: Python<'_>,
        name: &str,
    ) -> PyResult<Py<PyDatasetEntryInternal>> {
        let connection = self_.borrow_mut(py).connection.clone();
        let dataset_entry = connection.create_dataset(py, name.to_owned())?;

        Py::new(
            py,
            PyDatasetEntryInternal::new(self_.clone_ref(py), dataset_entry),
        )
    }

    fn register_table(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
        url: String,
    ) -> PyResult<Py<PyTableEntryInternal>> {
        let connection = self_.borrow_mut(py).connection.clone();

        let url = url
            .parse::<url::Url>()
            .map_err(|err| PyValueError::new_err(format!("Invalid URL: {err}")))?;

        let table_entry = connection.register_table(py, name, url)?;

        self_.borrow(py).update_catalog_providers(py, false)?;

        Py::new(
            py,
            PyTableEntryInternal::new(self_.clone_ref(py), table_entry),
        )
    }

    /// Create a table entry.
    ///
    /// NOTE: when provided, `url` is a _prefix_ for the table location, and we must ensure that
    /// the actual url is unique by inserting a UUID in the path. This is different from the
    /// semantics of the layers below ([`re_redap_client::ConnectionClient::create_table_entry`] and
    /// redap), which expect a full url that we must guarantee is free to use.
    fn create_table(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
        schema: PyArrowType<Schema>,
        url: Option<String>,
    ) -> PyResult<Py<PyTableEntryInternal>> {
        let connection = self_.borrow_mut(py).connection.clone();

        // Verify we have a valid table name
        let dialect = datafusion::logical_expr::sqlparser::dialect::GenericDialect;
        let _ = datafusion::logical_expr::sqlparser::parser::Parser::new(&dialect)
            .try_with_sql(name.as_str())
            .and_then(|mut parser| parser.parse_multipart_identifier())
            .map_err(|err| PyValueError::new_err(format!("Invalid table name. {err}")))?;

        let url = url
            .map(|url| {
                let mut url = url
                    .parse::<url::Url>()
                    .map_err(|err| PyValueError::new_err(format!("Invalid URL: {err}")))?;

                if url.cannot_be_a_base() {
                    return Err(PyValueError::new_err(format!(
                        "URL cannot be a base: {url}"
                    )));
                }
                url.path_segments_mut()
                    .expect("just checked with cannot_be_a_base()")
                    .push(&re_tuid::Tuid::new().to_string());
                Ok::<_, PyErr>(url)
            })
            .transpose()?;

        let schema = Arc::new(schema.0);
        let table_entry = connection.create_table_entry(py, name, schema, url)?;

        self_.borrow(py).update_catalog_providers(py, false)?;

        Py::new(
            py,
            PyTableEntryInternal::new(self_.clone_ref(py), table_entry),
        )
    }

    // ---

    /// Perform global maintenance tasks on the server.
    fn do_global_maintenance(self_: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let connection = self_.borrow_mut(py).connection.clone();

        connection.do_global_maintenance(py)
    }

    // ---

    /// The DataFusion context (if available).
    pub fn ctx(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(datafusion_ctx) = &self.datafusion_ctx {
            Ok(datafusion_ctx.clone_ref(py))
        } else {
            Err(PyRuntimeError::new_err(
                "DataFusion context not available (the `datafusion` package may need to be installed)".to_owned(),
            ))
        }
    }

    fn __repr__(&self) -> String {
        format!("CatalogClient({})", self.origin)
    }

    // ---

    fn _entry_id_from_entry_name(
        self_: Py<Self>,
        py: Python<'_>,
        name: String,
    ) -> PyResult<Py<PyEntryId>> {
        let connection = self_.borrow(py).connection.clone();

        let entry_details = connection.find_entries(py, EntryFilter::new().with_name(&name))?;

        if entry_details.is_empty() {
            return Err(PyLookupError::new_err(format!(
                "No entry found with name {name:?}"
            )));
        }

        Py::new(py, PyEntryId::from(entry_details[0].id))
    }
}

impl PyCatalogClientInternal {
    fn update_catalog_providers(&self, py: Python<'_>, force_register: bool) -> Result<(), PyErr> {
        let client = wait_for_future(py, self.connection.client())?;
        let runtime = get_tokio_runtime().handle();

        let provider_names = get_all_catalog_names(&client, runtime).map_err(to_py_err)?;
        let mut providers = provider_names
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>();
        if !providers.contains(&DEFAULT_CATALOG_NAME) {
            providers.push(DEFAULT_CATALOG_NAME);
        }

        if let Some(ctx) = self.datafusion_ctx.as_ref() {
            let existing_catalogs: HashSet<String> =
                ctx.call_method0(py, "catalog_names")?.extract(py)?;

            for provider_name in providers {
                if !force_register && existing_catalogs.contains(provider_name) {
                    continue;
                }

                let catalog_provider = PyDataFusionCatalogProvider::new(
                    Some(provider_name.to_owned()),
                    client.clone(),
                );

                ctx.call_method1(
                    py,
                    "register_catalog_provider",
                    (provider_name, catalog_provider),
                )?;
            }
        }

        Ok(())
    }
}
