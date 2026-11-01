//! This module finds the style issues on a branch's changed lines, by the
//! rules of the `code-documentation` skill. Each language lists its own
//! rules, so this file never checks which language a file is in. Read the
//! `StyleCheck::new(approvals, at_base)` and
//! `StyleCheck::find_file_style_issues(now, changed)` functions to understand
//! the module.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::approvals::{Approvals, DecisionMatch};
use super::comments::{self, Comment, CommentStyle, Definition, FileComments, NameCheck};

/// Holds what every changed file's style check shares, built once per check.
pub(crate) struct StyleCheck<'a> {
    approvals: &'a Approvals,
    /// Holds a whole-word pattern for each word in the `avoid` list, with what
    /// to say instead.
    avoided: Vec<(Regex, String)>,
    at_base: TextAtBase,
}

impl<'a> StyleCheck<'a> {
    /// Builds the avoided-word patterns once, for every file the check reads.
    pub(crate) fn new(approvals: &'a Approvals, at_base: TextAtBase) -> Self {
        Self {
            approvals,
            avoided: build_avoided_word_patterns(approvals.avoided_words()),
            at_base,
        }
    }

    /// Returns the style findings on one file's changed lines, where `changed`
    /// lists the lines Git's diff marks in `now`. Skips a generated file.
    pub(crate) fn find_file_style_issues(
        &self,
        now: &FileComments,
        changed: &BTreeSet<usize>,
    ) -> Vec<Finding> {
        // A generated file changes only when its generator runs again.
        if now.is_generated() {
            return Vec::new();
        }
        let rules = comments::find_style_rules(&now.file);
        let mut findings = Vec::new();
        for comment in &now.comments {
            if (comment.line..=comment.end_line).any(|line| changed.contains(&line)) {
                findings.extend(self.find_comment_style_issues(comment, now, rules));
            }
        }
        for definition in &now.list_definitions() {
            findings.extend(self.find_definition_style_issues(definition, now, changed, rules));
        }
        findings.sort_by_key(|finding| finding.line);
        findings
    }

    /// Returns the findings in one changed comment.
    fn find_comment_style_issues(
        &self,
        comment: &Comment,
        now: &FileComments,
        rules: &[Rule],
    ) -> Vec<Finding> {
        let decision = self.approvals.compare(comment);
        // Changed approved text already fails, and the agent must put it back as it was.
        if decision == DecisionMatch::DiffersFromApproved {
            return Vec::new();
        }
        let is_approved = decision == DecisionMatch::Approved;
        let paragraphs = comments::list_paragraphs(comment);
        let sentences = split_sentences(&paragraphs.join(" "));
        let name = comment.name.as_deref().map_or("", strip_qualifier);
        let mut broken: Vec<RuleMatch> = Vec::new();
        if !is_approved {
            broken.extend(find_restated_name(&sentences, name));
        }
        broken.extend(find_avoided_words(&sentences, &self.avoided));
        broken.extend(find_filler(&sentences, name));
        broken.extend(find_name_at_start(&sentences, name));
        if matches!(
            comment.style,
            CommentStyle::Line | CommentStyle::Hash | CommentStyle::AfterCode
        ) {
            let unmarked = list_unmarked_sentences(comment, &paragraphs);
            broken.extend(find_long_comment(&unmarked));
            broken.extend(find_colons(&unmarked));
        }
        // A language turns a rule on by listing it.
        broken.retain(|found| rules.contains(&found.rule));
        let mut findings = Vec::new();
        for RuleMatch {
            rule,
            sentences: found,
            detail,
        } in broken
        {
            let existing = self.at_base.has_sentences(&comment.file, &found);
            // An old sentence may be a person's words, so it is never called a restatement.
            if existing && rule == Rule::RestatedName {
                continue;
            }
            let outcome = if is_approved {
                Outcome::Reported(ReportNote::Approved)
            } else if now.is_header(comment) {
                Outcome::Reported(ReportNote::Header)
            } else if existing {
                Outcome::Reported(ReportNote::Existing)
            } else if rule == Rule::RestatedName {
                Outcome::Reported(ReportNote::ReportOnly)
            } else {
                Outcome::Fails
            };
            findings.push(Finding {
                file: comment.file.clone(),
                name: comment.name.clone(),
                line: comment.line,
                rule,
                outcome,
                detail,
            });
        }
        findings
    }

