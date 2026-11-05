//! String ordering that matches the web frontend's `localeCompare` for the
//! ASCII text Kubernetes names, kinds and layout keys use. Layout positions
//! depend on sort order, so byte order would move nodes.

use std::cmp::Ordering;

/// CLDR root collation order of ASCII punctuation and symbols, which sort
/// before digits and letters.
const SYMBOLS: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Unit<'a> {
    Space,
    Symbol(usize),
    /// A digit run under numeric collation compares by value: leading zeros
    /// are dropped, then the longer run is larger.
    Number(usize, &'a str),
    Digit(u8),
    Letter(u8),
    Other(char),
}

fn units(text: &str, numeric: bool) -> Vec<Unit<'_>> {
    let mut units = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if numeric && c.is_ascii_digit() {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let digits = rest[..end].trim_start_matches('0');
            units.push(Unit::Number(digits.len(), digits));
            rest = &rest[end..];
            continue;
        }
        units.push(if c.is_ascii_whitespace() {
            Unit::Space
        } else if let Some(index) = SYMBOLS.find(c) {
            Unit::Symbol(index)
        } else if c.is_ascii_digit() {
            Unit::Digit(c as u8)
        } else if c.is_ascii_alphabetic() {
            Unit::Letter(c.to_ascii_lowercase() as u8)
        } else {
            Unit::Other(c)
        });
        rest = &rest[c.len_utf8()..];
    }
    units
}

/// A text's collation units, built once so that sorting compares keys without
/// allocating.
pub struct Key<'a> {
    units: Vec<Unit<'a>>,
    text: &'a str,
}

impl<'a> Key<'a> {
    pub fn new(text: &'a str, numeric: bool) -> Self {
        Self {
            units: units(text, numeric),
            text,
        }
    }

    /// `Intl.Collator` with `sensitivity: "base"`: case-only differences compare equal.
    pub fn base_cmp(&self, other: &Self) -> Ordering {
        self.units.cmp(&other.units)
    }

    /// Like `collate`: equal letters break ties on case, lowercase first, then bytes.
    pub fn full_cmp(&self, other: &Self) -> Ordering {
        self.base_cmp(other)
            .then_with(|| {
                self.text
                    .chars()
                    .map(|c| c.is_ascii_uppercase())
                    .cmp(other.text.chars().map(|c| c.is_ascii_uppercase()))
            })
            .then_with(|| self.text.cmp(other.text))
    }
}

/// Compares like `a.localeCompare(b, undefined, { numeric })`: letters first
/// ignore case, then lowercase sorts before uppercase.
pub fn collate(a: &str, b: &str, numeric: bool) -> Ordering {
    Key::new(a, numeric).full_cmp(&Key::new(b, numeric))
}
