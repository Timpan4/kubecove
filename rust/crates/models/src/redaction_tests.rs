use super::*;

// Failure modes: a verbose plugin prints a token, an OAuth redirect or a
// header on stderr; escapes or bidirectional controls disguise the line;
// non-UTF-8 output panics; the chosen line is a trailing blank line or an
// escape sequence's continuation; formatting inside a secret defeats
// redaction; a quoted value keeps its words after the first; an empty
// stderr produces an empty message.

#[test]
fn plugin_line_is_the_last_non_blank_printable_line() {
    let stderr = b"Opening browser\n\x1b[31merror:\x1b[0m get-token failed\x1b]0;title\x07\n\n  \n";
    assert_eq!(
        plugin_error_line(stderr).as_deref(),
        Some("error: get-token failed")
    );
    assert_eq!(plugin_error_line(b""), None);
    assert_eq!(plugin_error_line(b"\n \n\t\n"), None);
    // An OSC title that spans a line, and the C1 forms of CSI and OSC.
    let stderr = "error: real failure\n\x1b]0;title\nstill title\x07\n\u{9b}2K\u{9d}0;t\u{9c}";
    assert_eq!(
        plugin_error_line(stderr.as_bytes()).as_deref(),
        Some("error: real failure")
    );
    assert_eq!(
        plugin_error_line(b"waiting 10%\rerror: timed out\r\n").as_deref(),
        Some("error: timed out")
    );
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
        (
            r#"error: password="first pw-trailing-secret""#.to_owned(),
            "pw-trailing-secret",
        ),
        (
            "client_secret: 'first secret-unterminated".to_owned(),
            "secret-unterminated",
        ),
        (
            "passphrase=pass-phrase-secret".to_owned(),
            "pass-phrase-secret",
        ),
        (
            "passphrase: colon-phrase-secret".to_owned(),
            "colon-phrase-secret",
        ),
        (
            "AWS_SECRET_ACCESS_KEY: aws-colon-secret".to_owned(),
            "aws-colon-secret",
        ),
        (
            r#"password="a\"b escaped-quote-secret""#.to_owned(),
            "escaped-quote-secret",
        ),
        (
            "AWS_SECRET_ACCESS_KEY=aws-secret-value".to_owned(),
            "aws-secret-value",
        ),
        (
            r#"{"sessionToken": "first aws-session-secret"}"#.to_owned(),
            "aws-session-secret",
        ),
        (
            "Author\u{202e}ization: Basic \x1b[1mdXNlcjpwYXNz\x1b[0m".to_owned(),
            "dXNlcjpwYXNz",
        ),
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