    /// Returns the findings in a new name, and in the docstring of a definition
    /// whose own lines changed.
    fn find_definition_style_issues(
        &self,
        definition: &Definition,
        now: &FileComments,
        changed: &BTreeSet<usize>,
        rules: &[Rule],
    ) -> Vec<Finding> {
        let mut broken: Vec<(Rule, Outcome, Option<String>)> = Vec::new();
        if definition.name_check != NameCheck::Skip
            && !self.at_base.has_name(&now.file, &definition.name)
        {
            let words = split_name_words(strip_qualifier(&definition.name));
            if words.len() > 7 {
                broken.push((
                    Rule::SevenWords,
                    Outcome::Fails,
                    Some(format!("{} words", words.len())),
                ));
            }
            if let Some(first_word) = words.first()
                && definition.name_check == NameCheck::WordCountAndVerb
                && !is_verb(first_word)
            {
                let detail = format!("\"{first_word}\" is not in the checker's verbs");
                let outcome = Outcome::Reported(ReportNote::ReportOnly);
                broken.push((Rule::VerbFirst, outcome, Some(detail)));
            }
        }
        if let Some(docstring) = &definition.docstring
            && !docstring.own_lines.is_disjoint(changed)
        {
            let comment = now.comments.iter().find(|comment| {
                comment.style == CommentStyle::Docstring
                    && comment.name.as_deref() == Some(definition.name.as_str())
            });
            let decision = comment.map(|comment| self.approvals.compare(comment));
            let outcome = match (comment, &decision) {
                (_, Some(DecisionMatch::Approved)) => Outcome::Reported(ReportNote::Approved),
                (Some(comment), _)
                    if self.at_base.has_sentences(
                        &now.file,
                        &split_sentences(&comments::list_paragraphs(comment).join(" ")),
                    ) =>
                {
                    Outcome::Reported(ReportNote::Existing)
                }
                _ => Outcome::Fails,
            };
            match &docstring.text {
                None if docstring.is_required => {
                    // Every line of a moved file counts as changed, so a function a deleted
                    // changed file held at `base` is old.
                    let is_moved = !self.at_base.has_file(&now.file)
                        && self.at_base.has_gone_name(&now.file, &definition.name);
                    let outcome = if is_moved {
                        Outcome::Reported(ReportNote::Existing)
                    } else {
                        Outcome::Fails
                    };
                    broken.push((Rule::Docstring, outcome, None));
                }
                None => {}
                // Changed approved text already fails, and the agent must put it back as it was.
                Some(_) if decision == Some(DecisionMatch::DiffersFromApproved) => {}
                Some(text) => {
                    for (rule, detail) in find_docstring_rules(text, docstring.is_test) {
                        broken.push((rule, outcome, detail));
                    }
                }
            }
        }
        // A language turns a rule on by listing it.
        broken.retain(|(rule, ..)| rules.contains(rule));
        broken
            .into_iter()
            .map(|(rule, outcome, detail)| Finding {
                file: now.file.clone(),
                name: Some(definition.name.clone()),
                line: definition.line,
                rule,
                outcome,
                detail,
            })
            .collect()
    }
}

/// Holds one comment or name that breaks a rule.
#[derive(Debug, PartialEq)]
pub struct Finding {
    pub file: PathBuf,
    pub name: Option<String>,
    pub line: usize,
    pub rule: Rule,
    pub outcome: Outcome,
    /// Names what to fix where the rule alone does not, such as the filler
    /// phrase, or `None`.
    pub detail: Option<String>,
}

/// Names each rule a comment or name can break.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rule {
    /// Marks a changed comment whose name has approved texts, none equal to its text.
    Approved,
    /// Marks an approved comment the branch removed while its code stays.
    ApprovedRemoved,
    Filler,
    /// Marks a word from the `avoid` list in the approvals file.
    AvoidedWord,
    RestatedName,
    StartsWithName,
    CommentLength,
    CommentColon,
    SevenWords,
    VerbFirst,
    Docstring,
    SummaryLine,
    TestWhy,
    SectionHeading,
}

impl Rule {
    /// Returns the `code-documentation` sentence the rule checks, word for word.
    pub fn sentence(self) -> &'static str {
        match self {
            Self::Approved | Self::ApprovedRemoved => {
                "On a later pass over unchanged behavior, edit existing prose only to fix a specific false or unsupported claim, an omission that makes it misleading for safe use, or an applicable convention violation; otherwise leave it byte-for-byte unchanged."
            }
            Self::Filler => "Avoid vague filler such as:",
            Self::AvoidedWord => {
                "Explain project-specific terms when a new contributor would not know them."
            }
            Self::RestatedName => {
                "Prose derived directly from a name, signature, single statement, or a test's name, inputs, and assertions is a restatement and must not be added."
            }
            Self::StartsWithName => "Do not start a doc comment with the name it documents",
            Self::CommentLength => "Keep each `//` or `#` comment to at most two sentences.",
            Self::CommentColon => {
                "Use no colon in a `//` or `#` comment, except inside code or a URL."
            }
            Self::SevenWords => "Every function name must contain at most seven words.",
            Self::VerbFirst => {
                "Start every function and method name with a verb, except a test, a fixture or a property."
            }
            Self::Docstring => {
                "Give every function, method and class a docstring, tests and fixtures included."
            }
            Self::SummaryLine => {
                "Start each docstring with a one-line summary that fits on its first line."
            }
            Self::TestWhy => "Follow a test's summary with a paragraph on why the test matters.",
            Self::SectionHeading => {
                "Never invent a docstring section, such as `Why it matters:`; write a plain paragraph instead."
            }
        }
    }
}

/// Says whether a finding fails the branch or is only reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Fails,
    Reported(ReportNote),
}

/// Says why a finding is only reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportNote {
    /// Marks text that matches an approval, which the agent may not change.
    Approved,
    /// Marks a sentence or function a changed file in the same language held at `base`.
    Existing,
    /// Marks a license header above a file's first code.
    Header,
    /// Marks a rule that never fails.
    ReportOnly,
}

