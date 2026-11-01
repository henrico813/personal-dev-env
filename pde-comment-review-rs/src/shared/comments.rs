//! This module finds each comment in a file's syntax tree, names the code it
//! describes, and rewrites comments in memory until a later step writes them.
//! It is the only file that calls `language_rules/`. Read the
//! `find_comments(file, source)`, `rewrite(dir, edits)`,
//! `to_plain_words(file, text)`, and `read_version_comments(file, version)`
//! functions to understand the module.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser, Tree};

use super::git::{self, CommitId, FileVersion};
use super::review_session::FeedbackRequest;
use super::{Error, ErrorCode};
use crate::language_rules::{LanguageRules, find_rules_for};

/// Finds and names every comment in a file's source, in file order,
/// leaving out tool lines.
///
/// # Errors
///
/// Fails when no language rules read the file or the source does not
/// parse.
pub fn find_comments(file: &Path, source: &str) -> Result<Vec<Comment>, FindCommentsError> {
    find_comments_and_tree(file, source).map(|(comments, _)| comments)
}

/// Returns each edited file's new contents, writing no file. A file whose
/// edits change nothing is left out.
///
/// # Errors
///
/// Refuses with `TextChanged` when a comment is no longer in its file as it
/// was read. Refuses with `WouldNotWrite` when an edit would change more than
/// its comment, changes a comment the user can only keep, rewords a comment
/// the user can only keep or cut, or fails the language's checks. Fails with
/// `FileUnreadable` when a file cannot be read or does not parse.
pub fn rewrite(dir: &Path, edits: &[CommentEdit]) -> Result<Vec<RewrittenFile>, Error> {
    let mut edits_by_file: BTreeMap<&Path, Vec<&CommentEdit>> = BTreeMap::new();
    for edit in edits {
        edits_by_file
            .entry(&edit.comment.file)
            .or_default()
            .push(edit);
    }
    let mut rewritten = Vec::new();
    for (file, file_edits) in edits_by_file {
        if let Some(new_contents) = rewrite_file(dir, file, &file_edits)? {
            rewritten.push(RewrittenFile {
                path: file.to_path_buf(),
                new_contents,
            });
        }
    }
    Ok(rewritten)
}

/// Returns the comment's words without `//`, `#` or quotes, for the user to
/// read and edit. Text that is not all `//` or `#` lines, or one whole
/// docstring, comes back as it is.
pub fn to_plain_words(file: &Path, text: &str) -> String {
    match find_rules_for(file) {
        Some(rules) => rules.to_plain_words(text),
        None => text.to_owned(),
    }
}

/// Returns `words` as comment lines laid out like `old_text`, with its
/// markers and indentation, or an empty text when `words` has none.
pub(crate) fn lay_out_plain_words(file: &Path, words: &str, old_text: &str) -> String {
    match find_rules_for(file) {
        Some(rules) => rules.lay_out_plain_words(words, old_text),
        None => words.to_owned(),
    }
}

/// Finds the comments in one version of a root-relative file, or none where
/// the file is absent. Refuses with `FileUnreadable`, naming the file and the
/// version, when that version does not parse.
pub(crate) fn read_version_comments(
    file: &Path,
    version: FileVersion,
) -> Result<FileComments, Error> {
    let file_label = format!("{} {}", file.display(), version.place);
    find_file_comments(file, version.text, &file_label)
}

/// Finds the comments in a root-relative file as it is on disk, or none where
/// it is absent. `root` is the worktree's top folder.
pub(crate) fn read_comments_on_disk(root: &Path, file: &Path) -> Result<FileComments, Error> {
    read_version_comments(file, git::read_file_on_disk(root, file)?)
}

/// Finds the comments in a root-relative file as it was at `commit`, or none
/// for a file added since. Refuses with `FileUnreadable`, naming the file, when
/// that version does not parse.
pub(crate) fn read_comments_at_commit(
    root: &Path,
    commit: &CommitId,
    file: &Path,
) -> Result<FileComments, Error> {
    let source = git::read_file_at_commit_from_root(root, commit, file)?;
    let file_label = format!("{} at {}", file.display(), commit.as_str());
    find_file_comments(file, source, &file_label)
}

/// Returns true when a language rule reads the file.
pub(crate) fn is_supported(file: &Path) -> bool {
    find_rules_for(file).is_some()
}

/// Returns every line of a comment joined into one line, examples included,
/// in a language whose length and colon rules read them all.
pub(crate) fn join_unfiltered_lines(comment: &Comment) -> Option<String> {
    find_rules_for(&comment.file)?.join_unfiltered_lines(&comment.text)
}

/// Returns a comment's paragraphs of prose, each on one line, leaving out
/// examples, code and the comment's markers.
pub(crate) fn list_paragraphs(comment: &Comment) -> Vec<String> {
    match find_rules_for(&comment.file) {
        Some(rules) => rules.to_paragraphs(&comment.text),
        None => Vec::new(),
    }
}

