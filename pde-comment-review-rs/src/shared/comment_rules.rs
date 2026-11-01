//! This module holds the style rules from the `code-documentation` skill,
//! checked on the lines a branch changed. Text the agent may not change is
//! reported, never failed. Read the
//! `check_file(now, old_text, changed, approvals)`,
//! `check_comment(comment, now, old_text, avoided, approvals)`, and
//! `check_definition(definition, now, old_text, changed, approvals)`
//! functions to understand the module.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::approvals::{Approvals, DecisionMatch};
use super::comments::{self, Comment, CommentStyle, Definition, FileComments, NameCheck};

/// Returns the style findings on one file's changed lines, where `changed`
/// lists the lines Git's diff marks in `now`.
pub(crate) fn check_file(
    now: &FileComments,
    old_text: &OldText,
    changed: &BTreeSet<usize>,
    approvals: &Approvals,
) -> Vec<Finding> {
    // A generated file changes only when its generator runs again.
    if now.is_generated() {
        return Vec::new();
    }
    let avoided = build_avoided_word_patterns(approvals.avoided_words());
    let mut findings = Vec::new();
    for comment in &now.comments {
        if (comment.line..=comment.end_line).any(|line| changed.contains(&line)) {
            findings.extend(check_comment(comment, now, old_text, &avoided, approvals));
        }
    }
    for definition in &now.list_definitions() {
        findings.extend(check_definition(
            definition, now, old_text, changed, approvals,
        ));
    }
    findings.sort_by_key(|finding| finding.line);
    findings
}

/// Returns the findings in one changed comment.
fn check_comment(
    comment: &Comment,
    now: &FileComments,
    old_text: &OldText,
    avoided: &[(Regex, String)],
    approvals: &Approvals,
) -> Vec<Finding> {
    let decision = approvals.compare(comment);
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
    broken.extend(find_avoided_words(&sentences, avoided));
    broken.extend(find_filler(&sentences, name));
    broken.extend(find_name_at_start(&sentences, name));
    if is_go_or_python(&comment.file)
        && matches!(
            comment.style,
            CommentStyle::Line | CommentStyle::Hash | CommentStyle::AfterCode
        )
    {
        let unmarked = list_unmarked_sentences(comment, &paragraphs);
        broken.extend(find_long_comment(&unmarked));
        broken.extend(find_colons(&unmarked));
    }
    let mut findings = Vec::new();
    for RuleMatch {
        rule,
        sentences: found,
        detail,
    } in broken
    {
        let existing = old_text.has_sentences(&comment.file, &found);
        // An old sentence may be a person's words, so it is never called a restatement.
        if existing && rule == Rule::RestatedName {
            continue;
        }
        let note = if is_approved {
            Some(ReportNote::Approved)
        } else if now.is_header(comment) {
            Some(ReportNote::Header)
        } else if existing {
            Some(ReportNote::Existing)
        } else if rule == Rule::RestatedName {
            Some(ReportNote::ReportOnly)
        } else {
            None
        };
        findings.push(Finding {
            file: comment.file.clone(),
            name: comment.name.clone(),
            line: comment.line,
            rule,
            note,
            detail,
        });
    }
    findings
}

/// Returns the findings in a new name, and in the docstring of a definition
/// whose own lines changed.
fn check_definition(
    definition: &Definition,
    now: &FileComments,
    old_text: &OldText,
    changed: &BTreeSet<usize>,
    approvals: &Approvals,
) -> Vec<Finding> {
    let mut broken: Vec<(Rule, Option<ReportNote>, Option<String>)> = Vec::new();
    if definition.name_check != NameCheck::Skip && !old_text.has_name(&now.file, &definition.name) {
        let words = split_name_words(strip_qualifier(&definition.name));
        if words.len() > 7 {
            broken.push((
                Rule::SevenWords,
                None,
                Some(format!("{} words", words.len())),
            ));
        }
        if let Some(first_word) = words.first()
            && definition.name_check == NameCheck::WordCountAndVerb
            && !is_verb(first_word)
        {
            let detail = format!("\"{first_word}\" is not in the checker's verbs");
            broken.push((Rule::VerbFirst, Some(ReportNote::ReportOnly), Some(detail)));
        }
    }
    if let Some(docstring) = &definition.docstring
        && !docstring.own_lines.is_disjoint(changed)
    {
        let comment = now.comments.iter().find(|comment| {
            comment.style == CommentStyle::Docstring
                && comment.name.as_deref() == Some(definition.name.as_str())
        });
        let decision = comment.map(|comment| approvals.compare(comment));
        let note = match (comment, &decision) {
            (_, Some(DecisionMatch::Approved)) => Some(ReportNote::Approved),
            (Some(comment), _)
                if old_text.has_sentences(
                    &now.file,
                    &split_sentences(&comments::list_paragraphs(comment).join(" ")),
                ) =>
            {
                Some(ReportNote::Existing)
            }
            _ => None,
        };
        match &docstring.text {
            None if docstring.is_required => {
                // Every line of a moved file counts as changed, so a function a deleted
                // changed file held at `base` is old.
                let is_moved = !old_text.has_file(&now.file)
                    && old_text.has_gone_name(&now.file, &definition.name);
                let note = is_moved.then_some(ReportNote::Existing);
                broken.push((Rule::Docstring, note, None));
            }
            None => {}
            // Changed approved text already fails, and the agent must put it back as it was.
            Some(_) if decision == Some(DecisionMatch::DiffersFromApproved) => {}
            Some(text) => {
                for (rule, detail) in find_docstring_rules(text, docstring.is_test) {
                    broken.push((rule, note, detail));
                }
            }
        }
    }
    broken
        .into_iter()
        .map(|(rule, note, detail)| Finding {
            file: now.file.clone(),
            name: Some(definition.name.clone()),
            line: definition.line,
            rule,
            note,
            detail,
        })
        .collect()
}

