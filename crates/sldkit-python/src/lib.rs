use pyo3::{
    Bound, PyResult, Python,
    exceptions::{PyRuntimeError, PyValueError},
    pyfunction, pymodule,
    types::{PyBytes, PyModule, PyModuleMethods},
    wrap_pyfunction,
};
use serde::Serialize;
use sldkit_core::{ExtractionMode, LimitProfile, ResourceLimits, StreamExtraction};

#[pyfunction]
fn probe_bytes_json(data: &[u8], profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::probe_bytes(
        data,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn probe_file_json(path: &str, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::probe_path(
        path,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn inspect_bytes_json(data: &[u8], filename: Option<&str>, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::inspect_bytes(
        data,
        filename,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn inspect_file_json(path: &str, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::inspect_path(
        path,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn parse_bytes_json(data: &[u8], filename: Option<&str>, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::parse_bytes(
        data,
        filename,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn parse_file_json(path: &str, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::parse_path(
        path,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn decode_geometry_bytes_json(
    data: &[u8],
    filename: Option<&str>,
    profile: &str,
) -> PyResult<String> {
    to_json(&sldkit_parser::decode_geometry_bytes(
        data,
        filename,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn decode_geometry_file_json(path: &str, profile: &str) -> PyResult<String> {
    to_json(&sldkit_parser::decode_geometry_path(
        path,
        &limits_for_profile(profile)?,
    ))
}

#[allow(clippy::too_many_arguments)]
#[pyfunction]
fn scan_project_json(
    path: &str,
    project_root: Option<&str>,
    configuration: Option<&str>,
    search_directories: Vec<String>,
    windows_prefix_mappings: Vec<(String, String)>,
    follow_suppressed: bool,
    profile: &str,
) -> PyResult<String> {
    let options = sldkit_parser::ProjectScanOptions {
        project_root: project_root.map(Into::into),
        root_configuration: configuration.map(str::to_owned),
        search_directories: search_directories.into_iter().map(Into::into).collect(),
        windows_prefix_mappings: windows_prefix_mappings
            .into_iter()
            .map(
                |(source_prefix, target_directory)| sldkit_parser::WindowsPrefixMapping {
                    source_prefix,
                    target_directory: target_directory.into(),
                },
            )
            .collect(),
        follow_suppressed,
    };
    to_json(&sldkit_parser::scan_project_path(
        path,
        &options,
        &limits_for_profile(profile)?,
    ))
}

#[pyfunction]
fn extract_bytes_result<'py>(
    py: Python<'py>,
    data: &[u8],
    entry_id: &str,
    mode: &str,
    profile: &str,
) -> PyResult<(String, Option<Bound<'py, PyBytes>>)> {
    extraction_to_python(
        py,
        sldkit_parser::extract_bytes(
            data,
            entry_id,
            extraction_mode(mode)?,
            &limits_for_profile(profile)?,
        ),
    )
}

#[pyfunction]
fn extract_file_result<'py>(
    py: Python<'py>,
    path: &str,
    entry_id: &str,
    mode: &str,
    profile: &str,
) -> PyResult<(String, Option<Bound<'py, PyBytes>>)> {
    extraction_to_python(
        py,
        sldkit_parser::extract_path(
            path,
            entry_id,
            extraction_mode(mode)?,
            &limits_for_profile(profile)?,
        ),
    )
}

fn extraction_mode(mode: &str) -> PyResult<ExtractionMode> {
    match mode {
        "stored" => Ok(ExtractionMode::Stored),
        "decoded" => Ok(ExtractionMode::Decoded),
        _ => Err(PyValueError::new_err(format!(
            "unknown extraction mode {mode:?}; expected 'stored' or 'decoded'"
        ))),
    }
}

fn extraction_to_python(
    py: Python<'_>,
    extraction: StreamExtraction,
) -> PyResult<(String, Option<Bound<'_, PyBytes>>)> {
    let StreamExtraction { result, data } = extraction;
    let metadata = to_json(&result)?;
    let data = data.as_deref().map(|payload| PyBytes::new(py, payload));
    Ok((metadata, data))
}

fn limits_for_profile(profile: &str) -> PyResult<ResourceLimits> {
    match profile {
        "desktop" => Ok(LimitProfile::Desktop.limits()),
        "service" => Ok(LimitProfile::Service.limits()),
        _ => Err(PyValueError::new_err(format!(
            "unknown resource-limit profile {profile:?}; expected 'desktop' or 'service'"
        ))),
    }
}

fn to_json(value: &impl Serialize) -> PyResult<String> {
    serde_json::to_string(value)
        .map_err(|error| PyRuntimeError::new_err(format!("result serialization failed: {error}")))
}

#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(probe_bytes_json, module)?)?;
    module.add_function(wrap_pyfunction!(probe_file_json, module)?)?;
    module.add_function(wrap_pyfunction!(inspect_bytes_json, module)?)?;
    module.add_function(wrap_pyfunction!(inspect_file_json, module)?)?;
    module.add_function(wrap_pyfunction!(parse_bytes_json, module)?)?;
    module.add_function(wrap_pyfunction!(parse_file_json, module)?)?;
    module.add_function(wrap_pyfunction!(decode_geometry_bytes_json, module)?)?;
    module.add_function(wrap_pyfunction!(decode_geometry_file_json, module)?)?;
    module.add_function(wrap_pyfunction!(scan_project_json, module)?)?;
    module.add_function(wrap_pyfunction!(extract_bytes_result, module)?)?;
    module.add_function(wrap_pyfunction!(extract_file_result, module)?)?;
    Ok(())
}