/// Lists the skill's filler examples and their other word forms.
const FILLER: [&str; 10] = [
    "facilitate",
    "facilitated",
    "facilitates",
    "facilitating",
    "is responsible for",
    "are responsible for",
    "provides a mechanism for",
    "provides a robust mechanism",
    "seamless",
    "seamlessly",
];

/// Lists the verbs a function name may start with, as today's checker does.
const VERBS: &str = "
    accept add allow answer append apply approve archive arm ask assert
    assign attach audit build bump calculate call cancel capture change
    check choose clean clear clone close collect combine commit compare
    compile complete compute configure confirm connect consume convert copy
    count create cut decide decode delete deliver describe detect disable
    discard dispatch download drop dump edit emit enable encode ensure
    enter escape evaluate exit expand expect explain export extract fail
    fetch fill filter find finish fix flush format freeze generate get give
    group guard handle hash help hide import include increment index init
    insert inspect install is iter join keep kill label launch list load lock
    log look looks make map mark match measure merge migrate mount move name
    normalize notify open order pack pad park parse pass patch pause pick pin
    plan poll pop post prepare preview print process produce prompt prune
    publish pull push put queue quote raise read receive record reduce
    refresh refund refuse register reject release reload remove rename
    render reorder repeat replace reply report request require reserve
    reset resolve restore resume retry return reverse review rewrite round
    run sanitize save scan schedule score scout search seed select send serve
    set setup show shrink skip sleep sort split start stash stop store strip
    submit subscribe summarize swap switch sync take teardown tell test
    trace track transform transition translate trim truncate try undo
    unlock unpack update upgrade upload use validate verify view visit wait
    walk warn watch wrap write yield
";

/// Lists the words that start a question or a conversion, such as `to_json`.
const QUESTION_OR_CONVERSION_WORDS: [&str; 8] =
    ["as", "can", "from", "has", "into", "should", "to", "was"];

/// Lists the headings that stand in for the plain paragraph a docstring needs.
const INVENTED_HEADINGS: [&str; 7] = [
    "Background",
    "Context",
    "Purpose",
    "Rationale",
    "Why",
    "Why it matters",
    "Why this matters",
];

