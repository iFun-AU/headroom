//! Claude Code status-line bridge executable.

use std::io;

fn main() {
    if let Ok(Some(mut output)) = statusline_bridge::run(std::env::args_os().skip(1), io::stdin()) {
        print_output(&mut output);
    }
}

#[allow(clippy::print_stdout)]
fn print_output(output: &mut statusline_bridge::BridgeOutput) {
    let _ = output.write_to(&mut io::stdout().lock());
}
