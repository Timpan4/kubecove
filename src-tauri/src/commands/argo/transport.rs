use super::connected::ConnectedArgo;
use crate::models::AppError;
use futures_util::StreamExt;
use reqwest::{Certificate, Client as HttpClient};
use serde_json::Value;
use std::time::Duration;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

pub(crate) async fn api_get(
    connection: &ConnectedArgo,
    path: &str,
) -> Result<Value, AppError> {
    let response = connection
        .client
        .get(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}

pub(crate) async fn api_post(
    connection: &ConnectedArgo,
    path: &str,
    payload: Value,
) -> Result<Value, AppError> {
    let response = connection
        .client
        .post(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .json(&payload)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}

pub(crate) async fn api_delete(
    connection: &ConnectedArgo,
    path: &str,
) -> Result<Value, AppError> {
    let response = connection
        .client
        .delete(url(&connection.profile.url, path)?)
        .bearer_auth(&connection.token)
        .send()
        .await
        .map_err(safe_http_error)?;
    if !response.status().is_success() {
        return Err(api_status_error(response).await);
    }
    response_json(response).await
}

pub(super) async fn response_json(response: reqwest::Response) -> Result<Value, AppError> {
    let bytes = capped_response_bytes(response).await?;
    serde_json::from_slice(&bytes).map_err(|_| AppError::new("invalid Argo CD response", "argoApi"))
}

async fn api_status_error(response: reqwest::Response) -> AppError {
    let status = response.status();
    let body = capped_response_bytes(response).await.unwrap_or_default();
    let body = &body[..body.len().min(4096)];
    let detail = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .or_else(|| value.get("error"))
                .and_then(Value::as_str)
                .map(safe_message)
        });
    AppError::new(
        detail.map_or_else(
            || format!("Argo CD API request failed ({status})"),
            |message| format!("Argo CD API request failed ({status}): {message}"),
        ),
        "argoApi",
    )
}

async fn capped_response_bytes(response: reqwest::Response) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::new(
            "Argo CD response exceeded size limit",
            "argoApi",
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        push_capped_chunk(&mut bytes, &chunk.map_err(safe_http_error)?)?;
    }
    Ok(bytes)
}

fn push_capped_chunk(bytes: &mut Vec<u8>, chunk: &[u8]) -> Result<(), AppError> {
    if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
        return Err(AppError::new(
            "Argo CD response exceeded size limit",
            "argoApi",
        ));
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}

fn safe_message(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(240)
        .collect()
}

pub(crate) fn redact_secret_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let is_secret = map.get("kind").and_then(Value::as_str) == Some("Secret");
            if is_secret {
                for key in ["data", "stringData"] {
                    if let Some(Value::Object(data)) = map.get_mut(key) {
                        for item in data.values_mut() {
                            *item = Value::String("[REDACTED]".into());
                        }
                    }
                }
            }
            for child in map.values_mut() {
                redact_secret_fields(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                redact_secret_fields(child);
            }
        }
        _ => {}
    }
}

pub(super) fn safe_http_error(error: reqwest::Error) -> AppError {
    AppError::new(
        format!("Argo CD request failed: {}", error.without_url()),
        "argoConnection",
    )
}

pub(super) fn http_client(
    insecure_tls: bool,
    custom_ca_pem: Option<Vec<u8>>,
) -> Result<HttpClient, AppError> {
    let mut builder = HttpClient::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8));
    if insecure_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(pem) = custom_ca_pem {
        let certificate = Certificate::from_pem(&pem)
            .map_err(|_| AppError::new("invalid custom CA", "argoConnection"))?;
        builder = builder.add_root_certificate(certificate);
    }
    builder.build().map_err(safe_http_error)
}

pub(super) fn url(base: &str, path: &str) -> Result<String, AppError> {
    let mut base = canonical_url(base)?;
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(path.trim_start_matches('/'))
        .map(|value| value.to_string())
        .map_err(|_| AppError::new("invalid Argo CD API path", "argoConnection"))
}

pub(super) fn canonical_url(value: &str) -> Result<reqwest::Url, AppError> {
    let mut base = reqwest::Url::parse(value.trim())
        .map_err(|_| AppError::new("invalid Argo CD URL", "argoConnection"))?;
    if base.scheme() != "https" {
        return Err(AppError::new(
            "Argo CD URL must use HTTPS",
            "argoConnection",
        ));
    }
    if !base.username().is_empty() || base.password().is_some() {
        return Err(AppError::new(
            "Argo CD URL must not contain credentials",
            "argoConnection",
        ));
    }
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
}
