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
        (
            "request Cookie: _oauth2_proxy=cookie-header-secret; other=1".to_owned(),
            "cookie-header-secret",
        ),
        (
            "response Set-Cookie: session=set-cookie-secret; Path=/".to_owned(),
            "set-cookie-secret",
        ),
        (
            "unknown flag in --client-secret flag-secret-value".to_owned(),
            "flag-secret-value",
        ),
        (
            r#"retry with --token "quoted flag-quoted-secret""#.to_owned(),
            "flag-quoted-secret",
        ),
        (
            r#"password="first backslash-secret\"#.to_owned(),
            "backslash-secret",
        ),
        (
            r#"--client-secret "first flag-backslash-secret\"#.to_owned(),
            "flag-backslash-secret",
        ),
        (
            r#"Get "https://idp.example/cb?a=it's&code=after-apostrophe-secret""#.to_owned(),
            "after-apostrophe-secret",
        ),
        (
            "callback https://idp.example/it's/cb?code=path-apostrophe-secret".to_owned(),
            "path-apostrophe-secret",
        ),
        (
            r#"msg="login failed" err="password=\"go-quoted secret-word\"""#.to_owned(),
            "secret-word",
        ),
        (
            r#"{\"client_secret\":\"escaped-json secret-tail\"}"#.to_owned(),
            "secret-tail",
        ),
        (
            r#"{"Cookie":"session=json-cookie-secret"}"#.to_owned(),
            "json-cookie-secret",
        ),
        (
            r#"body="{\"Cookie\":\"session=escaped-json-cookie-secret\"}""#.to_owned(),
            "escaped-json-cookie-secret",
        ),
        (
            r#"err="exec: --token \"esc flag-escaped-secret\" failed""#.to_owned(),
            "flag-escaped-secret",
        ),
    ] {
        let line = plugin_error_line(stderr.as_bytes()).unwrap();
        assert!(!line.contains(secret), "{line}");
        assert!(line.contains("[REDACTED]"), "{line}");
    }
}

#[test]
fn escaped_quoted_values_end_at_their_closing_quote() {
    let line = plugin_error_line(br#"err="password=\"go-quoted secret-word\"" code=1"#).unwrap();
    assert_eq!(line, r#"err="password="[REDACTED]"" code=1"#);
    let line =
        plugin_error_line(br#"err="exec: --token \"esc flag-secret\" failed" code=1"#).unwrap();
    assert_eq!(line, r#"err="exec: --token [REDACTED] failed" code=1"#);
}

#[test]
fn plugin_line_keeps_issuer_urls_without_queries() {
    let line = plugin_error_line(
        br#"oidc: issuer URL provided to client ("https://old.example/realms/a") did not match"#,
    )
    .unwrap();
    assert!(line.contains("https://old.example/realms/a"), "{line}");
    // A quote ends the URL, so the text after a quoted URL survives.
    let line =
        plugin_error_line(br#"Get "https://idp.example/x?code=c": dial tcp: refused"#).unwrap();
    assert_eq!(
        line,
        r#"Get "https://idp.example/x?[REDACTED]": dial tcp: refused"#
    );
}
