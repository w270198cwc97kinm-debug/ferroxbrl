//! Python bindings for ferroxbrl.
//!
//! ```python
//! import ferroxbrl
//! doc = ferroxbrl.extract("filing.htm")     # path or raw iXBRL string
//! doc["fact_count"], doc["facts"][0]
//! ```

use ferroxbrl_core::Document;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;

/// Convert a serde_json::Value into native Python objects.
fn json_to_py<'py>(py: Python<'py>, v: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        Value::Null => py.None().into_bound(py),
        Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py)?.into_any()
            } else {
                n.as_f64().unwrap_or(f64::NAN).into_pyobject(py)?.into_any()
            }
        }
        Value::String(s) => s.into_pyobject(py)?.into_any(),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_to_py(py, item)?)?;
            }
            list.into_any()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, val) in map {
                dict.set_item(k, json_to_py(py, val)?)?;
            }
            dict.into_any()
        }
    })
}

fn document_to_py<'py>(py: Python<'py>, doc: &Document, source: Option<&str>) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    if let Some(s) = source {
        dict.set_item("source", s)?;
    }
    dict.set_item("fact_count", doc.facts.len())?;
    dict.set_item("context_count", doc.contexts.len())?;
    dict.set_item("unit_count", doc.units.len())?;
    dict.set_item("warnings", &doc.warnings)?;
    let facts = PyList::empty(py);
    for f in doc.flat_facts() {
        facts.append(json_to_py(py, &f)?)?;
    }
    dict.set_item("facts", facts)?;
    Ok(dict)
}

/// Extract facts from an inline XBRL document.
///
/// `src` is either a filesystem path to an .htm/.xml file or a raw iXBRL
/// string. Returns a dict with keys: fact_count, context_count, unit_count,
/// warnings, facts (list of dicts).
#[pyfunction]
fn extract<'py>(py: Python<'py>, src: &str) -> PyResult<Bound<'py, PyDict>> {
    let path = std::path::Path::new(src);
    let doc = if path.exists() {
        ferroxbrl_core::extract_path(path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?
    } else if src.contains("<html") || src.contains("<?xml") || src.contains("ix:") {
        ferroxbrl_core::extract_str(src)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?
    } else {
        return Err(PyErr::new::<pyo3::exceptions::PyFileNotFoundError, _>(format!(
            "no such file and input is not iXBRL markup: {src}"
        )));
    };
    let source = if path.exists() { Some(src) } else { None };
    document_to_py(py, &doc, source)
}

/// Extract just the list of facts (convenience wrapper).
#[pyfunction]
fn extract_facts<'py>(py: Python<'py>, src: &str) -> PyResult<Bound<'py, PyList>> {
    let doc = extract(py, src)?;
    let facts = doc.get_item("facts")?.unwrap();
    Ok(facts.downcast_into::<PyList>()?)
}

#[pymodule]
fn ferroxbrl(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(extract, m)?)?;
    m.add_function(wrap_pyfunction!(extract_facts, m)?)?;
    Ok(())
}
