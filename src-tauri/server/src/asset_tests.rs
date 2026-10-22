use crate::{api_tests::state, assets::Assets, router};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use std::io::Read;
use tower::ServiceExt;

// Failure modes: compressed bytes mismatch the original, ignored q=0 or quality
// preference, missing Vary, public caching behind auth, cached HTML/API/errors,
// rejecting identity when no acceptable compressed variant exists, or auth bypass.
#[tokio::test]
async fn hashed_assets_negotiate_precompressed_bytes_and_preserve_guards() {
    let path = Assets::iter()
        .find(|path| path.starts_with("assets/") && path.ends_with(".js"))
        .expect("web build has hashed JavaScript");
    let original = Assets::get(&path).unwrap();
    let app = router(state());
    for (accept, encoding) in [
        ("br, gzip", Some("br")),
        ("gzip", Some("gzip")),
        ("br;q=0, gzip;q=0.5", Some("gzip")),
        ("br;q=0.1, gzip;q=0.9", Some("gzip")),
        ("br;q=0, gzip;q=0", None),
        ("identity;q=1, br;q=0.5", None),
        ("*;q=0.5, br;q=0", Some("gzip")),
        ("br;q=0, gzip;q=0, identity;q=0", None),
        ("*;q=0", None),
        ("", None),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/{path}"))
                    .header(header::HOST, "localhost:1234")
                    .header(header::ACCEPT_ENCODING, accept)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{accept}");
        assert_eq!(response.headers()[header::VARY], "Accept-Encoding");
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "max-age=31536000, immutable"
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .map(|value| value.to_str().unwrap()),
            encoding,
            "{accept}"
        );
        assert!(response
            .headers()
            .contains_key(header::CONTENT_SECURITY_POLICY));
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let mut decoded = Vec::new();
        match encoding {
            Some("gzip") => {
                flate2::read::GzDecoder::new(&bytes[..])
                    .read_to_end(&mut decoded)
                    .unwrap();
            }
            Some("br") => {
                brotli::BrotliDecompress(&mut &bytes[..], &mut decoded).unwrap();
            }
            _ => decoded.extend_from_slice(&bytes),
        }
        assert_eq!(decoded, original.data.as_ref(), "{accept}");
    }
    let mut access = state();
    std::sync::Arc::get_mut(&mut access).unwrap().access =
        Some(std::sync::Arc::new(crate::access::tests::access()));
    let response = router(access)
        .oneshot(
            Request::builder()
                .uri(format!("/{path}"))
                .header(header::HOST, "localhost:1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn html_api_and_missing_assets_are_never_immutable_or_compressed() {
    let app = router(state());
    for path in [
        "/",
        "/index.html",
        "/api/v1/auth",
        "/api/missing",
        "/assets/missing.js",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::HOST, "localhost:1234")
                    .header(header::ACCEPT_ENCODING, "br, gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-store",
            "{path}"
        );
        assert!(
            !response.headers().contains_key(header::CONTENT_ENCODING),
            "{path}"
        );
    }
}
