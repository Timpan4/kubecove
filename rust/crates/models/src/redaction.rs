//! Credential redaction for text that crosses the frontend boundary.

use regex::Regex;
use std::sync::LazyLock;

/// `SENSITIVE_DETAIL_PATTERNS` in `src/lib/error-redaction.ts`, applied in the same order.
static SENSITIVE_DETAIL: LazyLock<[(Regex, &str); 6]> = LazyLock::new(|| {
    [
        (
            r"(?i)(\bauthorization\s*:\s*(?:basic|bearer)\s+)[^\s,;]+",
            "${1}[REDACTED]",
        ),
        (r"(?i)(\b(?:x-api-key|api-key)\s*:\s*)[^\s,;]+", "${1}[REDACTED]"),
        (r"(?i)(\bbearer\s+)[A-Za-z0-9._~+/-]+=*", "${1}[REDACTED]"),
        (r"(?i)(https?://)[^/\s:@]+:[^/\s@]+@", "${1}[REDACTED]@"),
        (
            r"(?i)([?&](?:access[_-]?token|id[_-]?token|token|password|client[_-]?secret|x-amz-credential|x-amz-security-token|x-amz-signature|sig|signature|(?:x[-_])?api[-_]?key)=)[^&#\s]+",
            "${1}[REDACTED]",
        ),
        (
            r#"(?i)(["']?(?:access[_-]?token|id[_-]?token|token|password|client[_-]?secret|(?:x[-_])?api[-_]?key)["']?\s*[:=]\s*["']?)[^"',\s}]+"#,
            "${1}[REDACTED]",
        ),
    ]
    .map(compile)
});

/// Whole values of credential keys and flags. They run before the patterns
/// above, which stop at the first space or backslash and would leave the rest
/// of a value behind.
static WHOLE_VALUES: LazyLock<[(Regex, &str); 2]> = LazyLock::new(|| {
    [
        // A quoted value, closed or not, with escaped quotes inside. A lone
        // backslash can end an unclosed value.
        (
            r#"(?i)(\b[\w-]*(?:token|passw(?:or)?d|passphrase|secret|credential|api[-_]?key|access[-_]?key|private[-_]?key)[\w-]*["']?\s*[:=]\s*)(?:"(?:[^"\\]|\\.?)*(?:"|$)|'(?:[^'\\]|\\.?)*(?:'|$))"#,
            "${1}\"[REDACTED]\"",
        ),
        // A credential flag's separate value, such as `--client-secret abc`.
        (
            r#"(?i)((?:^|\s)--?[\w-]*(?:token|passw(?:or)?d|passphrase|secret|credential|api[-_]?key|access[-_]?key|private[-_]?key)[\w-]*\s+)(?:"(?:[^"\\]|\\.?)*(?:"|$)|'(?:[^'\\]|\\.?)*(?:'|$)|[^\s"'-]\S*)"#,
            "${1}[REDACTED]",
        ),
    ]
    .map(compile)
});

/// Credential plugins also print OAuth redirects and raw tokens, which the
/// frontend patterns above do not cover.
static PLUGIN_DETAIL: LazyLock<[(Regex, &str); 6]> = LazyLock::new(|| {
    [
        // Any URL query or fragment: authorization codes, state and tokens.
        // A double quote or angle bracket ends the URL, as in
        // `Get "https://…": `. An apostrophe does not, because a URL can hold one.
        (
            r#"(?i)(\bhttps?://[^\s?#"<>]*)[?#][^\s"<>]*"#,
            "${1}?[REDACTED]",
        ),
        // Cookie headers carry session credentials to the end of the line
        // (RFC 6265).
        (r"(?i)(\b(?:set-)?cookie\s*:\s*)\S.*", "${1}[REDACTED]"),
        (
            r#"(?i)(["']?(?:refresh[_-]?token|secret|session[_-]?token)["']?\s*[:=]\s*["']?)[^"',\s}&]+"#,
            "${1}[REDACTED]",
        ),
        // Any key that names a credential, such as `passphrase=` or
        // `AWS_SECRET_ACCESS_KEY=`, assigned with `=` or as a JSON key. A bare
        // `name:` would also match prose such as `credentials: not found`.
        (
            r#"(?i)(\b[\w-]*(?:token|passw(?:or)?d|passphrase|secret|credential|api[-_]?key|access[-_]?key|private[-_]?key)[\w-]*(?:\s*=|["']\s*:)\s*["']?)[^"',\s}&]+"#,
            "${1}[REDACTED]",
        ),
        // The same with a bare `:` for keys that do not read as prose, such as
        // `passphrase: ` or `AWS_SECRET_ACCESS_KEY: `.
        (
            r#"(?i)(\b(?:passphrase|[\w-]*(?:secret|api[-_]?key|access[-_]?key|private[-_]?key)[\w-]*)\s*:\s*)[^"',\s}&]+"#,
            "${1}[REDACTED]",
        ),
        // JSON Web Tokens and other base64url segments joined by dots.
        (
            r"\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]*)?",
            "[REDACTED]",
        ),
    ]
    .map(compile)
});

fn compile((pattern, replacement): (&str, &'static str)) -> (Regex, &'static str) {
    (
        Regex::new(pattern).expect("redaction pattern compiles"),
        replacement,
    )
}

fn apply(patterns: &[(Regex, &str)], detail: &str) -> String {
    patterns
        .iter()
        .fold(detail.to_owned(), |text, (pattern, replacement)| {
            pattern.replace_all(&text, *replacement).into_owned()
        })
}

/// Removes credentials from an error detail before it is shown or copied.
pub fn redact_sensitive_detail(detail: &str) -> String {
    apply(&*SENSITIVE_DETAIL, detail)
}

/// The last non-empty stderr line of a failed credential plugin, without
/// terminal escapes, control or bidirectional formatting characters, and with
/// credentials removed. Plugins print their final error last.
pub(crate) fn plugin_error_line(stderr: &[u8]) -> Option<String> {
    // Escapes go first: an OSC string can span lines. A carriage return
    // starts over, as progress output overwrites itself in a terminal.
    let text = strip_escapes(&String::from_utf8_lossy(stderr));
    let line = text
        .split(['\n', '\r'])
        .map(printable)
        .rfind(|line| !line.trim().is_empty())?;
    let whole = apply(&*WHOLE_VALUES, line.trim());
    let redacted = apply(&*PLUGIN_DETAIL, &redact_sensitive_detail(&whole));
    Some(redacted)
}

/// Removes ANSI CSI and OSC sequences, in their 7-bit and C1 forms, which
/// can hide or rewrite text.
fn strip_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let introducer = match ch {
            '\u{1b}' => chars.next(),
            '\u{9b}' => Some('['),
            '\u{9d}' => Some(']'),
            _ => {
                out.push(ch);
                continue;
            }
        };
        match introducer {
            Some('[') => {
                // CSI ends at the first byte in 0x40..=0x7e.
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC ends at BEL, ST or ESC \.
                while let Some(next) = chars.next() {
                    if matches!(next, '\u{7}' | '\u{9c}') {
                        break;
                    }
                    if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn printable(line: &str) -> String {
    line.chars()
        .map(|ch| if ch == '\t' { ' ' } else { ch })
        .filter(|ch| !ch.is_control() && !is_bidi_control(*ch))
        .collect()
}

const fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
#[path = "redaction_tests.rs"]
mod tests;
