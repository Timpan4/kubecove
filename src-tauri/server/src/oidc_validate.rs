//! Validation of operator configuration and provider discovery metadata.
use openidconnect::core::CoreProviderMetadata;

pub(super) fn authority_has_userinfo(value: &str) -> bool {
    value
        .split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|authority| authority.contains('@'))
}

pub(super) fn validate_issuer(issuer: &str) -> Result<(), String> {
    let url = openidconnect::url::Url::parse(issuer)
        .map_err(|_| "OIDC issuer must be a valid HTTPS URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || authority_has_userinfo(issuer)
    {
        return Err("OIDC issuer must be HTTPS without credentials, query, or fragment".into());
    }
    Ok(())
}

pub(crate) fn validate_public_origin(origin: &str) -> Result<String, String> {
    let url = openidconnect::url::Url::parse(origin)
        .map_err(|_| "OIDC public origin must be an exact HTTPS origin")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || authority_has_userinfo(origin)
    {
        return Err("OIDC public origin must be an exact HTTPS origin".into());
    }
    Ok(format!(
        "https://{}{}",
        url.host_str().unwrap_or_default(),
        url.port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default()
    ))
}

#[cfg(test)]
pub(super) fn test_origin(origin: &str) -> Result<String, String> {
    if origin.starts_with("https://") {
        return validate_public_origin(origin);
    }
    crate::oidc::mock::test_loopback_origin(origin)
}

pub(super) fn validate_metadata(
    issuer: &str,
    metadata: &CoreProviderMetadata,
    allow_loopback_http: bool,
) -> Result<(), String> {
    if metadata.issuer().as_str() != issuer {
        return Err("OIDC provider issuer does not match configured issuer".into());
    }
    let token_endpoint = metadata
        .token_endpoint()
        .ok_or("OIDC provider omitted its token endpoint")?;
    let endpoints = [
        metadata.authorization_endpoint().url(),
        token_endpoint.url(),
        metadata.jwks_uri().url(),
    ];
    for endpoint in endpoints {
        let url = openidconnect::url::Url::parse(endpoint.as_str())
            .map_err(|_| "invalid OIDC metadata endpoint")?;
        let local_http = allow_loopback_http
            && url.scheme() == "http"
            && url.host_str().is_some_and(|host| {
                host == "localhost"
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            });
        if (url.scheme() != "https" && !local_http) || url.host_str().is_none() {
            return Err("OIDC metadata endpoints must use HTTPS".into());
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_metadata(issuer: &str) -> Result<CoreProviderMetadata, String> {
    serde_json::from_value(serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/auth"),
        "token_endpoint": format!("{issuer}/token"),
        "jwks_uri": format!("{issuer}/jwks"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"]
    }))
    .map_err(|_| "invalid test provider metadata".into())
}
