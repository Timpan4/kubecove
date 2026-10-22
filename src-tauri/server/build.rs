#[cfg(feature = "web-ui")]
fn main() -> std::io::Result<()> {
    use std::{fmt::Write as _, fs, io::Write, path::PathBuf};

    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest"))
        .join("../../dist");
    println!("cargo:rerun-if-changed={}", root.display());
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output"));
    let mut directories = vec![root.join("assets")];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            } else if entry.file_type()?.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    let mut source = String::from(
        // Uncompressible files intentionally have identical table entries.
        "#[allow(clippy::match_same_arms)]\nfn compressed_asset(path: &str) -> Option<CompressedAsset> { Some(match path {\n",
    );
    for (index, file) in files.iter().enumerate() {
        let stem = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        // Vite's default [name]-[hash].[ext] uses eight base64url hash bytes.
        let suffix = stem
            .as_bytes()
            .get(stem.len().saturating_sub(9)..)
            .unwrap_or_default();
        if suffix.len() != 9
            || suffix[0] != b'-'
            || !suffix[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            continue;
        }
        let data = fs::read(file)?;
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&data)?;
        let gzip = gzip.finish()?;
        let mut brotli = Vec::new();
        brotli::BrotliCompress(
            &mut &data[..],
            &mut brotli,
            &brotli::enc::BrotliEncoderParams::default(),
        )?;
        let path = file
            .strip_prefix(&root)
            .expect("asset under dist")
            .components()
            .map(|part| part.as_os_str().to_str().expect("UTF-8 asset path"))
            .collect::<Vec<_>>()
            .join("/");
        write!(source, "{path:?} => CompressedAsset {{").expect("write to String");
        for (encoding, compressed) in [("gzip", gzip), ("brotli", brotli)] {
            if compressed.len() < data.len() {
                let destination = output.join(format!("{index}.{encoding}"));
                fs::write(&destination, compressed)?;
                let destination = destination.to_str().expect("UTF-8 Cargo output");
                write!(source, "{encoding}: Some(include_bytes!({destination:?})),")
                    .expect("write to String");
            } else {
                write!(source, "{encoding}: None,").expect("write to String");
            }
        }
        source.push_str("},\n");
    }
    source.push_str("_ => return None, }) }\n");
    fs::write(output.join("compressed_assets.rs"), source)
}

#[cfg(not(feature = "web-ui"))]
fn main() {}
