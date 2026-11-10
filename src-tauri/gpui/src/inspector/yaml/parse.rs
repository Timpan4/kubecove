//! YAML parser diagnostics for the editor: the errors and warnings that the
//! web's `parseYamlErrors` reports (npm `yaml` 2.9.1 with `strict` and
//! `uniqueKeys`), read from a tree-sitter-yaml tree.
//!
//! Duplicate keys, tag resolution, directives and a second document are ports
//! of the web's checks, with its messages and ranges. Syntax errors come from
//! tree-sitter's error recovery, which differs from the `yaml` package's. A
//! recovered region covers the valid text before the fault, so its finding sits
//! on the first token after the region, where the web usually reports. The
//! message is native. After a syntax error outside any collection the web also
//! reports every later token, where native reports the region once.
//!
//! Not ported: a `%YAML 1.1` directive, which switches the web to the YAML 1.1
//! schema, and the web's composer errors that tree-sitter accepts or reports
//! as a syntax error.

use super::tags::{self, KeyValue};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};
use tree_sitter::{Node, Parser};

const MULTIPLE_DOCS: &str =
    "Source contains multiple documents; please use YAML.parseAllDocuments()";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Level {
    Error,
    Warning,
}

/// One diagnostic, in byte offsets of the draft.
pub(super) struct Finding {
    pub range: Range<usize>,
    pub level: Level,
    pub message: String,
}

/// Every parser finding for `text`: errors, then warnings, each in document
/// order, as `[...document.errors, ...document.warnings]` lists them.
pub(super) fn diagnostics(text: &str) -> Vec<Finding> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_yaml::LANGUAGE.into())
        .expect("tree-sitter-yaml uses the tree-sitter ABI this crate links");
    let tree = parser
        .parse(text, None)
        .expect("a parse without a timeout or cancellation flag completes");
    let mut scan = Scan {
        text,
        findings: Vec::new(),
        handles: tags::default_handles(),
    };
    let root = tree.root_node();
    let mut cursor = root.walk();
    let second = (root.kind() == "stream")
        .then(|| {
            root.named_children(&mut cursor)
                .filter(|child| child.kind() == "document")
                .nth(1)
        })
        .flatten();
    // `parseDocument` composes the first document and stops at the second.
    let end = second.map(|document| {
        scan.push(
            document.byte_range(),
            Level::Error,
            MULTIPLE_DOCS.to_owned(),
        );
        document.start_byte()
    });
    scan.walk(root, end);
    let mut findings = scan.findings;
    findings.sort_by_key(|finding| (finding.level == Level::Warning, finding.range.start));
    findings.dedup_by(|a, b| a.range == b.range && a.message == b.message);
    findings
}

struct Scan<'text> {
    text: &'text str,
    findings: Vec<Finding>,
    /// Tag handles, from `%TAG` directives over the defaults.
    handles: HashMap<String, String>,
}

