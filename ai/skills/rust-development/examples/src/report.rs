//! Report data, headings, and output.

use std::io::{self, Write};

/// The person credited on a report.
#[derive(Debug, Clone)]
pub struct Author {
    /// The name printed in the heading.
    pub name: String,
}

/// A report title and its author.
#[derive(Debug, Clone)]
pub struct Report {
    title: String,
    author: Author,
}

impl Report {
    /// Creates a report with the supplied title and author.
    pub fn new(title: String, author: Author) -> Self {
        Self { title, author }
    }

    /// Returns the title without copying its text.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_skill_examples::report::{Author, Report};
    ///
    /// let author = Author { name: String::from("Avery") };
    /// let report = Report::new(String::from("Build summary"), author);
    /// assert_eq!(report.title(), "Build summary");
    /// ```
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the title, taking ownership of the supplied text.
    pub fn rename(&mut self, title: String) {
        self.title = title;
    }

    /// Consumes the report and returns its title.
    pub fn into_title(self) -> String {
        self.title
    }

    /// Formats the title and author for display.
    pub fn heading(&self) -> String {
        format!("{} - {}", self.title, self.author.name)
    }
}

/// Writes a report heading followed by a newline.
///
/// # Errors
/// Returns a writer error if the heading cannot be written completely.
/// Some output may already have been written when this function fails.
pub fn write_report(report: &Report, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "{}", report.heading())
}

/// Supplies a short description for the summary output.
pub trait Summary {
    /// Returns the text to show for this item.
    fn summary(&self) -> String;
}

impl Summary for Report {
    fn summary(&self) -> String {
        self.heading()
    }
}

/// The target and outcome of a build.
#[derive(Debug)]
pub struct BuildResult {
    /// The target that was built.
    pub target: String,
    /// Whether the build succeeded.
    pub passed: bool,
}

impl Summary for BuildResult {
    fn summary(&self) -> String {
        let status = if self.passed { "passed" } else { "failed" };
        format!("{}: {status}", self.target)
    }
}

/// Writes an item's summary followed by a newline.
///
/// # Errors
/// Returns a writer error. Output may be incomplete on failure.
pub fn write_summary(item: &impl Summary, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "{}", item.summary())
}

/// Writes summaries of borrowed items that may have different concrete types.
///
/// # Errors
/// Returns a writer error. Earlier summaries may already have been written.
pub fn write_summaries(items: &[&dyn Summary], output: &mut impl Write) -> io::Result<()> {
    for item in items {
        writeln!(output, "{}", item.summary())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{write_report, write_summaries, write_summary, Author, BuildResult, Report};

    #[test]
    fn writes_report_to_buffer() {
        let author = Author {
            name: String::from("Avery"),
        };
        let report = Report::new(String::from("Build summary"), author);
        let mut output = Vec::new();

        write_report(&report, &mut output).expect("writing to a byte buffer succeeds");

        assert_eq!(output, b"Build summary - Avery\n");
    }

    #[test]
    fn renaming_clone_leaves_original_unchanged() {
        let author = Author {
            name: String::from("Avery"),
        };
        let original = Report::new(String::from("Build summary"), author);
        let mut draft = original.clone();

        draft.rename(String::from("Revised summary"));

        assert_eq!(original.title(), "Build summary");
        assert_eq!(draft.into_title(), "Revised summary");
    }

    #[test]
    fn writes_build_summary() {
        let build = BuildResult {
            target: String::from("sensor"),
            passed: true,
        };
        let mut output = Vec::new();

        write_summary(&build, &mut output).expect("writing to a byte buffer succeeds");

        assert_eq!(output, b"sensor: passed\n");
    }

    #[test]
    fn writes_mixed_summaries() {
        let report = Report::new(
            String::from("Build summary"),
            Author {
                name: String::from("Avery"),
            },
        );
        let build = BuildResult {
            target: String::from("sensor"),
            passed: false,
        };
        let mut output = Vec::new();

        write_summaries(&[&report, &build], &mut output)
            .expect("writing to a byte buffer succeeds");

        assert_eq!(output, b"Build summary - Avery\nsensor: failed\n");
    }
}
