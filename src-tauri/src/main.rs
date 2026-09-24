//! Native process entry point for Headroom.

fn main() -> tauri::Result<()> {
    headroom::run()
}