impl Scan<'_> {
    /// Visits the nodes before `end` in document order, so `%TAG` directives
    /// apply to the tags after them. Only the outermost of nested recovered
    /// regions reports.
    fn walk(&mut self, root: Node, end: Option<usize>) {
        let mut stack = vec![(root, false)];
        let mut children = Vec::new();
        let mut cursor = root.walk();
        while let Some((node, in_error)) = stack.pop() {
            if end.is_some_and(|end| node.start_byte() >= end) {
                continue;
            }
            if node.is_error() && !in_error {
                self.syntax_error(node);
            }
            if node.is_missing() && !in_error {
                self.push(
                    node.start_byte()..node.start_byte(),
                    Level::Error,
                    format!("Missing \"{}\"", node.kind()),
                );
            }
            match node.kind() {
                "yaml_directive" | "tag_directive" | "reserved_directive" => self.directive(node),
                "tag" => self.tag(node),
                // Recovery keeps the pairs of a broken mapping in the region.
                "block_mapping" | "flow_mapping" | "ERROR" => self.unique_keys(node),
                _ => {}
            }
            children.extend(node.children(&mut cursor));
            let in_error = in_error || node.is_error();
            stack.extend(children.drain(..).rev().map(|child| (child, in_error)));
        }
    }

    /// `yamlErrorDiagnostic`: an empty range covers the next character, where
    /// the draft has one.
    fn push(&mut self, range: Range<usize>, level: Level, message: String) {
        let from = range.start.min(self.text.len());
        let to = if range.end > from {
            range.end.min(self.text.len())
        } else {
            from + self.text[from..].chars().next().map_or(0, char::len_utf8)
        };
        self.findings.push(Finding {
            range: from..to,
            level,
            message,
        });
    }

    /// One finding on the first token after the recovered region, or at the
    /// end of the draft when nothing follows.
    fn syntax_error(&mut self, region: Node) {
        let rest = &self.text[region.end_byte()..];
        let from = region.end_byte()
            + rest
                .find(|c: char| !matches!(c, ' ' | '\t' | '\r' | '\n'))
                .unwrap_or(rest.len());
        let token = token(&self.text[from..]);
        let message = if token.is_empty() {
            "Unexpected end of YAML".to_owned()
        } else {
            format!("Unexpected \"{token}\"")
        };
        self.push(from..from + token.len(), Level::Error, message);
    }

    /// `Directives.add`: `%TAG` registers a handle, `%YAML` names a version and
    /// any other directive is unknown.
    fn directive(&mut self, node: Node) {
        let range = node.byte_range();
        let parts: Vec<&str> = self.text[range.clone()]
            .split([' ', '\t'])
            .filter(|part| !part.is_empty())
            .collect();
        match parts.as_slice() {
            ["%TAG", rest @ ..] => {
                if rest.len() != 2 {
                    let message = "%TAG directive should contain exactly two parts";
                    self.push(range, Level::Error, message.to_owned());
                }
                if let [handle, prefix, ..] = rest {
                    self.handles
                        .insert((*handle).to_owned(), (*prefix).to_owned());
                }
            }
            ["%YAML", version] => {
                if !matches!(*version, "1.1" | "1.2") {
                    let numeric = version.split_once('.').is_some_and(|(major, minor)| {
                        [major, minor].iter().all(|part| {
                            !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                        })
                    });
                    // The web reports from the version, six characters in.
                    let from = (range.start + 6).min(range.end);
                    let level = if numeric {
                        Level::Warning
                    } else {
                        Level::Error
                    };
                    self.push(
                        from..range.end,
                        level,
                        format!("Unsupported YAML version {version}"),
                    );
                }
            }
            ["%YAML", ..] => {
                let message = "%YAML directive should contain exactly one part";
                self.push(range, Level::Error, message.to_owned());
            }
            [name, ..] => {
                self.push(range, Level::Warning, format!("Unknown directive {name}"));
            }
            [] => {}
        }
    }

    fn tag(&mut self, tag: Node) {
        let Some(holder) = tag
            .parent()
            .filter(|parent| matches!(parent.kind(), "flow_node" | "block_node"))
        else {
            return;
        };
        let content = tags::content(holder, self.text);
        let source = &self.text[tag.byte_range()];
        for (level, message) in tags::check(source, &content, &self.handles) {
            self.push(tag.byte_range(), level, message);
        }
    }

    /// `uniqueKeys`: each key equal to an earlier key of the same mapping.
    fn unique_keys(&mut self, mapping: Node) {
        let mut seen: HashSet<KeyValue> = HashSet::new();
        let mut cursor = mapping.walk();
        let keys: Vec<Node> = mapping
            .named_children(&mut cursor)
            .filter_map(|entry| match entry.kind() {
                "block_mapping_pair" | "flow_pair" => entry.child_by_field_name("key"),
                // `{a, b: 1}`: a key without a value.
                "flow_node" if mapping.kind() == "flow_mapping" => Some(entry),
                _ => None,
            })
            .collect();
        for key in keys {
            let Some(value) = tags::key_value(key, self.text, &self.handles) else {
                continue;
            };
            if !seen.insert(value) {
                let start = key.start_byte();
                self.push(
                    start..start,
                    Level::Error,
                    "Map keys must be unique".to_owned(),
                );
            }
        }
    }
}

/// The token at the start of `rest`: one flow indicator, colon, comment sign
/// or quote, or the text up to whitespace or the next of those.
fn token(rest: &str) -> &str {
    const INDICATORS: [char; 9] = [',', '[', ']', '{', '}', ':', '#', '"', '\''];
    let Some(first) = rest.chars().next() else {
        return rest;
    };
    if INDICATORS.contains(&first) {
        return &rest[..first.len_utf8()];
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || INDICATORS.contains(&c))
        .unwrap_or(rest.len());
    &rest[..end]
}

#[cfg(test)]
mod tests {
    use super::diagnostics;

    /// The 1-based lines of the duplicate-key errors in `text`.
    fn duplicate_lines(text: &str) -> Vec<usize> {
        diagnostics(text)
            .into_iter()
            .filter(|finding| finding.message == "Map keys must be unique")
            .map(|finding| text[..finding.range.start].matches('\n').count() + 1)
            .collect()
    }

    // An explicit core tag picks the key's type, as the web's `yaml` package
    // resolves it. Failure mode: every tagged key read as text, so `!!int 1`
    // and `1` differed and `!!str true` matched `!!bool true`.
    #[test]
    fn tagged_keys_compare_by_their_resolved_type() {
        assert_eq!(duplicate_lines("!!int 1: a\n1: b\n"), [2]);
        assert_eq!(duplicate_lines("!!int 0x1: a\n!<tag:yaml.org,2002:int> 1: b\n"), [2]);
        assert!(duplicate_lines("!!str true: a\n!!bool true: b\n").is_empty());
        assert!(duplicate_lines("!!str 1: a\n1: b\n").is_empty());
        assert_eq!(duplicate_lines("!!str 1: a\n\"1\": b\n"), [2]);
    }

    // `resolveBlockScalar` keeps the breaks around more-indented lines of a
    // folded scalar, and keeps leading blank lines. Failure mode: folding
    // them to spaces made distinct keys equal.
    #[test]
    fn folded_keys_keep_breaks_around_more_indented_lines() {
        assert!(duplicate_lines("? >-\n  a\n    b\n: one\n\"a   b\": two\n").is_empty());
        assert_eq!(duplicate_lines("? >-\n  a\n    b\n: one\n\"a\\n  b\": two\n"), [5]);
        assert_eq!(duplicate_lines("? >-\n  a\n  b\n: one\n\"a b\": two\n"), [5]);
        assert_eq!(duplicate_lines("? |-\n\n  a\n: one\n\"\\na\": two\n"), [5]);
    }
}