static FILLER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    FILLER
        .iter()
        .map(|phrase| build_whole_word_pattern(phrase))
        .collect()
});
static CODE_OR_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]*`|\w+://\S+").expect("the pattern is valid"));
/// Matches an abbreviation whose period does not end a sentence.
static ABBREVIATIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:e\.g|i\.e|etc)\.").expect("the pattern is valid"));

/// Holds the sentences and names the changed files held at `base`, by
/// language, so code moved from one file to another still counts as old.
#[derive(Default)]
pub(crate) struct TextAtBase {
    /// Holds each sentence with its file's language, since a rule's reach
    /// depends on the language.
    sentences: BTreeSet<(&'static str, String)>,
    names: BTreeSet<(&'static str, String)>,
    /// Lists the changed files that were there at `base`.
    files: BTreeSet<PathBuf>,
    /// Holds the definition names of the changed files the branch deleted.
    gone_names: BTreeSet<(&'static str, String)>,
}

impl TextAtBase {
    /// Adds one changed file's text at `base`. `is_gone` is true when the
    /// branch deleted the file.
    pub(crate) fn add(&mut self, before: &FileComments, is_gone: bool) {
        let Some(language) = comments::find_language_name(&before.file) else {
            return;
        };
        if before.exists() {
            self.files.insert(before.file.clone());
        }
        for comment in &before.comments {
            for sentence in split_sentences(&comments::list_paragraphs(comment).join(" ")) {
                self.sentences.insert((language, sentence));
            }
        }
        for definition in &before.list_definitions() {
            let name = (language, definition.name.clone());
            if is_gone {
                self.gone_names.insert(name.clone());
            }
            self.names.insert(name);
        }
    }

    fn has_sentences(&self, file: &Path, sentences: &[String]) -> bool {
        let Some(language) = comments::find_language_name(file) else {
            return false;
        };
        sentences
            .iter()
            .all(|sentence| self.sentences.contains(&(language, sentence.clone())))
    }

    fn has_name(&self, file: &Path, name: &str) -> bool {
        let Some(language) = comments::find_language_name(file) else {
            return false;
        };
        self.names.contains(&(language, name.to_owned()))
    }

    fn has_gone_name(&self, file: &Path, name: &str) -> bool {
        let Some(language) = comments::find_language_name(file) else {
            return false;
        };
        self.gone_names.contains(&(language, name.to_owned()))
    }

    fn has_file(&self, file: &Path) -> bool {
        self.files.contains(file)
    }
}

/// Holds a rule one comment breaks, the sentences that break it, and what to fix.
struct RuleMatch {
    rule: Rule,
    sentences: Vec<String>,
    detail: Option<String>,
}

impl RuleMatch {
    fn new(rule: Rule, sentences: Vec<String>, detail: Option<String>) -> Self {
        Self {
            rule,
            sentences,
            detail,
        }
    }
}

/// Returns the rules a docstring's text breaks.
fn find_docstring_rules(text: &str, is_test: bool) -> Vec<(Rule, Option<String>)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut broken = Vec::new();
    if lines.first().is_none_or(|line| line.trim().is_empty()) {
        broken.push((Rule::SummaryLine, Some("first line is empty".to_owned())));
    } else if lines.get(1).is_some_and(|line| !line.trim().is_empty()) {
        let detail = "summary runs onto a second line".to_owned();
        broken.push((Rule::SummaryLine, Some(detail)));
    }
    if is_test && count_paragraphs(text) < 2 {
        broken.push((Rule::TestWhy, None));
    }
    let invented_heading = lines.iter().skip(1).find_map(|line| {
        line.trim()
            .split_once(':')
            .map(|(heading, _)| heading)
            .filter(|heading| INVENTED_HEADINGS.contains(heading))
    });
    if let Some(heading) = invented_heading {
        broken.push((Rule::SectionHeading, Some(format!("\"{heading}:\""))));
    }
    broken
}

/// Returns how many blocks blank lines split the text into.
fn count_paragraphs(text: &str) -> usize {
    let mut count = 0;
    let mut in_paragraph = false;
    for line in text.lines() {
        let is_blank = line.trim().is_empty();
        if !is_blank && !in_paragraph {
            count += 1;
        }
        in_paragraph = !is_blank;
    }
    count
}

fn find_restated_name(sentences: &[String], name: &str) -> Option<RuleMatch> {
    let first = sentences.first()?;
    if name.is_empty() {
        return None;
    }
    // Prose may use a name's word in lowercase, so only the name's own case counts.
    let name_pattern = Regex::new(&format!(r"\b{}\b", regex::escape(name)));
    if !name_pattern.expect("escaped text is valid").is_match(first) {
        return None;
    }
    Some(RuleMatch::new(
        Rule::RestatedName,
        vec![first.clone()],
        None,
    ))
}

fn find_avoided_words(sentences: &[String], avoided: &[(Regex, String)]) -> Option<RuleMatch> {
    let mut found: Vec<String> = Vec::new();
    let mut details: Vec<&str> = Vec::new();
    for (word, detail) in avoided {
        let matched: Vec<&String> = sentences
            .iter()
            .filter(|sentence| word.is_match(sentence))
            .collect();
        if !matched.is_empty() {
            found.extend(matched.into_iter().cloned());
            details.push(detail);
        }
    }
    let detail = details.join("; ");
    (!found.is_empty()).then(|| RuleMatch::new(Rule::AvoidedWord, found, Some(detail)))
}

/// Finds filler only in sentences about the code, since a sentence such as
/// "The caller is responsible for closing it" states a fact the skill asks for.
fn find_filler(sentences: &[String], name: &str) -> Option<RuleMatch> {
    let about_code: Vec<&String> = sentences
        .iter()
        .filter(|sentence| is_about_code(sentence, name))
        .collect();
    let mut found: Vec<String> = Vec::new();
    let mut phrases: Vec<String> = Vec::new();
    for (phrase, pattern) in FILLER.iter().zip(FILLER_PATTERNS.iter()) {
        let matched: Vec<&&String> = about_code
            .iter()
            .filter(|sentence| pattern.is_match(sentence))
            .collect();
        if !matched.is_empty() {
            found.extend(matched.into_iter().map(|sentence| (*sentence).clone()));
            phrases.push(format!("\"{phrase}\""));
        }
    }
    found.sort();
    found.dedup();
    let detail = phrases.join(", ");
    (!found.is_empty()).then(|| RuleMatch::new(Rule::Filler, found, Some(detail)))
}

/// Returns true for a sentence that starts with "This", the code's name, or
/// "Package" and the name.
fn is_about_code(sentence: &str, name: &str) -> bool {
    let subject = if name.is_empty() {
        r"^This\b".to_owned()
    } else {
        let name = regex::escape(name);
        format!(r"^(?:This|{name}|Package {name})\b")
    };
    Regex::new(&subject)
        .expect("escaped text is valid")
        .is_match(sentence)
}

fn find_name_at_start(sentences: &[String], name: &str) -> Option<RuleMatch> {
    let first = sentences.first()?;
    let first_word = first.split_whitespace().next()?;
    let word = first_word.trim_matches(|c: char| "`'\"(),.:".contains(c));
    let starts_with_name = !name.is_empty() && (word == name || word == format!("{name}()"));
    starts_with_name.then(|| RuleMatch::new(Rule::StartsWithName, vec![first.clone()], None))
}

fn find_long_comment(sentences: &[String]) -> Option<RuleMatch> {
    let prose = CODE_OR_URL
        .replace_all(&sentences.join(" "), "code")
        .into_owned();
    let prose = ABBREVIATIONS.replace_all(&prose, "abbreviation");
    let count = split_sentences(&prose).len();
    let detail = format!("{count} sentences");
    (count > 2).then(|| RuleMatch::new(Rule::CommentLength, sentences.to_vec(), Some(detail)))
}

