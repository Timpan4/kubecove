//! Tag and scalar resolution for the YAML parser diagnostics, ported from the
//! web's `yaml` package 2.9.1 with its default YAML 1.2 `core` schema:
//! `Directives.tagName`, `composeScalar`, `composeCollection`, the core tag
//! tests and `coreKnownTags`. Scalar values are decoded only as far as tag
//! tests and key comparison need them.

use super::parse::Level;
use regex::Regex;
use std::{
    collections::HashMap,
    iter::Peekable,
    str::Chars,
    sync::LazyLock,
};
use tree_sitter::Node;

const CORE: &str = "tag:yaml.org,2002:";

/// `coreKnownTags`: tags the core schema resolves only when named, with the
/// collection each expects. `None` is a scalar.
const KNOWN_TAGS: [(&str, Option<&str>); 6] = [
    ("binary", None),
    ("merge", None),
    ("omap", Some("seq")),
    ("pairs", Some("seq")),
    ("set", Some("map")),
    ("timestamp", None),
];

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("the core schema patterns compile")
}

// The `test` patterns of `schema/common/null.js` and `schema/core/*.js`.
static NULL: LazyLock<Regex> = LazyLock::new(|| pattern(r"^(?:~|[Nn]ull|NULL)?$"));
static BOOL: LazyLock<Regex> = LazyLock::new(|| pattern(r"^(?:[Tt]rue|TRUE|[Ff]alse|FALSE)$"));
static INT_OCT: LazyLock<Regex> = LazyLock::new(|| pattern(r"^0o[0-7]+$"));
static INT: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[-+]?[0-9]+$"));
static INT_HEX: LazyLock<Regex> = LazyLock::new(|| pattern(r"^0x[0-9a-fA-F]+$"));
static FLOAT_NAN: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^(?:[-+]?\.(?:inf|Inf|INF)|\.nan|\.NaN|\.NAN)$"));
static FLOAT_EXP: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)[eE][-+]?[0-9]+$"));
static FLOAT: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[-+]?(?:\.[0-9]+|[0-9]+\.[0-9]*)$"));

/// `Directives.defaultTags`, which `%TAG` directives extend.
pub(super) fn default_handles() -> HashMap<String, String> {
    HashMap::from([
        ("!".to_owned(), "!".to_owned()),
        ("!!".to_owned(), CORE.to_owned()),
    ])
}

/// What a tag applies to: the node after the tag and anchor in a
/// `flow_node` or `block_node`.
pub(super) enum Content {
    Map,
    Seq,
    Scalar(String),
    /// An alias, which the tag check skips.
    Other,
}

pub(super) fn content(holder: Node, text: &str) -> Content {
    let mut cursor = holder.walk();
    let value = holder
        .named_children(&mut cursor)
        .find(|child| !matches!(child.kind(), "tag" | "anchor" | "comment"));
    match value.map(|value| (value.kind(), value)) {
        // A tag without a value tags an empty scalar.
        None => Content::Scalar(String::new()),
        Some(("block_mapping" | "flow_mapping", _)) => Content::Map,
        Some(("block_sequence" | "flow_sequence", _)) => Content::Seq,
        Some((
            "plain_scalar" | "single_quote_scalar" | "double_quote_scalar" | "block_scalar",
            value,
        )) => Content::Scalar(scalar_value(value, text)),
        Some(_) => Content::Other,
    }
}

/// The errors and warnings for tag `source` on `content`, in the web's words.
pub(super) fn check(
    source: &str,
    content: &Content,
    handles: &HashMap<String, String>,
) -> Vec<(Level, String)> {
    let mut findings = Vec::new();
    let Some(name) = tag_name(source, handles, &mut findings) else {
        return findings;
    };
    let known = KNOWN_TAGS
        .iter()
        .find(|(known, _)| name.strip_prefix(CORE) == Some(*known))
        .map(|(_, collection)| *collection);
    let unresolved = || (Level::Warning, format!("Unresolved tag: {name}"));
    match content {
        Content::Map | Content::Seq => {
            let kind = if matches!(content, Content::Map) { "map" } else { "seq" };
            if name == "!" || name.strip_prefix(CORE) == Some(kind) {
                return findings;
            }
            match known {
                Some(Some(collection)) if collection == kind => {}
                Some(expects) => findings.push((
                    Level::Warning,
                    format!(
                        "{name} used for {kind} collection, but expects {}",
                        expects.unwrap_or("scalar")
                    ),
                )),
                None => findings.push(unresolved()),
            }
        }
        Content::Scalar(value) => {
            // `findScalarTagByName`: `str` and `!` take any value, the other
            // core tags need their test, then the known scalar tags.
            let value = value.as_str();
            let resolved = name == "!"
                || match name.strip_prefix(CORE) {
                    Some("str") => true,
                    Some("null") => NULL.is_match(value),
                    Some("bool") => BOOL.is_match(value),
                    Some("int") => [&INT_OCT, &INT, &INT_HEX].iter().any(|test| test.is_match(value)),
                    Some("float") => {
                        [&FLOAT_NAN, &FLOAT_EXP, &FLOAT].iter().any(|test| test.is_match(value))
                    }
                    _ => false,
                }
                || matches!(known, Some(None));
            if !resolved {
                findings.push(unresolved());
            }
        }
        Content::Other => {}
    }
    findings
}

