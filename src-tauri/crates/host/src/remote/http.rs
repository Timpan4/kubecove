//! URL validation and response decoding shared by the remote client and its sign-in.
use kubecove_models::{AppError, AppErrorKind};
use reqwest::Url;
use serde_json::Value;

pub(super) fn normalize_server_url(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let url = Url::parse(value).map_err(|error| {
        AppError::new(
            "server URL must be a valid HTTP or HTTPS URL",
            AppErrorKind::Validation,
        )
        .with_source(error)
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(validation_error(
            "server URL must use HTTP or HTTPS and cannot include credentials, query, or fragment",
        ));
    }
    if url.scheme() == "http" {
        let host = url.host_str().expect("host validated above");
        let is_loopback = host == "localhost"
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if !is_loopback {
            return Err(validation_error("non-loopback server URLs require HTTPS"));
        }
    }
    Ok(value.trim_end_matches('/').to_string())
}

pub(super) fn api_url(server_url: &str, path: &str) -> Result<Url, AppError> {
    Url::parse(&format!(
        "{}/api/v1/{path}",
        server_url.trim_end_matches('/')
    ))
    .map_err(|error| {
        AppError::new("server URL is invalid", AppErrorKind::Validation).with_source(error)
    })
}

pub(super) async fn response_value(response: reqwest::Response) -> Result<Value, AppError> {
    let status = response.status();
    let value = response.json::<Value>().await.map_err(network_error)?;
    if status.is_success() {
        return Ok(value);
    }
    let error = serde_json::from_value::<AppError>(value).unwrap_or_else(|_| {
        AppError::new(
            "remote server returned an invalid error response",
            AppErrorKind::Transport,
        )
    });
    Err(error)
}

pub(super) async fn response_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, AppError> {
    let status = response.status();
    if !status.is_success() {
        let error = response.json::<AppError>().await.unwrap_or_else(|_| {
            AppError::new(
                "remote server rejected the connection request",
                AppErrorKind::Transport,
            )
        });
        return Err(error);
    }
    response.json::<T>().await.map_err(network_error)
}

pub(super) fn install_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub(super) fn network_error(error: impl std::error::Error + Send + Sync + 'static) -> AppError {
    AppError::new(
        "remote server network request failed",
        AppErrorKind::Network,
    )
    .with_source(error)
}

pub(super) fn validation_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Validation)
}

pub(super) fn session_error(message: &str) -> AppError {
    AppError::new(message, AppErrorKind::Session)
}

/// No redirects: a redirect from a sign-in endpoint is never followed here.
pub(super) fn http_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(network_error)
}