/// Finds a colon outside code or a URL, except one before a digit as in a time.
fn find_colons(sentences: &[String]) -> Option<RuleMatch> {
    let found: Vec<String> = sentences
        .iter()
        .filter(|sentence| {
            let prose = CODE_OR_URL.replace_all(sentence, "code");
            prose
                .match_indices(':')
                .any(|(index, _)| !prose[index + 1..].starts_with(|c: char| c.is_ascii_digit()))
        })
        .cloned()
        .collect();
    (!found.is_empty()).then(|| RuleMatch::new(Rule::CommentColon, found, None))
}

/// Returns the sentences the length and colon rules read. Python joins every
/// line, as today's checker does, and the other languages leave out the notes
/// they exempt, such as Go's `Deprecated:` paragraph.
fn list_unmarked_sentences(comment: &Comment, paragraphs: &[String]) -> Vec<String> {
    if let Some(prose) = comments::join_unfiltered_lines(comment) {
        return split_sentences(&prose);
    }
    let kept: Vec<&str> = paragraphs
        .iter()
        .map(String::as_str)
        .filter(|paragraph| !comments::is_exempt_note(&comment.file, paragraph))
        .collect();
    split_sentences(&kept.join(" "))
}

/// Splits prose after `.`, `!` or `?` and the spaces that follow.
fn split_sentences(prose: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut chars = prose.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        let ends_sentence = matches!(c, '.' | '!' | '?')
            && chars.peek().is_some_and(|(_, next)| next.is_whitespace());
        if ends_sentence {
            sentences.push(prose[start..=index].to_owned());
            while chars.next_if(|(_, next)| next.is_whitespace()).is_some() {}
            start = chars.peek().map_or(prose.len(), |(next, _)| *next);
        }
    }
    if start < prose.len() {
        sentences.push(prose[start..].to_owned());
    }
    sentences.retain(|sentence| !sentence.is_empty());
    sentences
}

/// Splits a name into words at underscores and case changes, so
/// `parse_HTTPConfig` gives `parse`, `HTTP` and `Config`.
fn split_name_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in name.split('_') {
        let chars: Vec<char> = part.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            let start = index;
            let c = chars[index];
            if c.is_ascii_uppercase() {
                while index < chars.len() && chars[index].is_ascii_uppercase() {
                    index += 1;
                }
                let next_is_lowercase = chars.get(index).is_some_and(char::is_ascii_lowercase);
                // In `HTTPConfig` the last capital starts the next word.
                if next_is_lowercase && index - start > 1 {
                    index -= 1;
                } else if next_is_lowercase {
                    while index < chars.len() && chars[index].is_ascii_lowercase() {
                        index += 1;
                    }
                }
            } else if c.is_ascii_lowercase() {
                while index < chars.len() && chars[index].is_ascii_lowercase() {
                    index += 1;
                }
            } else if c.is_ascii_digit() {
                while index < chars.len() && chars[index].is_ascii_digit() {
                    index += 1;
                }
            } else {
                index += 1;
                continue;
            }
            words.push(chars[start..index].iter().collect());
        }
    }
    words
}

/// Returns true for a known verb or question word, or a verb after `re` or
/// `un`, such as `rebuild`.
fn is_verb(word: &str) -> bool {
    let word = word.to_lowercase();
    let is_known = |word: &str| VERBS.split_whitespace().any(|verb| verb == word);
    is_known(&word)
        || QUESTION_OR_CONVERSION_WORDS.contains(&word.as_str())
        || ["re", "un"]
            .iter()
            .any(|prefix| word.strip_prefix(prefix).is_some_and(is_known))
}

/// Returns the last part of a name, as prose writes it, so `Cart.Total`,
/// `Store::save` and `const Red` give `Total`, `save` and `Red`.
fn strip_qualifier(name: &str) -> &str {
    name.rsplit(['.', ' ', ':']).next().unwrap_or(name)
}

/// Returns a pattern that finds `text` as whole words, in any case.
fn build_whole_word_pattern(text: &str) -> Regex {
    Regex::new(&format!(r"(?i)\b{}\b", regex::escape(text))).expect("escaped text is valid")
}

