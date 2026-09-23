//! Native process entry point for How Is It.

fn main() -> tauri::Result<()> {
    how_is_it::run()
}
