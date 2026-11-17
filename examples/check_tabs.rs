//! Run with --check-tabs. Uses the real browser controller in a hidden local test profile.
#[path = "../src/browser.rs"]
mod browser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    browser::run()
}