/// Holds one comment or name that breaks a rule.
#[derive(Debug, PartialEq)]
pub struct Finding {
    pub file: PathBuf,
    pub name: Option<String>,
    pub line: usize,
    pub rule: Rule,
    /// Says why the finding is only reported, or `None` when it fails `check_comments`.
    pub note: Option<ReportNote>,
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
/// Matches a Go paragraph that Go's tools read, whose colon and length stay.
static GO_DOC_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:Deprecated|BUG\([^)]*\)):").expect("the pattern is valid"));

/// Holds the comment sentences and definition names the changed files held at
/// `base`, so code moved from one file to another still counts as old.
#[derive(Default)]
pub(crate) struct OldText {
    /// Holds each sentence with its file's extension, since a rule's reach
    /// depends on the language.
    sentences: BTreeSet<(String, String)>,
    names: BTreeSet<(String, String)>,
    /// Lists the changed files that were there at `base`.
    files: BTreeSet<PathBuf>,
    /// Holds the definition names of the changed files the branch deleted.
    gone_names: BTreeSet<(String, String)>,
}

impl OldText {
    /// Adds one changed file's comments and definitions as it was at `base`,
    /// where `is_gone` says the branch deleted the file.
    pub(crate) fn add(&mut self, before: &FileComments, is_gone: bool) {
        let language = read_extension(&before.file);
        if before.exists() {
            self.files.insert(before.file.clone());
        }
        for comment in &before.comments {
            for sentence in split_sentences(&comments::list_paragraphs(comment).join(" ")) {
                self.sentences.insert((language.clone(), sentence));
            }
        }
        for definition in &before.list_definitions() {
            let name = (language.clone(), definition.name.clone());
            if is_gone {
                self.gone_names.insert(name.clone());
            }
            self.names.insert(name);
        }
    }

    fn has_sentences(&self, file: &Path, sentences: &[String]) -> bool {
        let language = read_extension(file);
        sentences.iter().all(|sentence| {
            self.sentences
                .contains(&(language.clone(), sentence.clone()))
        })
    }

    fn has_name(&self, file: &Path, name: &str) -> bool {
        self.names
            .contains(&(read_extension(file), name.to_owned()))
    }

    fn has_gone_name(&self, file: &Path, name: &str) -> bool {
        self.gone_names
            .contains(&(read_extension(file), name.to_owned()))
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
/// line, as today's checker does, and Go leaves out its `Deprecated:` and
/// `BUG():` paragraphs.
fn list_unmarked_sentences(comment: &Comment, paragraphs: &[String]) -> Vec<String> {
    if let Some(prose) = comments::join_unfiltered_lines(comment) {
        return split_sentences(&prose);
    }
    let is_go = comment
        .file
        .extension()
        .is_some_and(|suffix| suffix == "go");
    let kept: Vec<&str> = paragraphs
        .iter()
        .map(String::as_str)
        .filter(|paragraph| !(is_go && GO_DOC_MARKER.is_match(paragraph)))
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

fn read_extension(file: &Path) -> String {
    file.extension().map_or(String::new(), |suffix| {
        suffix.to_string_lossy().into_owned()
    })
}

fn is_go_or_python(file: &Path) -> bool {
    file.extension()
        .is_some_and(|suffix| suffix == "go" || suffix == "py")
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
    use super::*;

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
