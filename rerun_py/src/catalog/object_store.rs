use std::error::Error;
use std::sync::Arc;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use re_datafusion::ObjectStoreAuthenticator;

#[derive(FromPyObject)]
pub enum AnyObjectStoreAuthenticator {
    BearerToken(Py<BearerTokenObjectStoreAuthenticator>),
}

impl AnyObjectStoreAuthenticator {
    pub fn into_authenticator(self, py: Python<'_>) -> Arc<dyn ObjectStoreAuthenticator> {
        match self {
            Self::BearerToken(auth) => Arc::new(auth.get().clone_ref(py)),
        }
    }
}

/// Authenticates object store fetches with an `Authorization: Bearer` header.
#[pyclass( // NOLINT: ignore[py-cls-eq]
    name = "BearerTokenObjectStoreAuth",
    module = "rerun_bindings.rerun_bindings",
    frozen
)]
pub struct BearerTokenObjectStoreAuthenticator {
    get_token: Py<PyAny>,
}

#[pymethods]
impl BearerTokenObjectStoreAuthenticator {
    /// Create an authenticator from a callable returning the current bearer token.
    ///
    /// `get_token` is invoked during queries and must return a `str`.
    #[new]
    #[pyo3(text_signature = "(self, get_token)")]
    fn new(py: Python<'_>, get_token: Py<PyAny>) -> PyResult<Self> {
        if !get_token.bind(py).is_callable() {
            return Err(PyTypeError::new_err("get_token must be callable"));
        }
        Ok(Self { get_token })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let get_token = self.get_token.bind(py).repr()?;
        Ok(format!("BearerTokenObjectStoreAuth(get_token={get_token})"))
    }
}

impl BearerTokenObjectStoreAuthenticator {
    fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            get_token: self.get_token.clone_ref(py),
        }
    }

    fn token(&self) -> Result<String, Box<dyn Error>> {
        Python::attach(|py| {
            self.get_token
                .call0(py)
                .and_then(|token| token.extract::<String>(py))
                .map_err(|err| Box::new(err) as Box<dyn Error>)
        })
    }
}

impl std::fmt::Debug for BearerTokenObjectStoreAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BearerTokenObjectStoreAuthenticator")
            .finish_non_exhaustive()
    }
}

impl ObjectStoreAuthenticator for BearerTokenObjectStoreAuthenticator {
    fn authenticate_requests(
        &self,
        requests: &mut dyn Iterator<Item = &mut reqwest::Request>,
    ) -> Result<(), Box<dyn Error>> {
        let token = self.token()?;
        let mut header_value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?;
        header_value.set_sensitive(true);

        for request in requests {
            request
                .headers_mut()
                .insert(reqwest::header::AUTHORIZATION, header_value.clone());
        }

        Ok(())
    }
}
