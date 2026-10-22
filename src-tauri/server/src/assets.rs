use crate::{index_with_policy, ApiError, AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use kubecove_lib::models::AppErrorKind;
use rust_embed::Embed;
use std::sync::Arc;

#[derive(Embed)]
#[folder = "../../dist/"]
pub(super) struct Assets;

struct CompressedAsset {
    gzip: Option<&'static [u8]>,
    brotli: Option<&'static [u8]>,
}

include!(concat!(env!("OUT_DIR"), "/compressed_assets.rs"));

pub(super) async fn asset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    if uri.path().starts_with("/api/") {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "unknown API route",
            AppErrorKind::UnsupportedOperation,
        )
        .into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let Some(asset) = Assets::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if path == "index.html" {
        let host = headers
            .get(header::HOST)
            .and_then(|host| host.to_str().ok())
            .expect("validated Host");
        let policy = state.policy.content_security_policy(host);
        let html = std::str::from_utf8(&asset.data)
            .ok()
            .and_then(|html| index_with_policy(html, &policy));
        return html.map_or_else(
            || StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            |html| ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        );
    }
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();
    let Some(compressed) = compressed_asset(path) else {
        return ([(header::CONTENT_TYPE, mime)], asset.data.into_owned()).into_response();
    };
    let encoding = negotiate(&headers, &compressed);
    let mut response = match encoding {
        "br" => compressed
            .brotli
            .expect("negotiated Brotli")
            .into_response(),
        "gzip" => compressed.gzip.expect("negotiated gzip").into_response(),
        _ => asset.data.into_owned().into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, mime.parse().expect("MIME header"));
    headers.insert(
        header::VARY,
        "Accept-Encoding".parse().expect("static header"),
    );
    // One year, the conventional maximum for immutable assets (RFC 9111; RFC 2616 §14.21).
    headers.insert(
        header::CACHE_CONTROL,
        "max-age=31536000, immutable"
            .parse()
            .expect("static header"),
    );
    if encoding != "identity" {
        headers.insert(
            header::CONTENT_ENCODING,
            encoding.parse().expect("static encoding"),
        );
    }
    response
}

fn negotiate(headers: &HeaderMap, asset: &CompressedAsset) -> &'static str {
    let mut br = None;
    let mut gzip = None;
    let mut identity = None;
    let mut wildcard = None;
    for value in headers.get_all(header::ACCEPT_ENCODING) {
        for item in value.to_str().unwrap_or("").split(',') {
            let mut parts = item.split(';');
            let name = parts.next().unwrap_or("").trim();
            let mut quality = 1.0_f32;
            for parameter in parts {
                if let Some((key, value)) = parameter.trim().split_once('=') {
                    if key.eq_ignore_ascii_case("q") {
                        quality = value
                            .parse::<f32>()
                            .ok()
                            .filter(|q| (0.0..=1.0).contains(q))
                            .unwrap_or(0.0);
                    }
                }
            }
            if name.eq_ignore_ascii_case("br") {
                br = Some(quality);
            } else if name.eq_ignore_ascii_case("gzip") {
                gzip = Some(quality);
            } else if name.eq_ignore_ascii_case("identity") {
                identity = Some(quality);
            } else if name == "*" {
                wildcard = Some(quality);
            }
        }
    }
    let mut selected = None;
    let mut quality = 0.0;
    // Prefer Brotli at equal quality; preserve the legacy identity fallback.
    for (encoding, available, q) in [
        ("br", asset.brotli.is_some(), br.or(wildcard).unwrap_or(0.0)),
        (
            "gzip",
            asset.gzip.is_some(),
            gzip.or(wildcard).unwrap_or(0.0),
        ),
        ("identity", true, identity.unwrap_or(0.0)),
    ] {
        if available && q > quality {
            selected = Some(encoding);
            quality = q;
        }
    }
    selected.unwrap_or("identity")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_without_smaller_variants_fall_back_to_identity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT_ENCODING,
            "identity;q=0, br, gzip".parse().unwrap(),
        );
        let asset = CompressedAsset {
            gzip: None,
            brotli: None,
        };
        assert_eq!(negotiate(&headers, &asset), "identity");
    }
}