/// One comment, where it is in its file, and the code it describes.
/// Lines count from 1.
#[derive(Debug)]
pub struct Comment {
    pub file: PathBuf,
    /// The key approvals use for this comment. It is `None` when no
    /// approval can be saved for the comment.
    pub name: Option<String>,
    pub line: usize,
    pub end_line: usize,
    pub style: CommentStyle,
    /// The comment's lines as written, leaving out any code that shares
    /// them. It ends in a newline unless code follows it.
    pub text: String,
    /// Where `text` sits in the file. The text can end in a newline the file
    /// lacks.
    pub byte_range: Range<usize>,
    /// False for a comment the user can only keep, because a tool reads it or
    /// a cut could change the code around it.
    pub can_cut: bool,
}

/// How a comment is marked. Lines join into one comment only when
/// their style matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStyle {
    /// `//` in Go and Rust.
    Line,
    /// `///` before a Rust function or type.
    ItemDoc,
    /// `//!` at the top of a Rust module.
    ModuleDoc,
    /// `#` in Python.
    Hash,
    /// A Python `"""` docstring.
    Docstring,
    /// A comment after code on its line, holding none of the code.
    AfterCode,
    /// `/* */`, `/** */` or `/*! */`, kept as whole lines unless code
    /// follows it.
    Block,
    /// A Rust `#[doc]` attribute.
    DocAttribute,
}

impl CommentStyle {
    /// Returns false for the styles the user can only keep or cut.
    pub fn can_reword(self) -> bool {
        match self {
            Self::Line | Self::ItemDoc | Self::ModuleDoc | Self::Hash | Self::Docstring => true,
            Self::AfterCode | Self::Block | Self::DocAttribute => false,
        }
    }

    /// Returns true for the line styles whose neighboring lines join
    /// into one comment.
    fn joins_neighboring_lines(self) -> bool {
        matches!(
            self,
            Self::Line | Self::ItemDoc | Self::ModuleDoc | Self::Hash
        )
    }
}

/// Why a file's comments could not be read.
#[derive(Debug, PartialEq, Eq)]
pub enum FindCommentsError {
    /// No language rules match the file's extension.
    UnsupportedLanguage,
    /// The file does not parse, so its comments could be misread.
    SyntaxError,
}

impl fmt::Display for FindCommentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLanguage => write!(f, "no language rules read this file"),
            Self::SyntaxError => write!(f, "the file does not parse"),
        }
    }
}

impl std::error::Error for FindCommentsError {}

/// Holds a comment and the text that replaces it, or `None` to cut it.
pub struct CommentEdit {
    pub comment: Comment,
    pub new_text: Option<String>,
}

/// Holds a file's new contents, checked and formatted, not yet written.
#[derive(Debug)]
pub struct RewrittenFile {
    pub path: PathBuf,
    pub new_contents: String,
}

/// Holds every comment in one file, in file order, with the file's source and
/// syntax tree.
pub(crate) struct FileComments {
    pub(crate) comments: Vec<Comment>,
    pub(crate) file: PathBuf,
    source: Option<String>,
    /// Holds the syntax tree parsed with the comments, so the style rules read
    /// definitions without parsing the file again.
    tree: Option<Tree>,
}

impl FileComments {
    /// Returns true when the file holds a comment with the same name and text.
    pub(crate) fn has_same(&self, comment: &Comment) -> bool {
        self.comments
            .iter()
            .any(|old| old.name == comment.name && old.text == comment.text)
    }

    /// Returns the comment each request matches, never reusing a comment for two
    /// requests. A named request matches by name; otherwise it takes the nearest
    /// comment with the same text, or the unnamed comment at its line.
    pub(crate) fn match_requests(&self, requests: &[&FeedbackRequest]) -> Vec<Option<&Comment>> {
        let mut matched = vec![None; requests.len()];
        let mut taken = vec![false; self.comments.len()];
        for (slot, request) in matched.iter_mut().zip(requests) {
            let nearest = self
                .comments
                .iter()
                .enumerate()
                .filter(|(index, comment)| {
                    !taken[*index]
                        && comment.name == request.name
                        && (request.name.is_some() || comment.text == request.text)
                })
                .min_by_key(|(_, comment)| comment.line.abs_diff(request.line));
            if let Some((index, _)) = nearest {
                taken[index] = true;
                *slot = Some(index);
            }
        }
        // A comment reworded in place keeps its request, unless another request took it.
        for (slot, request) in matched.iter_mut().zip(requests) {
            if slot.is_some() || request.name.is_some() {
                continue;
            }
            let at_line = self
                .comments
                .iter()
                .position(|comment| comment.name.is_none() && comment.line == request.line);
            if let Some(index) = at_line.filter(|index| !taken[*index]) {
                taken[index] = true;
                *slot = Some(index);
            }
        }
        matched
            .into_iter()
            .map(|slot| slot.map(|index| &self.comments[index]))
            .collect()
    }

