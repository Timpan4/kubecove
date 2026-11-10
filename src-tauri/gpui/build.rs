//! Writes `$OUT_DIR/target_names.rs`, the static names the interaction recorder
//! may print (see the module comment in `src/targets.rs`). A name is a string
//! literal's text in `src/` up to its first `:` or `{`, kept when it is only
//! ASCII lowercase letters, digits and `-`.
use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=src");
    let mut names = Vec::new();
    collect(Path::new("src"), &mut names);
    names.sort_unstable();
    names.dedup();
    let mut table = String::from("&[\n");
    for name in names {
        table += &format!("    \"{name}\",\n");
    }
    table += "]\n";
    let out_dir = env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR");
    fs::write(Path::new(&out_dir).join("target_names.rs"), table).expect("write the table");
}

fn collect(dir: &Path, names: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read a source directory") {
        let path = entry.expect("read a source directory entry").path();
        if path.is_dir() {
            collect(&path, names);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path).expect("read a source file");
            for literal in literals(&source) {
                let name = literal.split([':', '{']).next().unwrap_or_default();
                if is_static_name(name) {
                    names.push(name.to_owned());
                }
            }
        }
    }
}

fn is_static_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// The text of each `"`-delimited literal. A backslash skips the next byte,
/// except in raw strings, which end at a quote followed by their opening `#`s.
/// The char literals `'"'` and `'\X'` are stepped over so their quote does not
/// open a literal. Line comments are skipped, so a lone quote in prose cannot
/// pair with the quote that opens the next literal.
fn literals(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += bytes[index..]
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .unwrap_or(bytes.len() - index);
            }
            b'\'' if matches!(bytes.get(index + 1), Some(b'"' | b'\\')) => index += 3,
            b'"' => {
                let hashes = bytes[..index]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte == b'#')
                    .count();
                let raw = index > hashes && bytes[index - hashes - 1] == b'r';
                let closing = if raw { hashes } else { 0 };
                let mut end = index + 1;
                loop {
                    match bytes.get(end) {
                        None => break,
                        Some(b'\\') if !raw => end += 2,
                        Some(b'"') if hashes_at(bytes, end + 1, closing) => break,
                        Some(_) => end += 1,
                    }
                }
                let end = end.min(bytes.len());
                literals.push(&source[index + 1..end]);
                index = end + 1 + closing;
            }
            _ => index += 1,
        }
    }
    literals
}

/// Whether `count` `#`s start at `at`.
fn hashes_at(bytes: &[u8], at: usize, count: usize) -> bool {
    bytes
        .get(at..at + count)
        .is_some_and(|tail| tail.iter().all(|&byte| byte == b'#'))
}
