//! Simple HTTP client for registry queries.

use crate::PackageError;

/// Perform a GET request and return the response body as a string.
pub fn get(url: &str) -> Result<String, PackageError> {
    get_with_headers(url, &[])
}

/// Perform a GET request with custom headers.
pub fn get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String, PackageError> {
    let mut request = ureq::get(url);
    for (key, value) in headers {
        request = request.set(key, value);
    }

    let response = request.call().map_err(|e| match e {
        ureq::Error::Status(404, _) => PackageError::NotFound(url.to_string()),
        ureq::Error::Status(code, _) => PackageError::RegistryError(format!("HTTP {}", code)),
        ureq::Error::Transport(t) => PackageError::RegistryError(t.to_string()),
    })?;

    response
        .into_string()
        .map_err(|e| PackageError::ParseError(format!("failed to read response: {}", e)))
}

/// Perform a GET request and return the response body as raw bytes.
///
/// Mirrors [`get`] but reads the body via `into_reader().read_to_end()`, suitable
/// for downloading binary artifacts (source archives, tarballs).
pub fn get_bytes(url: &str) -> Result<Vec<u8>, PackageError> {
    use std::io::Read;

    let response = ureq::get(url).call().map_err(|e| match e {
        ureq::Error::Status(404, _) => PackageError::NotFound(url.to_string()),
        ureq::Error::Status(code, _) => PackageError::RegistryError(format!("HTTP {}", code)),
        ureq::Error::Transport(t) => PackageError::RegistryError(t.to_string()),
    })?;

    let mut data = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut data)
        .map_err(|e| PackageError::ParseError(format!("failed to read response: {}", e)))?;
    Ok(data)
}