/// `Directives.tagName`: the full tag name, or `None` when it cannot resolve.
fn tag_name(
    source: &str,
    handles: &HashMap<String, String>,
    findings: &mut Vec<(Level, String)>,
) -> Option<String> {
    if source == "!" {
        return Some(source.to_owned());
    }
    if let Some(body) = source.strip_prefix("!<") {
        // `source.slice(2, -1)` drops the last character, `>` or not.
        let verbatim = body.char_indices().last().map_or("", |(end, _)| &body[..end]);
        if matches!(verbatim, "!" | "!!") {
            findings.push((
                Level::Error,
                format!("Verbatim tags aren't resolved, so {source} is invalid."),
            ));
            return None;
        }
        if !source.ends_with('>') {
            findings.push((Level::Error, "Verbatim tags must end with a >".to_owned()));
        }
        return Some(verbatim.to_owned());
    }
    // Tree-sitter tags start with `!`, so the handle runs to the last `!`.
    let split = source.rfind('!').map_or(0, |index| index + 1);
    let (handle, suffix) = source.split_at(split);
    if suffix.is_empty() {
        findings.push((Level::Error, format!("The {source} tag has no suffix")));
    }
    let Some(prefix) = handles.get(handle) else {
        findings.push((Level::Error, format!("Could not resolve tag: {source}")));
        return None;
    };
    match decode_uri_component(suffix) {
        Some(suffix) => Some(format!("{prefix}{suffix}")),
        None => {
            findings.push((Level::Error, "URIError: URI malformed".to_owned()));
            None
        }
    }
}

/// JavaScript's `decodeURIComponent`: `%XX` bytes decoded as UTF-8.
fn decode_uri_component(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes.get(index + 1..index + 3)?;
            if !hex.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            decoded.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// A mapping key as `uniqueKeys` compares it: the resolved scalar value, with
/// numbers by their bits so `1` and `0x1` match. `None` for keys that never
/// equal another: collections, aliases, empty keys and NaN.
#[derive(Hash, PartialEq, Eq)]
pub(super) enum KeyValue {
    Null,
    Bool(bool),
    Number(u64),
    Text(String),
}

pub(super) fn key_value(key: Node, text: &str) -> Option<KeyValue> {
    let mut cursor = key.walk();
    let mut tagged = false;
    let mut scalar = None;
    for child in key.named_children(&mut cursor) {
        match child.kind() {
            "tag" => tagged = true,
            "anchor" | "comment" => {}
            "plain_scalar" | "single_quote_scalar" | "double_quote_scalar" | "block_scalar" => {
                scalar = Some(child);
            }
            _ => return None,
        }
    }
    let scalar = scalar?;
    let value = scalar_value(scalar, text);
    if tagged || scalar.kind() != "plain_scalar" {
        return Some(KeyValue::Text(value));
    }
    Some(if NULL.is_match(&value) {
        KeyValue::Null
    } else if BOOL.is_match(&value) {
        KeyValue::Bool(value.starts_with(['t', 'T']))
    } else if let Some(number) = core_number(&value) {
        if number.is_nan() {
            return None;
        }
        // `-0 === 0` in JavaScript.
        KeyValue::Number(if number == 0.0 { 0 } else { number.to_bits() })
    } else {
        KeyValue::Text(value)
    })
}

/// The number a plain scalar resolves to in the core schema, in the order of
/// its tags: octal, decimal and hex integers, then floats.
fn core_number(value: &str) -> Option<f64> {
    let digits = |radix: u32| {
        value[2..]
            .chars()
            .filter_map(|digit| digit.to_digit(radix))
            .fold(0.0, |total, digit| total * f64::from(radix) + f64::from(digit))
    };
    if INT_OCT.is_match(value) {
        Some(digits(8))
    } else if INT.is_match(value) {
        value.parse().ok()
    } else if INT_HEX.is_match(value) {
        Some(digits(16))
    } else if FLOAT_NAN.is_match(value) {
        Some(if value.ends_with(['n', 'N']) {
            f64::NAN
        } else if value.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    } else if FLOAT_EXP.is_match(value) || FLOAT.is_match(value) {
        value.parse().ok()
    } else {
        None
    }
}

/// The scalar's value with quotes, escapes and line folding resolved.
pub(super) fn scalar_value(scalar: Node, text: &str) -> String {
    let raw = &text[scalar.byte_range()];
    match scalar.kind() {
        "single_quote_scalar" => flow_value(unquote(raw, '\''), Some('\'')),
        "double_quote_scalar" => flow_value(unquote(raw, '"'), Some('"')),
        "block_scalar" => block_value(raw),
        _ => flow_value(raw, None),
    }
}

fn unquote(raw: &str, quote: char) -> &str {
    let inner = raw.strip_prefix(quote).unwrap_or(raw);
    inner.strip_suffix(quote).unwrap_or(inner)
}

/// Folds line breaks to spaces, and blank lines to line breaks, then resolves
/// `''` in single quotes and backslash escapes in double quotes.
fn flow_value(inner: &str, quote: Option<char>) -> String {
    let mut value = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quote) {
            ('\'', Some('\'')) if chars.next_if_eq(&'\'').is_some() => value.push('\''),
            ('\\', Some('"')) => match chars.next() {
                // An escaped line break joins the lines.
                Some('\n') => skip_blanks(&mut chars),
                Some(escape) => push_escape(&mut value, escape, &mut chars),
                None => value.push('\\'),
            },
            ('\n', _) => {
                value.truncate(value.trim_end_matches([' ', '\t', '\r']).len());
                skip_blanks(&mut chars);
                let mut breaks = 0;
                while chars.next_if_eq(&'\n').is_some() {
                    breaks += 1;
                    skip_blanks(&mut chars);
                }
                if breaks == 0 {
                    value.push(' ');
                } else {
                    value.extend(std::iter::repeat_n('\n', breaks));
                }
            }
            _ => value.push(c),
        }
    }
    value
}

