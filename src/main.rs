#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    browser::run()
}
