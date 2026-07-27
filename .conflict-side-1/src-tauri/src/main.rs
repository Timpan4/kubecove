// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::all, clippy::pedantic, clippy::style)]
#![allow(clippy::missing_errors_doc)]

fn main() {
    kubecove_lib::run();
}
