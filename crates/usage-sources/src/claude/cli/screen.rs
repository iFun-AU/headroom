//! Bounded terminal emulation and narrowly recognized setup interactions.

use super::CliError;
use std::io::Write;
use usage_core::{Reading, UnixSeconds, parse::parse_claude_cli_usage};

pub(super) struct CaptureScreen {
    parser: vt100::Parser,
    bytes: usize,
    tail: Vec<u8>,
    contents: String,
    trust_answered: bool,
    trust_moved: bool,
}

impl CaptureScreen {
    pub(super) fn new() -> Self {
        Self {
            parser: vt100::Parser::new(60, 160, 0),
            bytes: 0,
            tail: Vec::new(),
            contents: String::new(),
            trust_answered: false,
            trust_moved: false,
        }
    }

    pub(super) fn feed(&mut self, chunk: &[u8], writer: &mut dyn Write) -> Result<bool, CliError> {
        self.bytes += chunk.len();
        if self.bytes > 1_048_576 {
            return Err(CliError::OutputLimit);
        }
        self.parser.process(chunk);
        // Respond to cursor-position queries, including sequences split across reads.
        self.tail.extend_from_slice(chunk);
        if self.tail.windows(4).any(|bytes| bytes == b"\x1b[6n") {
            let (row, col) = self.parser.screen().cursor_position();
            write!(writer, "\x1b[{};{}R", row + 1, col + 1).map_err(|_| CliError::Process)?;
        }
        let keep = self.tail.len().saturating_sub(3);
        self.tail.drain(..keep);
        let next = self.parser.screen().contents();
        if next == self.contents {
            return Ok(false);
        }
        self.contents = next;
        // Recognized failures stop immediately rather than waiting for silence.
        let _ = self.reading()?;
        Ok(true)
    }

    pub(super) fn respond_to_trust(&mut self, writer: &mut dyn Write) -> Result<(), CliError> {
        let compact: String = self
            .contents
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if !compact.contains("quicksafetycheck") {
            return Ok(());
        }
        if self.trust_answered {
            return Ok(());
        }
        let no_selected = compact.contains("❯no,exit") || compact.contains("❯2.no,exit");
        let yes_selected = compact.contains("❯yes,itrustthisfolder")
            || compact.contains("❯1.yes,itrustthisfolder");
        let input = if yes_selected {
            self.trust_answered = true;
            "\r"
        } else if no_selected && !self.trust_moved {
            let (Some(yes), Some(no)) = (
                compact.find("yes,itrustthisfolder"),
                compact.find("no,exit"),
            ) else {
                return Ok(());
            };
            self.trust_moved = true;
            // Select only the known affirmative choice, then wait for its redraw
            // before confirming. Ink can discard Enter coalesced with an arrow.
            if yes < no { "\x1b[A" } else { "\x1b[B" }
        } else {
            return Ok(());
        };
        writer
            .write_all(input.as_bytes())
            .map_err(|_| CliError::Process)?;
        Ok(())
    }

    pub(super) fn reading(&self) -> Result<Option<Reading>, CliError> {
        let now = chrono::Utc::now().timestamp();
        parse_claude_cli_usage(&self.contents, UnixSeconds(now)).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_query_can_span_reads_and_erased_data_is_not_reused() {
        let mut screen = CaptureScreen::new();
        let mut input = Vec::new();
        screen
            .feed(b"Current session\r\n40% used\x1b[", &mut input)
            .unwrap();
        screen.feed(b"6n", &mut input).unwrap();
        assert!(input.ends_with(b"R"));
        assert!(screen.reading().unwrap().is_some());
        screen
            .feed(b"\x1b[2J\x1b[HLoading usage data", &mut input)
            .unwrap();
        assert!(screen.reading().unwrap().is_none());
    }

    #[test]
    fn selects_only_known_trust_choice_once() {
        let mut screen = CaptureScreen::new();
        let mut input = Vec::new();
        screen
            .feed(
                "Quick safety check\r\n❯ No, exit\r\nYes, I trust this folder".as_bytes(),
                &mut input,
            )
            .unwrap();
        screen.respond_to_trust(&mut input).unwrap();
        assert_eq!(input, b"\x1b[B");
        screen.respond_to_trust(&mut input).unwrap();
        assert_eq!(input, b"\x1b[B");
        screen
            .feed(
                "\x1b[2J\x1b[HQuick safety check\r\nNo, exit\r\n❯ Yes, I trust this folder"
                    .as_bytes(),
                &mut input,
            )
            .unwrap();
        screen.respond_to_trust(&mut input).unwrap();
        assert_eq!(input, b"\x1b[B\r");
        screen.respond_to_trust(&mut input).unwrap();
        assert_eq!(input, b"\x1b[B\r");
    }
}
