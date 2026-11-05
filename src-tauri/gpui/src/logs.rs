//! Log line parsing, ported from `src/features/resource-detail/log-helpers.ts`
//! and `log-structure.ts`. Lines are parsed once on arrival, into
//! `log_buffer::LogBuffer`, so rendering only reads precomputed text.

use gpui_kit::SharedString;
use kubecove_lib::models::LogLineSource;
use regex::Regex;
use serde_json::Value;
use std::fmt;
use std::sync::LazyLock;

/// `MAX_RETAINED_LOG_LINES`; the backend clamps tail requests to the same value.
pub const MAX_RETAINED_LOG_LINES: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub const ALL: [LogLevel; 4] = [Self::Error, Self::Warn, Self::Info, Self::Debug];

    /// `LEVEL_LABELS`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "ERR",
            Self::Warn => "WRN",
            Self::Info => "INF",
            Self::Debug => "DBG",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    fn from_alias(value: &str) -> Option<Self> {
        Some(match value.to_lowercase().as_str() {
            "err" | "error" | "fatal" | "panic" | "crit" | "critical" => Self::Error,
            "wrn" | "warn" | "warning" => Self::Warn,
            "inf" | "info" | "notice" => Self::Info,
            "dbg" | "debug" | "trc" | "trace" => Self::Debug,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogField {
    pub key: SharedString,
    pub value: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogStructure {
    pub level: Option<LogLevel>,
    pub headline: SharedString,
    pub error: Option<SharedString>,
    pub fields: Vec<LogField>,
}

#[derive(Clone, Debug)]
pub struct ParsedLogLine {
    /// Arrival order across the stream, which addresses the line in the buffer.
    pub index: u64,
    pub structure: LogStructure,
    /// `line.raw`: the line as streamed, copied by the expanded row's Copy line.
    pub raw: SharedString,
    /// `formatLogTime`: local time of day with milliseconds, or `-`.
    pub time: SharedString,
    /// `LogEntry`'s `sourceLabel`: `pod/container`, the Pod alone, or empty.
    pub source_label: SharedString,
    /// `logLineSearchText`: raw text, Pod and container, lowercased.
    pub search: String,
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("log pattern compiles")
}

// JavaScript `\d` and `\w` are ASCII-only, so the ports spell out ASCII classes.
static LINE_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})?)\s+(.*)$",
    )
});
static EMBEDDED_TIME: LazyLock<Regex> = LazyLock::new(|| regex(r#"\btime=(?:"([^"]+)"|([^\s]+))"#));
static STRUCTURE_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:?[0-9]{2})?\s+",
    )
});
static KLOG_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^([IWEF])[0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+\s+[0-9]+\s+[^\s\]]+:[0-9]+\]\s?")
});
static LEVEL_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?i)^\[?(err|error|fatal|panic|crit|critical|wrn|warn|warning|inf|info|notice|dbg|debug|trc|trace)\]?(?::|\s|$)\s*",
    )
});
static FIRST_FIELD: LazyLock<Regex> = LazyLock::new(|| regex(r"(?:^|\s)[A-Za-z_][A-Za-z0-9_.-]*="));
// `FIELD_RE` is sticky in JavaScript; matching an anchored pattern on the
// remaining slice is equivalent because the pattern has no look-behind.
static FIELD: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"^\s*([A-Za-z_][A-Za-z0-9_.-]*)=("(?:[^"\\]|\\.)*"|\S*)"#));
static QUOTED: LazyLock<Regex> = LazyLock::new(|| regex(r#"^"(?:[^"\\]|\\.)*"$"#));
static ESCAPE: LazyLock<Regex> = LazyLock::new(|| regex(r"\\(.)"));

const LEVEL_KEYS: &[&str] = &["level", "lvl", "severity"];
const MESSAGE_KEYS: &[&str] = &["msg", "message"];
const ERROR_KEYS: &[&str] = &["error", "err"];
const TIME_KEYS: &[&str] = &["time", "ts", "timestamp", "@timestamp"];

fn unquote(value: &str) -> String {
    if !QUOTED.is_match(value) {
        return value.to_owned();
    }
    serde_json::from_str::<String>(value).unwrap_or_else(|_| {
        ESCAPE
            .replace_all(&value[1..value.len() - 1], "$1")
            .into_owned()
    })
}

/// `stringValue`: strings stay raw, anything else is its JSON text.
fn string_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn finish(
    level: Option<LogLevel>,
    headline: String,
    message: Option<String>,
    error: Option<String>,
    fields: Vec<LogField>,
) -> LogStructure {
    let mut text = if headline.is_empty() {
        message.unwrap_or_default()
    } else {
        headline
    };
    let mut error = error.filter(|error| !error.is_empty());
    if text.is_empty()
        && let Some(message) = error.take()
    {
        text = message;
    }
    LogStructure {
        level,
        headline: text.into(),
        error: error.map(SharedString::from),
        fields,
    }
}

/// Object entries in document order, as `Object.entries` yields them.
struct Entries(Vec<(String, Value)>);

impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Entries;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Entries, A::Error> {
                let mut entries: Vec<(String, Value)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    // A repeated key keeps its first position and its last value.
                    match entries.iter_mut().find(|(existing, _)| *existing == key) {
                        Some(entry) => entry.1 = value,
                        None => entries.push((key, value)),
                    }
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

fn field(key: impl Into<SharedString>, value: impl Into<SharedString>) -> LogField {
    LogField {
        key: key.into(),
        value: value.into(),
    }
}

fn parse_json(text: &str) -> Option<LogStructure> {
    let Entries(entries) = serde_json::from_str(text).ok()?;
    let (mut level, mut message, mut error) = (None, None, None);
    let mut fields = Vec::new();
    for (key, raw) in entries {
        let lower = key.to_lowercase();
        if TIME_KEYS.contains(&lower.as_str()) {
            continue;
        }
        let value = string_value(&raw);
        let mapped = LEVEL_KEYS
            .contains(&lower.as_str())
            .then(|| LogLevel::from_alias(&value))
            .flatten();
        if let Some(mapped) = mapped {
            level = level.or(Some(mapped));
        } else if MESSAGE_KEYS.contains(&lower.as_str()) && message.is_none() {
            message = Some(value);
        } else if ERROR_KEYS.contains(&lower.as_str()) && error.is_none() {
            error = Some(value);
        } else {
            fields.push(field(key, value));
        }
    }
    Some(finish(level, String::new(), message, error, fields))
}

/// `splitFields`: trailing logfmt pairs, or none when the tail is not pure logfmt.
fn split_fields(text: &str) -> (&str, Vec<(&str, &str)>) {
    let Some(start) = FIRST_FIELD.find(text).map(|found| found.start()) else {
        return (text, Vec::new());
    };
    let mut pairs = Vec::new();
    let mut position = start;
    while position < text.len() {
        let rest = &text[position..];
        let Some(captures) = FIELD.captures(rest) else {
            if rest.trim().is_empty() {
                break;
            }
            return (text, Vec::new());
        };
        let (key, value) = (captures.get(1).unwrap(), captures.get(2).unwrap());
        pairs.push((key.as_str(), value.as_str()));
        position += captures.get(0).unwrap().end();
    }
    (text[..start].trim(), pairs)
}

/// `parseLogStructure`.
pub fn parse_structure(message: &str) -> LogStructure {
    let text = STRUCTURE_TIMESTAMP.replace(message, "");
    if text.starts_with('{') {
        return parse_json(&text).unwrap_or_else(|| LogStructure {
            level: None,
            headline: text.into_owned().into(),
            error: None,
            fields: Vec::new(),
        });
    }
    let mut level = None;
    let mut rest: &str = &text;
    if let Some(klog) = KLOG_PREFIX.captures(rest) {
        level = match &klog[1] {
            "I" => Some(LogLevel::Info),
            "W" => Some(LogLevel::Warn),
            _ => Some(LogLevel::Error),
        };
        rest = &rest[klog.get(0).unwrap().end()..];
    } else if let Some(token) = LEVEL_TOKEN.captures(rest) {
        level = LogLevel::from_alias(&token[1]);
        rest = &rest[token.get(0).unwrap().end()..];
    }
    let (headline, pairs) = split_fields(rest);
    let headline = unquote(headline.trim());
    let (mut message, mut error) = (None, None);
    let mut fields = Vec::new();
    for (key, raw) in pairs {
        let lower = key.to_lowercase();
        if TIME_KEYS.contains(&lower.as_str()) {
            continue;
        }
        let value = unquote(raw);
        let mapped = LEVEL_KEYS
            .contains(&lower.as_str())
            .then(|| LogLevel::from_alias(&value))
            .flatten();
        if let Some(mapped) = mapped {
            level = level.or(Some(mapped));
        // A prose headline already fills the headline slot, so msg stays visible as a field.
        } else if MESSAGE_KEYS.contains(&lower.as_str()) && message.is_none() && headline.is_empty()
        {
            message = Some(value);
        } else if ERROR_KEYS.contains(&lower.as_str()) && error.is_none() {
            error = Some(value);
        } else {
            fields.push(field(key.to_owned(), value));
        }
    }
    finish(level, headline, message, error, fields)
}

/// `formatExactTimeOnly(..., "local", "millisecond") ?? "-"`.
fn time_label(timestamp: Option<&str>) -> SharedString {
    timestamp
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map_or(SharedString::new_static("-"), |time| {
            time.with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
                .into()
        })
}

/// `parseLogLine`.
pub fn parse_line(line: &str, source: Option<&LogLineSource>, index: u64) -> ParsedLogLine {
    let (timestamp, structure) = match LINE_TIMESTAMP.captures(line) {
        Some(captures) => (
            captures.get(1).map(|found| found.as_str()),
            parse_structure(&captures[2]),
        ),
        None => (
            EMBEDDED_TIME
                .captures(line)
                .and_then(|captures| captures.get(1).or(captures.get(2)))
                .map(|found| found.as_str()),
            parse_structure(line),
        ),
    };
    let mut search = line.to_lowercase();
    if let Some(source) = source {
        for part in [Some(source.pod_name.as_str()), source.container.as_deref()]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
        {
            search.push(' ');
            search.push_str(&part.to_lowercase());
        }
    }
    let source_label = source.map_or(SharedString::default(), |source| {
        match source.container.as_deref() {
            Some(container) if !container.is_empty() => {
                format!("{}/{container}", source.pod_name).into()
            }
            _ => source.pod_name.clone().into(),
        }
    });
    ParsedLogLine {
        index,
        raw: line.into(),
        time: time_label(timestamp),
        source_label,
        structure,
        search,
    }
}

/// `highlightSegments` as byte ranges of `text` that match `term` (already lowercased).
pub fn highlight_ranges(text: &str, term: &str) -> Vec<std::ops::Range<usize>> {
    if term.is_empty() {
        return Vec::new();
    }
    let haystack = text.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; ranges would not map back.
    if haystack.len() != text.len() {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while let Some(found) = haystack[cursor..].find(term) {
        let start = cursor + found;
        let end = start + term.len();
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            break;
        }
        ranges.push(start..end);
        cursor = end;
    }
    ranges
}

/// Names the Logs tab's container picker offers for a Pod: regular containers,
/// or every status entry when there are only init containers.
pub fn container_options(status: Option<&Value>) -> Vec<String> {
    let names = |key: &str| -> Vec<String> {
        status
            .and_then(|status| status.get(key))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|container| container.is_object())
            .map(|container| {
                container["name"]
                    .as_str()
                    .map_or("container".into(), str::to_owned)
            })
            .collect()
    };
    let regular: Vec<String> = ["containerStatuses", "ephemeralContainerStatuses"]
        .iter()
        .flat_map(|key| names(key))
        .collect();
    if regular.is_empty() {
        names("initContainerStatuses")
    } else {
        regular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Failure modes: logfmt tails that are not pure logfmt must not lose text;
    // `msg` beside a prose headline must stay visible; JSON key order must
    // follow the document.

    #[test]
    fn logfmt_tail_must_be_pure() {
        let parsed = parse_structure("level=warn msg=\"retry budget\" attempt=3");
        assert_eq!(parsed.level, Some(LogLevel::Warn));
        assert_eq!(parsed.headline.as_ref(), "retry budget");
        assert_eq!(parsed.fields, vec![field("attempt", "3")]);
        let prose = parse_structure("handled a=b then more prose");
        assert_eq!(prose.headline.as_ref(), "handled a=b then more prose");
        assert!(prose.fields.is_empty());
    }

    #[test]
    fn prose_headline_keeps_msg_as_a_field() {
        let parsed = parse_structure("INFO started msg=hello");
        assert_eq!(parsed.headline.as_ref(), "started");
        assert_eq!(parsed.fields, vec![field("msg", "hello")]);
    }

    #[test]
    fn json_fields_keep_document_order() {
        let parsed = parse_structure(r#"{"zeta":1,"level":"error","msg":"boom","alpha":{"b":2}}"#);
        assert_eq!(parsed.level, Some(LogLevel::Error));
        assert_eq!(parsed.headline.as_ref(), "boom");
        assert_eq!(
            parsed.fields,
            vec![field("zeta", "1"), field("alpha", r#"{"b":2}"#)]
        );
    }
}