    /// Returns the line of the code a comment named `name` would describe, or
    /// `None` when no code in the file would give a comment that name.
    pub(crate) fn find_code_line(&self, name: &str) -> Option<usize> {
        let source = self.source.as_deref()?;
        // A module comment describes the whole file, which is still here.
        if name == "<module>" {
            return Some(1);
        }
        let rules = find_rules_for(&self.file)?;
        let tree = parse_source(rules, source);
        let root = tree.root_node();
        // Each line's first node is the code a comment just above it would describe.
        let mut line_start = 0;
        for (index, line) in source.split_inclusive('\n').enumerate() {
            let code_start = line_start + line.len() - line.trim_start().len();
            line_start += line.len();
            if line.trim().is_empty() {
                continue;
            }
            let code = root.named_descendant_for_byte_range(code_start, code_start + 1);
            if code
                .and_then(|code| rules.name_code_below(code, source))
                .as_deref()
                == Some(name)
            {
                return Some(index + 1);
            }
        }
        // A docstring names the definition it sits in, not the code below it.
        let mut nodes = vec![root];
        while let Some(node) = nodes.pop() {
            if rules.name_docstring_owner(node, source).as_deref() == Some(name) {
                return Some(node.start_position().row + 1);
            }
            let mut cursor = node.walk();
            nodes.extend(node.named_children(&mut cursor));
        }
        None
    }

    /// Returns true for a comment with no name above the file's first code,
    /// where a license sits.
    pub(crate) fn is_header(&self, comment: &Comment) -> bool {
        if comment.name.is_some() {
            return false;
        }
        let Some(tree) = &self.tree else {
            return true;
        };
        let root = tree.root_node();
        let mut cursor = root.walk();
        let first_code_line = root
            .named_children(&mut cursor)
            .find(|node| !node.is_extra())
            .map(|node| node.start_position().row + 1);
        first_code_line.is_none_or(|line| comment.end_line < line)
    }

    /// Returns the file's functions, methods and classes, read from its tree
    /// only when a style rule asks.
    pub(crate) fn list_definitions(&self) -> Vec<Definition> {
        match (&self.source, &self.tree, find_rules_for(&self.file)) {
            (Some(source), Some(tree), Some(rules)) => {
                rules.list_definitions(&self.file, tree.root_node(), source)
            }
            _ => Vec::new(),
        }
    }

    /// Returns true for a file a generator wrote, whose style is not checked.
    pub(crate) fn is_generated(&self) -> bool {
        match (&self.source, find_rules_for(&self.file)) {
            (Some(source), Some(rules)) => rules.is_generated(source),
            _ => false,
        }
    }
}

/// Holds one function, method or class and what its rules need to know.
pub(crate) struct Definition {
    /// Holds the name with its type or class, as the comment above it is named.
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) name_check: NameCheck,
    /// Holds what the docstring rules need, or `None` in a language without
    /// docstrings.
    pub(crate) docstring: Option<DocstringFacts>,
}

/// Says which rules a definition's name must meet.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum NameCheck {
    /// Skips a name the language or a framework chose, and a class name.
    Skip,
    /// Checks only the word count, for a name that needs no verb.
    WordCount,
    WordCountAndVerb,
}

/// Holds what the docstring rules need to know about one definition.
pub(crate) struct DocstringFacts {
    /// Holds the text between the quotes, or `None` when there is no docstring.
    pub(crate) text: Option<String>,
    /// Lists the definition's lines and its decorators, leaving out the
    /// definitions inside it.
    pub(crate) own_lines: BTreeSet<usize>,
    pub(crate) is_required: bool,
    pub(crate) is_test: bool,
}

/// Returns what `find_comments` returns and the tree it parsed.
fn find_comments_and_tree(
    file: &Path,
    source: &str,
) -> Result<(Vec<Comment>, Tree), FindCommentsError> {
    let rules = find_rules_for(file).ok_or(FindCommentsError::UnsupportedLanguage)?;
    let tree = parse_source(rules, source);
    if tree.root_node().has_error() {
        return Err(FindCommentsError::SyntaxError);
    }
    let mut comments = Vec::new();
    collect_comments(rules, file, tree.root_node(), source, &mut comments);
    unname_shared_names(&mut comments);
    for comment in &mut comments {
        if rules.is_read_by_tools(file, &comment.text) {
            comment.can_cut = false;
        }
    }
    Ok((comments, tree))
}

/// Parses `source` with the language's grammar.
fn parse_source(rules: &dyn LanguageRules, source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&rules.grammar())
        .expect("each grammar crate supports this tree-sitter version");
    parser
        .parse(source, None)
        .expect("parsing fails only after a timeout or cancel, and neither is set")
}

/// Adds the comments in `node` and its children to `comments`.
fn collect_comments(
    rules: &dyn LanguageRules,
    file: &Path,
    node: Node,
    source: &str,
    comments: &mut Vec<Comment>,
) {
    match rules.classify_node(node, source) {
        Some(style) => add_comment(rules, file, node, style, source, comments),
        None => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_comments(rules, file, child, source, comments);
            }
        }
    }
}

