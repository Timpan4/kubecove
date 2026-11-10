//! String ordering that matches the web frontend's `localeCompare` for the
//! ASCII text Kubernetes names, kinds and layout keys use. Layout positions
//! depend on sort order, so byte order would move nodes.

use std::cmp::Ordering;

/// CLDR root collation order of ASCII punctuation and symbols, which sort
/// before digits and letters.
const SYMBOLS: &[u8] = b"_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

/// `SYMBOL_RANK[byte]` is one more than the byte's index in `SYMBOLS`, or zero.
const SYMBOL_RANK: [u8; 128] = {
    let mut rank = [0; 128];
    let mut index = 0;
    while index < SYMBOLS.len() {
        rank[SYMBOLS[index] as usize] = index as u8 + 1;
        index += 1;
    }
    rank
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Unit<'a> {
    Space,
    Symbol(u8),
    /// A digit run under numeric collation compares by value: leading zeros
    /// are dropped, then the longer run is larger.
    Number(usize, &'a str),
    Digit(u8),
    Letter(u8),
    Other(char),
}

/// The collation units of a string, produced lazily so a comparison
/// allocates nothing and stops at the first difference.
struct Units<'a> {
    rest: &'a str,
    numeric: bool,
}

impl<'a> Iterator for Units<'a> {
    type Item = Unit<'a>;

    fn next(&mut self) -> Option<Unit<'a>> {
        let c = self.rest.chars().next()?;
        if self.numeric && c.is_ascii_digit() {
            let end = self
                .rest
                .bytes()
                .position(|byte| !byte.is_ascii_digit())
                .unwrap_or(self.rest.len());
            let digits = self.rest[..end].trim_start_matches('0');
            self.rest = &self.rest[end..];
            return Some(Unit::Number(digits.len(), digits));
        }
        self.rest = &self.rest[c.len_utf8()..];
        let symbol = if c.is_ascii() {
            SYMBOL_RANK[c as usize]
        } else {
            0
        };
        Some(if c.is_ascii_whitespace() {
            Unit::Space
        } else if symbol > 0 {
            Unit::Symbol(symbol)
        } else if c.is_ascii_digit() {
            Unit::Digit(c as u8)
        } else if c.is_ascii_alphabetic() {
            Unit::Letter(c.to_ascii_lowercase() as u8)
        } else {
            Unit::Other(c)
        })
    }
}

fn units(text: &str, numeric: bool) -> Units<'_> {
    Units {
        rest: text,
        numeric,
    }
}

/// A sort key materializes the primary units once per row. Case and byte
/// tie-breaks still read the original string, and only run for equal primaries.
pub struct Key<'a> {
    primary: Vec<u8>,
    text: &'a str,
    numeric: bool,
}

/// Equality follows `Ord`, so numeric keys that differ only in leading zeros are equal.
impl PartialEq for Key<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Key<'_> {}

impl<'a> Key<'a> {
    pub fn base_cmp(&self, other: &Self) -> Ordering {
        self.primary.cmp(&other.primary)
    }
    pub fn full_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
    pub fn new(text: &'a str, numeric: bool) -> Self {
        let mut primary = Vec::with_capacity(text.len() * 2);
        append_units(&mut primary, text, numeric);
        Self {
            primary,
            text,
            numeric,
        }
    }
}

/// A nonnumeric layout key. Joining encoded segments preserves the collation
/// units and reuses the destination's buffers instead of decoding each prefix.
#[derive(Default, Eq, PartialEq)]
pub(crate) struct Encoded {
    primary: Vec<u8>,
    pub(crate) text: String,
}

impl Encoded {
    pub(crate) fn new(text: String) -> Self {
        let primary = Key::new(&text, false).primary;
        Self { primary, text }
    }

    pub(crate) fn join(&mut self, prefix: &Self, depth: i32, local: &Self) {
        use std::fmt::Write;
        self.text.clear();
        self.text.push_str(&prefix.text);
        let start = self.text.len();
        write!(&mut self.text, ">{depth:02}>").expect("writing to a String");
        self.primary.clear();
        self.primary.extend_from_slice(&prefix.primary);
        // Only the fixed separator and integer depth need decoding here.
        append_units(&mut self.primary, &self.text[start..], false);
        self.text.push_str(&local.text);
        self.primary.extend_from_slice(&local.primary);
    }
}

impl Ord for Encoded {
    fn cmp(&self, other: &Self) -> Ordering {
        self.primary
            .cmp(&other.primary)
            .then_with(|| tie(&self.text, &other.text, false))
    }
}

impl PartialOrd for Encoded {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn tie(a: &str, b: &str, numeric: bool) -> Ordering {
    let uppercase = |c: char| c.is_ascii_uppercase();
    significant(a, numeric)
        .map(uppercase)
        .cmp(significant(b, numeric).map(uppercase))
        .then_with(|| significant(a, numeric).cmp(significant(b, numeric)))
}

/// The characters the tie-breaks compare. Numeric collation ignores the
/// leading zeros of a digit run, so `pod2` and `pod02` are equal names, as
/// with `Intl.Collator(undefined, { numeric: true })`.
fn significant(text: &str, numeric: bool) -> impl Iterator<Item = char> + '_ {
    let mut in_number = false;
    text.chars().filter(move |&c| {
        let leading_zero = numeric && c == '0' && !in_number;
        in_number = c.is_ascii_digit() && !leading_zero;
        !leading_zero
    })
}

fn append_units(primary: &mut Vec<u8>, text: &str, numeric: bool) {
    for unit in units(text, numeric) {
        match unit {
            Unit::Space => primary.push(0),
            Unit::Symbol(rank) => primary.extend([1, rank]),
            Unit::Number(length, digits) => {
                primary.push(2);
                primary.extend(length.to_be_bytes());
                primary.extend(digits.bytes());
            }
            Unit::Digit(byte) => primary.extend([3, byte]),
            Unit::Letter(byte) => primary.extend([4, byte]),
            Unit::Other(character) => {
                primary.push(5);
                primary.extend(u32::from(character).to_be_bytes());
            }
        }
    }
}

impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.primary
            .cmp(&other.primary)
            .then_with(|| tie(self.text, other.text, self.numeric))
    }
}

impl PartialOrd for Key<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compares like `a.localeCompare(b, undefined, { numeric })`: letters first
/// ignore case, then lowercase sorts before uppercase.
pub fn collate(a: &str, b: &str, numeric: bool) -> Ordering {
    Key::new(a, numeric).full_cmp(&Key::new(b, numeric))
}