fn skip_blanks(chars: &mut Peekable<Chars<'_>>) {
    while chars.next_if(|c| matches!(*c, ' ' | '\t' | '\r')).is_some() {}
}

/// The double-quoted escapes of `resolve-flow-scalar.js`. An unknown escape
/// keeps its text.
fn push_escape(value: &mut String, escape: char, chars: &mut Peekable<Chars<'_>>) {
    let simple = match escape {
        '0' => Some('\0'),
        'a' => Some('\x07'),
        'b' => Some('\x08'),
        'e' => Some('\x1b'),
        'f' => Some('\x0c'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' | '\t' => Some('\t'),
        'v' => Some('\x0b'),
        'N' => Some('\u{85}'),
        '_' => Some('\u{a0}'),
        'L' => Some('\u{2028}'),
        'P' => Some('\u{2029}'),
        ' ' | '"' | '/' | '\\' => Some(escape),
        _ => None,
    };
    if let Some(decoded) = simple {
        value.push(decoded);
        return;
    }
    let length = match escape {
        'x' => 2,
        'u' => 4,
        'U' => 8,
        _ => 0,
    };
    let digits: String = chars.by_ref().take(length).collect();
    let hex = length > 0 && digits.len() == length && digits.bytes().all(|b| b.is_ascii_hexdigit());
    match u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32) {
        Some(decoded) if hex => value.push(decoded),
        _ => {
            value.push('\\');
            value.push(escape);
            value.push_str(&digits);
        }
    }
}

/// A block scalar's content: lines without the first content line's indent,
/// joined by line breaks for `|` and folded for `>`, then chomped.
fn block_value(raw: &str) -> String {
    let (header, body) = raw.split_once('\n').unwrap_or((raw, ""));
    let header = header.split('#').next().unwrap_or_default();
    let lines: Vec<&str> = body.split('\n').collect();
    let indent = lines
        .iter()
        .find(|line| !line.trim().is_empty())
        .map_or(0, |line| line.len() - line.trim_start_matches(' ').len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |last| last + 1);
    let mut value = String::new();
    let mut breaks = 0;
    for line in &lines[..end] {
        if line.trim().is_empty() {
            breaks += 1;
            continue;
        }
        if !value.is_empty() {
            let folded = header.starts_with('>') && breaks == 0;
            if folded {
                value.push(' ');
            } else {
                value.extend(std::iter::repeat_n('\n', breaks + usize::from(!header.starts_with('>'))));
            }
        }
        value.push_str(line.get(indent..).unwrap_or_default());
        breaks = 0;
    }
    // Line breaks after the last content line.
    let trailing = if end == 0 { 0 } else { lines.len() - end };
    if header.contains('+') {
        value.extend(std::iter::repeat_n('\n', trailing));
    } else if !header.contains('-') && end > 0 && trailing > 0 {
        value.push('\n');
    }
    value
}