/// Adds a comment, joining it to the comment above when both are
/// neighboring lines of one style.
fn add_comment(
    rules: &dyn LanguageRules,
    file: &Path,
    node: Node,
    style: CommentStyle,
    source: &str,
    comments: &mut Vec<Comment>,
) {
    let can_cut = !rules.is_keep_only(node, source);
    let line_start = node.start_byte() - node.start_position().column;
    let after_code = !source[line_start..node.start_byte()].trim().is_empty();
    // A comment after code starts at its marker, so the code stays out.
    let text_start = if after_code {
        node.start_byte()
    } else {
        line_start
    };
    let style = if after_code {
        CommentStyle::AfterCode
    } else {
        style
    };

    let line_end = find_line_end(source, node.end_byte());
    // Code after a block comment on its line stays out of its text.
    let code_after = !source[node.end_byte()..line_end].trim().is_empty();
    let text_end = if code_after {
        node.end_byte()
    } else {
        line_end
    };

    let mut text = source[text_start..text_end].to_string();
    if !code_after && !text.ends_with('\n') {
        text.push('\n');
    }
    let line = node.start_position().row + 1;
    let end_line = line + text.lines().count() - 1;
    // Today's comment checker names no comment that shares a line with code.
    let name = if after_code || code_after {
        None
    } else {
        let code_below = find_code_below(node, line_end, source);
        rules.name_comment(node, code_below, source)
    };

    if let Some(above) = comments.last_mut()
        && style.joins_neighboring_lines()
        && above.style == style
        && above.end_line + 1 == line
    {
        // The code below the last line names the whole comment.
        above.name = name;
        above.end_line = end_line;
        above.text.push_str(&text);
        above.byte_range.end = text_end;
        above.can_cut &= can_cut;
        return;
    }
    comments.push(Comment {
        file: file.to_path_buf(),
        name,
        line,
        end_line,
        style,
        text,
        byte_range: text_start..text_end,
        can_cut,
    });
}

/// Returns the end of the line holding `node_end`, just past its
/// newline.
fn find_line_end(source: &str, node_end: usize) -> usize {
    // Rust's `///` and `//!` nodes already end with their newline.
    if source[..node_end].ends_with('\n') {
        return node_end;
    }
    match source[node_end..].find('\n') {
        Some(offset) => node_end + offset + 1,
        None => source.len(),
    }
}

/// Returns the first node on the line that starts at `line_start`, or
/// `None` when that line is blank or missing.
fn find_code_below<'tree>(
    comment: Node<'tree>,
    line_start: usize,
    source: &str,
) -> Option<Node<'tree>> {
    let line = source[line_start..].lines().next()?;
    if line.trim().is_empty() {
        return None;
    }
    let code_start = line_start + line.len() - line.trim_start().len();
    let mut root = comment;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    root.named_descendant_for_byte_range(code_start, code_start + 1)
}

/// Takes the name off comments that share it, since an approval could
/// not tell them apart.
fn unname_shared_names(comments: &mut [Comment]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in comments.iter().filter_map(|comment| comment.name.clone()) {
        *counts.entry(name).or_default() += 1;
    }
    for comment in comments {
        if comment.name.as_ref().is_some_and(|name| counts[name] > 1) {
            comment.name = None;
        }
    }
}

/// Returns one file's new contents, or `None` when its edits change nothing.
fn rewrite_file(dir: &Path, file: &Path, edits: &[&CommentEdit]) -> Result<Option<String>, Error> {
    let path = dir.join(file);
    let source = fs::read_to_string(&path).map_err(|error| refuse_unreadable(file, error))?;
    let comments = find_comments(file, &source).map_err(|error| refuse_unreadable(file, error))?;
    let rules = find_rules_for(file).expect("find_comments found rules for the file");
    let mut new_texts: Vec<Option<&str>> = comments
        .iter()
        .map(|comment| Some(comment.text.as_str()))
        .collect();
    for edit in edits {
        let Some(index) = comments.iter().position(|comment| {
            comment.byte_range == edit.comment.byte_range && comment.text == edit.comment.text
        }) else {
            return Err(refuse(
                ErrorCode::TextChanged,
                file,
                "A comment changed after it was read.",
            ));
        };
        let new_text = edit.new_text.as_deref();
        if new_text != Some(comments[index].text.as_str()) && !comments[index].can_cut {
            return Err(refuse(
                ErrorCode::WouldNotWrite,
                file,
                "This comment can only be kept, since a tool reads it or it sits inside code.",
            ));
        }
        if new_text.is_some_and(|text| text != comments[index].text) {
            check_reword(rules, file, &comments[index], new_text.unwrap_or_default())?;
        }
        new_texts[index] = new_text;
    }

    let cuts: Vec<&Comment> = comments
        .iter()
        .zip(&new_texts)
        .filter(|(_, new_text)| new_text.is_none())
        .map(|(comment, _)| comment)
        .collect();
    let mut splices: Vec<(Range<usize>, &str)> = find_cut_ranges(&source, &cuts)
        .into_iter()
        .map(|range| (range, ""))
        .collect();
    let mut expected = Vec::new();
    for (comment, new_text) in comments.iter().zip(&new_texts) {
        if let Some(text) = new_text {
            if *text != comment.text {
                splices.push((comment.byte_range.clone(), text));
            }
            expected.push((comment.style, *text));
        }
    }
    splices.sort_by_key(|(range, _)| range.start);
    let mut new_source = source.clone();
    // The last splice goes first, so earlier byte ranges still hold their text.
    for (range, text) in splices.into_iter().rev() {
        new_source.replace_range(range, text);
    }
    if new_source == source {
        return Ok(None);
    }

    let folder = path.parent().unwrap_or(dir);
    let finished = rules
        .finish_rewrite(folder, &source, &new_source)
        .map_err(|error| refuse(error.code, file, &error.message))?;
    let would_change_code = || {
        refuse(
            ErrorCode::WouldNotWrite,
            file,
            "The new text would change more than the edited comments.",
        )
    };
    // Reading the formatted comments back shows each holds the text the user approves.
    let found = find_comments(file, &finished).map_err(|_| would_change_code())?;
    let found: Vec<_> = found
        .iter()
        .map(|comment| (comment.style, comment.text.as_str()))
        .collect();
    if found != expected {
        return Err(would_change_code());
    }
    // Checking the text that would be written catches a change by the splice or the formatter.
    let old_code =
        read_code_shape(file, &source).map_err(|error| refuse_unreadable(file, error))?;
    let new_code = read_code_shape(file, &finished).map_err(|_| would_change_code())?;
    if new_code != old_code {
        return Err(would_change_code());
    }
    Ok(Some(finished))
}