/// Returns a whole-word pattern for each word in the approvals file's `avoid` list.
fn build_avoided_word_patterns(avoid: &BTreeMap<String, String>) -> Vec<(Regex, String)> {
    avoid
        .iter()
        .map(|(word, instead)| {
            (
                build_whole_word_pattern(word),
                format!("\"{word}\": {instead}"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests the style rules on source text, with no Git clone. The tests that
    //! need Git's diff or a branch sit in `tests/check_comments.rs`.

    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::{Value, json};

    use super::*;
    use crate::shared::approvals::{self, Approvals};
    use crate::shared::git::{FileVersion, ON_DISK};

    /// Names the Go file `find_in_changed_store` reads.
    const STORE: &str = "review_store.go";
    /// Holds the comment above `SaveAnswer` in the store file at `base`.
    const BASE_COMMENT: &str = "// Saves the answer under the lock.\n";
    /// Holds an avoid list with one word, which the avoided-word rows break.
    const AVOID_BLOB: &str = r#"{"approved": [], "avoid": {"blob": "say file"}}"#;
    /// Numbers the temporary folders, so tests that run at once never share one.
    static NEXT_FOLDER: AtomicUsize = AtomicUsize::new(0);

    /// Returns Go source with `comment` above a `SaveAnswer` function whose body is `body`.
    fn store_source(comment: &str, body: &str) -> String {
        format!("package review\n\n{comment}func SaveAnswer() {{{body}}}\n")
    }

    /// Returns the comments in `source`, read as `file` on disk.
    fn read_source(file: &str, source: &str) -> FileComments {
        let version = FileVersion {
            place: ON_DISK,
            text: Some(source.to_owned()),
        };
        comments::read_version_comments(Path::new(file), version).unwrap()
    }

    /// Writes `saved` as an approvals file in a temporary folder and loads it.
    fn load_approvals(saved: Value) -> Approvals {
        let number = NEXT_FOLDER.fetch_add(1, Ordering::SeqCst);
        let folder =
            std::env::temp_dir().join(format!("style-rules-{}-{number}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("code-comments.json");
        fs::write(&path, saved.to_string()).unwrap();
        let approvals = approvals::load_file(&path).unwrap();
        fs::remove_dir_all(&folder).unwrap();
        approvals
    }

    /// Returns each finding's rule, outcome and detail, failures first.
    fn list_rules(findings: Vec<Finding>) -> Vec<(Rule, Outcome, String)> {
        let (failures, reported): (Vec<Finding>, Vec<Finding>) = findings
            .into_iter()
            .partition(|finding| finding.outcome == Outcome::Fails);
        failures
            .into_iter()
            .chain(reported)
            .map(|finding| {
                (
                    finding.rule,
                    finding.outcome,
                    finding.detail.unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Returns the findings, failures first, in a new file holding `source`.
    fn find_in_new_file(file: &str, source: &str) -> Vec<(Rule, Outcome, String)> {
        let approvals = load_approvals(serde_json::from_str(AVOID_BLOB).unwrap());
        let check = StyleCheck::new(&approvals, TextAtBase::default());
        let changed = (1..=source.lines().count()).collect();
        list_rules(check.find_file_style_issues(&read_source(file, source), &changed))
    }

    /// Returns the findings, failures first, when the store file changes from `before` to `now`.
    fn find_in_changed_store(
        approvals: &Approvals,
        before: &str,
        now: &str,
        changed: &[usize],
    ) -> Vec<(Rule, Outcome, String)> {
        let mut at_base = TextAtBase::default();
        at_base.add(&read_source(STORE, before), false);
        let check = StyleCheck::new(approvals, at_base);
        let changed = changed.iter().copied().collect();
        list_rules(check.find_file_style_issues(&read_source(STORE, now), &changed))
    }

    mod find_file_style_issues {
        use super::*;

        /// `find_file_style_issues` must find a break of each style rule and name what broke it.
        /// Otherwise filler, long comments or unclear names an agent writes would reach the user unflagged.
        #[test]
        fn finds_each_style_rule_break() {
            let go = |code: &str| format!("package review\n\n{code}");
            let cases = [
                (
                    "filler",
                    "answer.go",
                    go("// This facilitates saving.\nfunc SaveAnswer() {}\n"),
                    vec![(Rule::Filler, Outcome::Fails, "\"facilitates\"".to_owned())],
                ),
                (
                    "avoided-word",
                    "answer.go",
                    go("// Saves the blob.\nfunc SaveAnswer() {}\n"),
                    vec![(Rule::AvoidedWord, Outcome::Fails, "\"blob\": say file".to_owned())],
                ),
                (
                    "restated-name",
                    "answer.go",
                    go("// Writes the answer, as SaveAnswer says.\nfunc SaveAnswer() {}\n"),
                    vec![(
                        Rule::RestatedName,
                        Outcome::Reported(ReportNote::ReportOnly),
                        String::new(),
                    )],
                ),
                (
                    "starts-with-name",
                    "answer.go",
                    go("// SaveAnswer writes it.\nfunc SaveAnswer() {}\n"),
                    vec![
                        (Rule::StartsWithName, Outcome::Fails, String::new()),
                        (
                            Rule::RestatedName,
                            Outcome::Reported(ReportNote::ReportOnly),
                            String::new(),
                        ),
                    ],
                ),
                (
                    "comment-length",
                    "answer.go",
                    go("// Saves it. Locks it. Logs it.\nfunc SaveAnswer() {}\n"),
                    vec![(Rule::CommentLength, Outcome::Fails, "3 sentences".to_owned())],
                ),
                (
                    "comment-colon",
                    "answer.go",
                    go("// Saves it: now.\nfunc SaveAnswer() {}\n"),
                    vec![(Rule::CommentColon, Outcome::Fails, String::new())],
                ),
                (
                    "comment-colon-inside-function",
                    "answer.go",
                    go("func SaveAnswer() {\n\t// Saves it: now.\n}\n"),
                    vec![(Rule::CommentColon, Outcome::Fails, String::new())],
                ),
                (
                    "seven-words",
                    "answer.go",
                    go("func SaveTheAnswerToTheStoreRightNow() {}\n"),
                    vec![(Rule::SevenWords, Outcome::Fails, "8 words".to_owned())],
                ),
                (
                    "verb-first",
                    "answer.go",
                    go("func Total() {}\n"),
                    vec![(
                        Rule::VerbFirst,
                        Outcome::Reported(ReportNote::ReportOnly),
                        "\"Total\" is not in the checker's verbs".to_owned(),
                    )],
                ),
                (
                    "docstring",
                    "answer.py",
                    "def save_answer():\n    return 1\n".to_owned(),
                    vec![(Rule::Docstring, Outcome::Fails, String::new())],
                ),
                (
                    "docstring-on-init",
                    "answer.py",
                    "class Store:\n    \"\"\"Holds answers.\"\"\"\n\n    def __init__(self):\n        pass\n"
                        .to_owned(),
                    vec![(Rule::Docstring, Outcome::Fails, String::new())],
                ),
                (
                    "summary-line",
                    "answer.py",
                    "def save_answer():\n    \"\"\"Saves\n    the answer.\"\"\"\n".to_owned(),
                    vec![(
                        Rule::SummaryLine,
                        Outcome::Fails,
                        "summary runs onto a second line".to_owned(),
                    )],
                ),
                (
                    "test-why",
                    "answer.py",
                    "def test_saves_answer():\n    \"\"\"Saves the answer.\"\"\"\n".to_owned(),
                    vec![(Rule::TestWhy, Outcome::Fails, String::new())],
                ),
                (
                    "section-heading",
                    "answer.py",
                    "def save_answer():\n    \"\"\"Saves the answer.\n\n    Why: it is lost.\n    \"\"\"\n"
                        .to_owned(),
                    vec![(Rule::SectionHeading, Outcome::Fails, "\"Why:\"".to_owned())],
                ),
                (
                    "python-comment-with-example",
                    "answer.py",
                    "# Saves it. Logs it.\n#\n#   example: 1\n#   example: 2\nLIMIT = 1\n".to_owned(),
                    vec![
                        (Rule::CommentLength, Outcome::Fails, "3 sentences".to_owned()),
                        (Rule::CommentColon, Outcome::Fails, String::new()),
                    ],
                ),
                (
                    "rust-filler",
                    "lib.rs",
                    "/// This facilitates saving.\npub fn save_answer() {}\n".to_owned(),
                    vec![(Rule::Filler, Outcome::Fails, "\"facilitates\"".to_owned())],
                ),
                (
                    "rust-avoided-word",
                    "lib.rs",
                    "/// Saves the blob.\npub fn save_answer() {}\n".to_owned(),
                    vec![(
                        Rule::AvoidedWord,
                        Outcome::Fails,
                        "\"blob\": say file".to_owned(),
                    )],
                ),
                (
                    "rust-restated-name",
                    "lib.rs",
                    "/// Writes the answer, as save_answer says.\npub fn save_answer() {}\n".to_owned(),
                    vec![(
                        Rule::RestatedName,
                        Outcome::Reported(ReportNote::ReportOnly),
                        String::new(),
                    )],
                ),
                (
                    "rust-starts-with-name",
                    "lib.rs",
                    "/// save_answer writes it.\npub fn save_answer() {}\n".to_owned(),
                    vec![
                        (Rule::StartsWithName, Outcome::Fails, String::new()),
                        (
                            Rule::RestatedName,
                            Outcome::Reported(ReportNote::ReportOnly),
                            String::new(),
                        ),
                    ],
                ),
            ];

            for (case, file, source, expected) in cases {
                assert_eq!(find_in_new_file(file, &source), expected, "{case}");
            }
        }

        /// `find_file_style_issues` must only report a style break in a comment that matches the user's approval.
        /// Otherwise an agent would be stuck on text it may not change.
        #[test]
        fn only_reports_approved_text_style_breaks() {
            let filler = "// This facilitates saving.\n";
            let decision = json!({"file": STORE, "name": "SaveAnswer", "text": filler});
            let approvals = load_approvals(json!({"approved": [decision], "avoid": {}}));

            let found = find_in_changed_store(
                &approvals,
                &store_source(BASE_COMMENT, ""),
                &store_source(filler, ""),
                &[3],
            );

            let expected = (
                Rule::Filler,
                Outcome::Reported(ReportNote::Approved),
                "\"facilitates\"".to_owned(),
            );
            assert_eq!(found, [expected]);
        }

        /// `find_file_style_issues` must only report a style break in a sentence already there at `base` or in a license header.
        /// Otherwise an agent would be stuck on an earlier author's text or a license it may not change.
        #[test]
        fn only_reports_old_or_license_style_breaks() {
            let approvals = load_approvals(json!({}));
            let cases = [
                (
                    "sentence-at-base",
                    "// This facilitates saving.\n",
                    store_source("// This facilitates saving. It holds the lock.\n", ""),
                    vec![3],
                    (
                        Rule::Filler,
                        Outcome::Reported(ReportNote::Existing),
                        "\"facilitates\"".to_owned(),
                    ),
                ),
                (
                    "license-header",
                    BASE_COMMENT,
                    format!("// One. Two. Three.\n\n{}", store_source(BASE_COMMENT, "")),
                    vec![1, 2],
                    (
                        Rule::CommentLength,
                        Outcome::Reported(ReportNote::Header),
                        "3 sentences".to_owned(),
                    ),
                ),
            ];

            for (case, base_comment, now, changed, expected) in cases {
                let before = store_source(base_comment, "");

                let found = find_in_changed_store(&approvals, &before, &now, &changed);

                assert_eq!(found, [expected], "{case}");
            }
        }

        /// `find_file_style_issues` must fail an old comment that new sentences make too long.
        /// Otherwise an agent could add sentences to an old comment until it is too long, and the branch would pass.
        #[test]
        fn fails_old_comment_made_too_long() {
            let approvals = load_approvals(json!({}));

            let found = find_in_changed_store(
                &approvals,
                &store_source("// Saves it.\n", ""),
                &store_source("// Saves it. Locks it. Logs it.\n", ""),
                &[3],
            );

            let expected = (
                Rule::CommentLength,
                Outcome::Fails,
                "3 sentences".to_owned(),
            );
            assert_eq!(found, [expected]);
        }

        /// `find_file_style_issues` must find nothing in a file a generator wrote, whatever its comments say.
        /// Otherwise an agent would be told to fix comments that the next run of the generator writes back.
        #[test]
        fn skips_generated_file() {
            let generated = "// Code generated by protoc. DO NOT EDIT.\n\npackage review\n\n// This facilitates saving.\nfunc SaveAnswer() {}\n";

            assert_eq!(find_in_new_file("answer.go", generated), []);
        }

        /// `find_file_style_issues` must read a Python comment without the `# noqa` mark after it.
        /// Otherwise an agent would be told to fix the colon in a mark that a linter reads.
        #[test]
        fn ignores_linter_mark_after_python_comment() {
            let source = "LIMIT = 1  # Counts it.  # noqa: E501\n";

            assert_eq!(find_in_new_file("answer.py", source), []);
        }

        /// `find_file_style_issues` must not run the length and colon rules on Rust comments, which this version checks only in Go and Python.
        /// Otherwise a Rust comment of three sentences or with a colon would fail the branch.
        #[test]
        fn skips_length_and_colon_rules_in_rust() {
            let source = "// Saves it: now. Locks it. Logs it.\npub fn save_answer() {}\n";

            assert_eq!(find_in_new_file("lib.rs", source), []);
        }

        /// `find_file_style_issues` must not ask for a verb at the start of a test, getter, constructor or framework method name.
        /// Otherwise an agent would be told to put a verb on a name that must not have one.
        #[test]
        fn skips_verb_rule_on_conventional_names() {
            let go = |code: &str| format!("package review\n\n{code}");
            let python = r#"import pytest
from typing import override


class Store:
    """Holds answers."""

    def __init__(self):
        """Starts empty."""

    def setUp(self):
        """Prepares it."""

    def tearDown(self):
        """Cleans up."""

    @property
    def total(self):
        """Counts answers."""

    @override
    def ledger(self):
        """Lists answers."""


@pytest.fixture
def database():
    """Opens an empty one."""
"#;
            let not_a_verb = |word: &str| {
                let detail = format!("\"{word}\" is not in the checker's verbs");
                vec![(
                    Rule::VerbFirst,
                    Outcome::Reported(ReportNote::ReportOnly),
                    detail,
                )]
            };
            let cases = [
                (
                    "go-constructor",
                    "answer.go",
                    go("func NewStore() {}\n"),
                    vec![],
                ),
                (
                    "go-getter",
                    "answer.go",
                    go("type Store struct{}\n\nfunc (s Store) Total() int { return 0 }\n"),
                    vec![],
                ),
                (
                    "go-test-and-benchmark",
                    "answer_test.go",
                    go("func TestSave(t *testing.T) {}\n\nfunc BenchmarkSave(b *testing.B) {}\n"),
                    vec![],
                ),
                (
                    "go-testify-needs-verb",
                    "answer_test.go",
                    go("func Testify() {}\n"),
                    not_a_verb("Testify"),
                ),
                (
                    "python-framework-names",
                    "answer.py",
                    python.to_owned(),
                    vec![],
                ),
            ];

            for (case, file, source, expected) in cases {
                assert_eq!(find_in_new_file(file, &source), expected, "{case}");
            }
        }
    }

    /// `Rule::sentence` must quote the `code-documentation` skill word for word.
    /// Otherwise an agent could be told to follow a sentence the skill no longer says.
    #[test]
    fn rule_sentences_quote_the_skill() {
        let skill_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ai/skills/code-documentation/SKILL.md"
        );
        let skill = std::fs::read_to_string(skill_path).unwrap();
        let flat_skill = skill.split_whitespace().collect::<Vec<_>>().join(" ");
        let rules = [
            Rule::Approved,
            Rule::ApprovedRemoved,
            Rule::Filler,
            Rule::AvoidedWord,
            Rule::RestatedName,
            Rule::StartsWithName,
            Rule::CommentLength,
            Rule::CommentColon,
            Rule::SevenWords,
            Rule::VerbFirst,
            Rule::Docstring,
            Rule::SummaryLine,
            Rule::TestWhy,
            Rule::SectionHeading,
        ];

        let unquoted: Vec<Rule> = rules
            .into_iter()
            .filter(|rule| !flat_skill.contains(rule.sentence()))
            .collect();

        assert_eq!(unquoted, []);
    }
}
