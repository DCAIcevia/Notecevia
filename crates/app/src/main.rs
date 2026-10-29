// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(rustdoc::broken_intra_doc_links)]
#![forbid(unsafe_code)]

fn main() {
    notecevia_lib::run()
}