fn refuse_unreadable(file: &Path, error: impl fmt::Display) -> Error {
    refuse(ErrorCode::FileUnreadable, file, &format!("{error}."))
}

fn refuse(code: ErrorCode, file: &Path, reason: &str) -> Error {
    Error {
        code,
        message: format!("Cannot rewrite {}. {reason}", file.display()),
    }
}

/// Refuses a reword the user cannot make, or text a tool or the language
/// would read differently.
fn check_reword(
    rules: &dyn LanguageRules,
    file: &Path,
    comment: &Comment,
    new_text: &str,
) -> Result<(), Error> {
    if !comment.style.can_reword() {
        return Err(refuse(
            ErrorCode::WouldNotWrite,
            file,
            "A comment that can only be kept or cut was reworded.",
        ));
    }
    if rules.is_read_by_tools(file, new_text) {
        return Err(refuse(
            ErrorCode::WouldNotWrite,
            file,
            "A tool would read the new text.",
        ));
    }
    // Python reads a carriage return as a line end, and Rust rejects one in a doc comment.
    if new_text.contains(['\r', '\0']) {
        return Err(refuse(
            ErrorCode::WouldNotWrite,
            file,
            "The new text holds a carriage return or a NUL.",
        ));
    }
    // rustc rejects these in a comment, since they make code read unlike how it runs.
    let changes_direction = |letter: char| {
        ('\u{202A}'..='\u{202E}').contains(&letter) || ('\u{2066}'..='\u{2069}').contains(&letter)
    };
    if new_text.contains(changes_direction) {
        return Err(refuse(
            ErrorCode::WouldNotWrite,
            file,
            "The new text holds a character that changes the direction of text.",
        ));
    }
    Ok(())
}

/// Returns the bytes the cuts remove, in file order. Cuts that overlap or
/// touch merge first, so no cut takes a line end another cut left, and a cut
/// that leaves its lines blank takes them whole.
fn find_cut_ranges(source: &str, cuts: &[&Comment]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for comment in cuts {
        let range = find_cut_range(source, comment);
        match ranges.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => ranges.push(range),
        }
    }
    for range in &mut ranges {
        let line_start = source[..range.start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = find_line_end(source, range.end);
        if source[line_start..range.start].trim().is_empty()
            && source[range.end..line_end].trim().is_empty()
        {
            *range = line_start..line_end;
        }
    }
    ranges
}

/// Returns the bytes one cut removes, the spaces before it included and its
/// line end left out.
fn find_cut_range(source: &str, comment: &Comment) -> Range<usize> {
    let Range { start, end } = comment.byte_range.clone();
    let text = &source[start..end];
    let before = &source[..start];
    let spaces_before = before.len() - before.trim_end_matches([' ', '\t']).len();
    let line_end = text.len() - text.trim_end_matches(['\r', '\n']).len();
    start - spaces_before..end - line_end
}

/// Returns each node's depth and kind and each leaf's text, in tree order,
/// leaving out nodes inside a comment. Equal results mean equal code, Python's
/// blocks too.
fn read_code_shape<'source>(
    file: &Path,
    source: &'source str,
) -> Result<Vec<(usize, u16, &'source str)>, FindCommentsError> {
    let comments = find_comments(file, source)?;
    let rules = find_rules_for(file).ok_or(FindCommentsError::UnsupportedLanguage)?;
    let tree = parse_source(rules, source);
    let mut shape = Vec::new();
    let mut nodes = vec![(tree.root_node(), 0)];
    while let Some((node, depth)) = nodes.pop() {
        let range = node.byte_range();
        // A docstring's string node sits inside its comment's text, so it is left out too.
        // The root stays, so a file that holds only a comment can still be cut.
        let in_comment = depth > 0
            && comments.iter().any(|comment| {
                comment.byte_range.start <= range.start && range.end <= comment.byte_range.end
            });
        if in_comment {
            continue;
        }
        let text = if node.child_count() == 0 {
            &source[range]
        } else {
            ""
        };
        shape.push((depth, node.kind_id(), text));
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        nodes.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }
    Ok(shape)
}

