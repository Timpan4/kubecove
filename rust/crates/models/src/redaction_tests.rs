use super::*;

// Failure modes: a verbose plugin prints a token, an OAuth redirect or a
// header on stderr; escapes or bidirectional controls disguise the line;
// non-UTF-8 output panics; the chosen line is a trailing blank or progress
// line rather than the error; an empty stderr produces an empty message.

#[test]
fn plugin_line_is_the_last_printable_error_line() {
    let stderr = b"Opening browser\n\x1b[31merror:\x1b[0m get-token failed\x1b]0;title\x07\n\n  \n";
    assert_eq!(
        plugin_error_line(stderr).as_deref(),
        Some("error: get-token failed")
    );
    assert_eq!(plugin_error_line(b""), None);
    assert_eq!(plugin_error_line(b"\n \n\t\n"), None);
}

#[test]
fn plugin_line_removes_control_and_bidi_characters_and_survives_invalid_utf8() {
    let line = plugin_error_line(b"error \xe2\x80\xaeevil\xe2\x80\xac\x07 \xff done").unwrap();
    assert_eq!(line, "error evil \u{fffd} done");
}

#[test]
fn plugin_line_redacts_tokens_redirects_and_headers() {
    let jwt = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.c2lnbmF0dXJl";
    for (stderr, secret) in [
        (format!("got id token {jwt}"), "eyJ"),
        (
            "callback http://localhost:8000/?code=auth-code-secret&state=state-secret".to_owned(),
            "secret",
        ),
        (
            "request Authorization: Bearer opaque-bearer-secret".to_owned(),
            "opaque-bearer-secret",
        ),
        ("refresh_token=refresh-secret".to_owned(), "refresh-secret"),
        (
            r#"{"token":"json-token-secret"}"#.to_owned(),
            "json-token-secret",
        ),
        ("password: pw-secret".to_owned(), "pw-secret"),
    ] {
        let line = plugin_error_line(stderr.as_bytes()).unwrap();
        assert!(!line.contains(secret), "{line}");
        assert!(line.contains("[REDACTED]"), "{line}");
    }
}

#[test]
fn plugin_line_keeps_issuer_urls_without_queries() {
    let line = plugin_error_line(
        br#"oidc: issuer URL provided to client ("https://old.example/realms/a") did not match"#,
    )
    .unwrap();
    assert!(line.contains("https://old.example/realms/a"), "{line}");
}
