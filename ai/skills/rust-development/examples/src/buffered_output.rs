//! Buffered output with explicit completion.

use std::io::{self, BufWriter, Write};

/// Writes lines with trailing newlines and flushes the output.
///
/// # Errors
/// Returns a write or flush error. Some output may already have been written.
/// A successful flush does not, by itself, guarantee durable storage.
pub fn write_lines(output: impl Write, lines: &[&str]) -> io::Result<()> {
    let mut output = BufWriter::new(output);
    for line in lines {
        writeln!(output, "{line}")?;
    }
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::write_lines;
    use std::io::{self, Write};

    #[test]
    fn writes_lines_with_newlines() {
        let mut output = Vec::new();

        write_lines(&mut output, &["build passed", "tests passed"])
            .expect("writing to a byte buffer succeeds");

        assert_eq!(output, b"build passed\ntests passed\n");
    }

    #[test]
    fn reports_flush_failure() {
        let error = write_lines(RejectFlush, &["build passed"])
            .expect_err("the test writer rejects flushes");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    struct RejectFlush;

    impl Write for RejectFlush {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "output disconnected",
            ))
        }
    }
}