fn find_file_comments(
    file: &Path,
    source: Option<String>,
    file_label: &str,
) -> Result<FileComments, Error> {
    // Approvals hold `\n` line ends, so a checkout with `\r\n` ones must still match them.
    let source = source.map(|source| source.replace("\r\n", "\n"));
    let (comments, tree) = match &source {
        Some(source) => find_comments_and_tree(file, source)
            .map_err(|error| Error {
                code: ErrorCode::FileUnreadable,
                message: format!("Cannot read the comments in {file_label}, since {error}."),
            })
            .map(|(comments, tree)| (comments, Some(tree)))?,
        None => (Vec::new(), None),
    };
    Ok(FileComments {
        comments,
        file: file.to_path_buf(),
        source,
        tree,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use CommentStyle::{AfterCode, Block, DocAttribute, Docstring, Hash, ItemDoc, Line, ModuleDoc};

    /// One comment a test expects `find_comments` to return.
    struct Expected {
        line: usize,
        end_line: usize,
        style: CommentStyle,
        text: &'static str,
    }

    /// Checks each comment's lines, style and exact text.
    fn assert_finds(file: &str, source: &str, expected: &[Expected]) {
        let comments = find_comments(Path::new(file), source).expect("the sample parses");
        let found: Vec<_> = comments
            .iter()
            .map(|c| (c.line, c.end_line, c.style, c.text.as_str()))
            .collect();
        let wanted: Vec<_> = expected
            .iter()
            .map(|e| (e.line, e.end_line, e.style, e.text))
            .collect();
        assert_eq!(found, wanted, "{file}");
    }

    /// `find_comments` must return every Go, Python and Rust comment with its whole text.
    /// Otherwise a missed or cut-short comment never reaches the user for review.
    #[test]
    fn find_comments_returns_every_comment_whole() {
        assert_finds(
            "review_store.go",
            include_str!("../../tests/samples/review_store.go"),
            &[
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Line,
                    text: "// Saves the owner's answer under the review's lock.\n",
                },
                Expected {
                    line: 7,
                    end_line: 7,
                    style: Line,
                    text: "\t// The status is saved, never worked out from files.\n",
                },
            ],
        );
        assert_finds(
            "approvals.py",
            include_str!("../../tests/samples/approvals.py"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Hash,
                    text: "# Approvals live in the clone's shared Git folder.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: Docstring,
                    text: "    \"\"\"Return every approval entry, oldest first.\"\"\"\n",
                },
            ],
        );
        assert_finds(
            "review.rs",
            include_str!("../../tests/samples/review.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: ModuleDoc,
                    text: "//! Saves each review in one file, under a lock.\n",
                },
                Expected {
                    line: 3,
                    end_line: 3,
                    style: ItemDoc,
                    text: "/// Saves the owner's answer under the review's lock.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: Line,
                    text: "    // Entries are only added, never changed.\n",
                },
            ],
        );
    }

    /// `find_comments` must join neighboring comment lines into one comment only when their style matches.
    /// Otherwise an approval, which matches a comment's exact text, would no longer match.
    #[test]
    fn find_comments_joins_lines_of_matching_style() {
        assert_finds(
            "review_store.go",
            "package review\n\n// Saves the answer.\n// Waits for the lock.\nfunc SaveAnswer() {}\n",
            &[Expected {
                line: 3,
                end_line: 4,
                style: Line,
                text: "// Saves the answer.\n// Waits for the lock.\n",
            }],
        );
        assert_finds(
            "approvals.py",
            "# Approvals live in Git.\n# Each one has a name.\nAPPROVALS_PATH = \"\"\n",
            &[Expected {
                line: 1,
                end_line: 2,
                style: Hash,
                text: "# Approvals live in Git.\n# Each one has a name.\n",
            }],
        );
        assert_finds(
            "review.rs",
            "//! Saves each review in one file.\n//! Holds one lock per review.\n",
            &[Expected {
                line: 1,
                end_line: 2,
                style: ModuleDoc,
                text: "//! Saves each review in one file.\n//! Holds one lock per review.\n",
            }],
        );
        assert_finds(
            "comment_styles.rs",
            include_str!("../../tests/samples/comment_styles.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 2,
                    style: ItemDoc,
                    text: concat!(
                        "/// Saves one User Feedback under the lock.\n",
                        "/// Waits while another screen holds the lock.\n",
                    ),
                },
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Line,
                    text: "// Only the review screen calls this.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: AfterCode,
                    text: "// The lock waits at most three times.\n",
                },
            ],
        );
    }

    /// `find_comments` must find each comment the user can only keep or cut, holding none of the code beside it.
    /// Otherwise the comment slips past review, or a cut takes the code that shares its line.
    #[test]
    fn find_comments_keeps_code_out_of_comments() {
        assert_finds(
            "keep_or_cut.go",
            include_str!("../../tests/samples/keep_or_cut.go"),
            &[
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Block,
                    text: "/* Saves the owner's answer under the review's lock. */\n",
                },
                Expected {
                    line: 7,
                    end_line: 7,
                    style: AfterCode,
                    text: "// The lock waits at most three times.\n",
                },
                Expected {
                    line: 8,
                    end_line: 8,
                    style: AfterCode,
                    text: "// Each wait is one second.\n",
                },
            ],
        );
        assert_finds(
            "keep_or_cut.rs",
            include_str!("../../tests/samples/keep_or_cut.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Block,
                    text: "/*! Saves each review in one file, under a lock. */\n",
                },
                Expected {
                    line: 2,
                    end_line: 3,
                    style: Block,
                    text: "/* Entries are only added,\n   never changed. */\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: AfterCode,
                    text: "/* The lock waits at most three times. */",
                },
                Expected {
                    line: 8,
                    end_line: 8,
                    style: Block,
                    text: "/** Finishes writing the review. */\n",
                },
                Expected {
                    line: 9,
                    end_line: 9,
                    style: DocAttribute,
                    text: "#[doc = \"Writes the answers into the code.\"]\n",
                },
            ],
        );
    }

    /// `find_comments` must find a docstring below a `#` comment or inside parentheses, and no later string.
    /// Otherwise that docstring never reaches the user for review.
    #[test]
    fn find_comments_finds_commented_or_parenthesized_docstrings() {
        assert_finds(
            "docstrings.py",
            include_str!("../../tests/samples/docstrings.py"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Hash,
                    text: "# Reads the approvals file for the review tool.\n",
                },
                Expected {
                    line: 2,
                    end_line: 2,
                    style: Docstring,
                    text: "\"\"\"Reads the approvals file.\"\"\"\n",
                },
                Expected {
                    line: 6,
                    end_line: 6,
                    style: Hash,
                    text: "    # Approvals are read once per step.\n",
                },
                Expected {
                    line: 7,
                    end_line: 10,
                    style: Docstring,
                    text: concat!(
                        "    \"\"\"Return every approval entry, oldest first.\n",
                        "\n",
                        "    A missing file has no entries.\n",
                        "    \"\"\"\n",
                    ),
                },
                Expected {
                    line: 15,
                    end_line: 15,
                    style: Docstring,
                    text: "    (\"\"\"Write every approval entry, newest last.\"\"\")\n",
                },
            ],
        );
        assert_finds(
            "approvals.py",
            "(\n    \"\"\"Reads the approvals file.\"\"\"\n)\n",
            &[Expected {
                line: 1,
                end_line: 3,
                style: Docstring,
                text: "(\n    \"\"\"Reads the approvals file.\"\"\"\n)\n",
            }],
        );
        assert_finds("approvals.py", "(load_approvals())\n", &[]);
    }

    /// `find_comments` must not take an f-string, bytes or t-string for a docstring, as Python does not.
    /// Otherwise a cut could remove code Python runs, such as the call inside an f-string.
    #[test]
    fn find_comments_skips_strings_that_are_code() {
        let sources = [
            "def shout():\n    f\"{print('Not a docstring, since Python runs this call.')}\"\n    return 1\n",
            "def read_bytes():\n    b\"\"\"Not a docstring, since Python skips bytes.\"\"\"\n",
            "def template():\n    t\"\"\"Not a docstring, since Python skips template strings.\"\"\"\n",
        ];

        for source in sources {
            assert_finds("docstrings.py", source, &[]);
        }
    }

    /// One source and how many comments `find_comments` should find.
    struct ToolCase {
        file: &'static str,
        source: &'static str,
        expected_count: usize,
    }

    /// `find_comments` must leave out tool lines, such as `//go:generate` and `# noqa`, and keep prose that looks like them.
    /// Otherwise a cut tool line changes the build, or a prose line skips review.
    #[test]
    fn find_comments_leaves_out_tool_lines() {
        let cases = [
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n//go:generate stringer -type=Status\n",
                expected_count: 0,
            },
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n//line review_store.go:10\n",
                expected_count: 0,
            },
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n// Saves one answer: the newest.\n",
                expected_count: 1,
            },
            ToolCase {
                file: "approvals.py",
                source: "#!/usr/bin/env python3\n",
                expected_count: 0,
            },
            ToolCase {
                file: "approvals.py",
                source: "# pylint: disable=invalid-name\n",
                expected_count: 0,
            },
            ToolCase {
                file: "approvals.py",
                source: "import json  # noqa: F401\n",
                expected_count: 0,
            },
            ToolCase {
                file: "approvals.py",
                source: "# vim: set fileencoding=utf-8\n",
                expected_count: 0,
            },
        ];

        for case in cases {
            let comments =
                find_comments(Path::new(case.file), case.source).expect("the source parses");
            assert_eq!(comments.len(), case.expected_count, "{:?}", case.source);
        }
    }

    /// `find_comments` must refuse a file that does not parse.
    /// Otherwise it could misread the file and leave comments out of review.
    #[test]
    fn find_comments_refuses_unparsable_files() {
        let source = "package review\n\nfunc SaveAnswer( {}\n";

        let result = find_comments(Path::new("broken.go"), source);

        assert_eq!(result.err(), Some(FindCommentsError::SyntaxError));
    }

    /// One comment's first line and the name a test expects for it.
    struct ExpectedName {
        line: usize,
        name: Option<&'static str>,
    }

    /// Checks each comment's first line and name.
    fn assert_names(file: &str, source: &str, expected: &[ExpectedName]) {
        let comments = find_comments(Path::new(file), source).expect("the sample parses");
        let found: Vec<_> = comments
            .iter()
            .map(|c| (c.line, c.name.as_deref()))
            .collect();
        let wanted: Vec<_> = expected.iter().map(|e| (e.line, e.name)).collect();
        assert_eq!(found, wanted, "{file}");
    }

    /// `find_comments` must name each comment by the code it describes, and name no two comments the same.
    /// Otherwise an approval, which matches by name, is lost or matches the wrong comment.
    #[test]
    fn find_comments_names_comments_by_their_code() {
        assert_names(
            "names.go",
            include_str!("../../tests/samples/names.go"),
            &[
                ExpectedName {
                    line: 1,
                    name: Some("review"),
                },
                ExpectedName {
                    line: 5,
                    name: Some("const Waiting"),
                },
                ExpectedName {
                    line: 7,
                    name: Some("Waiting"),
                },
                ExpectedName {
                    line: 12,
                    name: Some("ReviewStore"),
                },
                ExpectedName {
                    line: 14,
                    name: Some("ReviewStore.Answers"),
                },
                ExpectedName {
                    line: 18,
                    name: Some("ReviewStore.SaveAnswer"),
                },
                ExpectedName {
                    line: 20,
                    name: None,
                },
                ExpectedName {
                    line: 24,
                    name: None,
                },
                ExpectedName {
                    line: 28,
                    name: None,
                },
                ExpectedName {
                    line: 31,
                    name: None,
                },
            ],
        );
        assert_names(
            "names.py",
            include_str!("../../tests/samples/names.py"),
            &[
                ExpectedName {
                    line: 1,
                    name: Some("<module>"),
                },
                ExpectedName {
                    line: 3,
                    name: Some("# APPROVALS_PATH"),
                },
                ExpectedName {
                    line: 8,
                    name: Some("Approvals"),
                },
                ExpectedName {
                    line: 10,
                    name: Some("# Approvals.load"),
                },
                ExpectedName {
                    line: 13,
                    name: Some("Approvals.load"),
                },
                ExpectedName {
                    line: 14,
                    name: None,
                },
                ExpectedName {
                    line: 19,
                    name: None,
                },
                ExpectedName {
                    line: 22,
                    name: None,
                },
            ],
        );
        assert_names(
            "names.rs",
            include_str!("../../tests/samples/names.rs"),
            &[
                ExpectedName {
                    line: 1,
                    name: Some("<module>"),
                },
                ExpectedName {
                    line: 3,
                    name: Some("ReviewStore"),
                },
                ExpectedName {
                    line: 6,
                    name: Some("ReviewStore::answers"),
                },
                ExpectedName {
                    line: 10,
                    name: Some("impl ReviewStore"),
                },
                ExpectedName {
                    line: 12,
                    name: Some("ReviewStore::save_answer"),
                },
                ExpectedName {
                    line: 14,
                    name: None,
                },
                ExpectedName {
                    line: 19,
                    name: Some("LOCK_TRIES"),
                },
                ExpectedName {
                    line: 20,
                    name: None,
                },
                ExpectedName {
                    line: 23,
                    name: None,
                },
                ExpectedName {
                    line: 29,
                    name: Some("tests::saves_each_answer"),
                },
            ],
        );
    }

    /// `lay_out_plain_words` must turn the words `to_plain_words` returns back into the same comment.
    /// Otherwise a reword could drop an example, a section or the comment's markers.
    #[test]
    fn lay_out_plain_words_rebuilds_the_comment() {
        let cases = [
            (
                "save.go",
                "\t// Saves the answer under the lock, so two screens that save at once keep\n\t// both answers.\n\t//\n\t//\tsave(dir, answer)\n",
                "Saves the answer under the lock, so two screens that save at once keep both answers.\n\n\tsave(dir, answer)\n",
            ),
            (
                "save.rs",
                "    /// Returns the answer.\n    ///\n    /// ```\n    /// let answer = save();\n    ///\n    /// ```\n",
                "Returns the answer.\n\n```\nlet answer = save();\n\n```\n",
            ),
            (
                "save.rs",
                "    /// Returns the answer.\n    ///\n    /// ~~~\n    /// assert_eq!(save(), 1);\n    /// ~~~\n",
                "Returns the answer.\n\n~~~\nassert_eq!(save(), 1);\n~~~\n",
            ),
            ("save.rs", "//! Holds answers.\n", "Holds answers.\n"),
            ("save.py", "    # Keeps it.\n", "Keeps it.\n"),
            ("save.py", "    \"\"\"Saves it.\"\"\"\n", "Saves it.\n"),
            (
                "save.py",
                "    \"\"\"Saves the answer.\n\n    Args:\n        answer: The answer the user gave on the Review Screen, kept exactly as\n            they typed it.\n        dir: The worktree.\n\n    Example:\n        >>> save(answer)\n    \"\"\"\n",
                "Saves the answer.\n\nArgs:\nanswer: The answer the user gave on the Review Screen, kept exactly as they typed it.\ndir: The worktree.\n\nExample:\n    >>> save(answer)\n",
            ),
            (
                "save.py",
                "    \"\"\"Greets the user.\n\n    >>> greet()\n    hello\n    world\n    \"\"\"\n",
                "Greets the user.\n\n>>> greet()\nhello\nworld\n",
            ),
        ];

        for (file, text, words) in cases {
            let file = Path::new(file);

            let plain_words = to_plain_words(file, text);
            let laid_out = lay_out_plain_words(file, &plain_words, text);

            assert_eq!(plain_words, words, "{text}");
            assert_eq!(laid_out, text, "{text}");
        }
    }
}
