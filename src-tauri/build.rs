#![deny(clippy::all, clippy::pedantic, clippy::style)]
#![allow(clippy::missing_errors_doc)]

fn main() {
    tauri_build::build();
}
