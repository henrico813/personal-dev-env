"""Check a comment against the RULES, and find gone approvals."""

from __future__ import annotations

import re
import shlex
from pathlib import Path
from typing import NamedTuple

from .languages import LANGUAGES
from .records import Comment, Finding, Function

GO_AND_PYTHON = frozenset({".go", ".py"})
# Python files get these rules from check_python_rules.py instead.
GO_ONLY = frozenset({".go"})


class Rule(NamedTuple):
    """A rule, the code-documentation sentence it checks, and its reach.

    ``suffixes`` are the files whose comments and names it checks. With
    ``--base``, a ``changed_only`` rule skips function names present at
    that commit and comments whose every sentence is.
    """

    sentence: str
    suffixes: frozenset[str] = GO_AND_PYTHON
    changed_only: bool = False


# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "approved": Rule(
        "On a later pass over unchanged behavior, edit existing prose only "
        "to fix a specific false or unsupported claim, an omission that "
        "makes it misleading for safe use, or an applicable convention "
        "violation; otherwise leave it byte-for-byte unchanged."
    ),
    "restated-name": Rule(
        "Prose derived directly from a name, signature, single statement, "
        "or a test's name, inputs, and assertions is a restatement and "
        "must not be added."
    ),
    "avoided-word": Rule(
        "Explain project-specific terms when a new contributor would not "
        "know them."
    ),
    "filler": Rule("Avoid vague filler such as:"),
    "seven-words": Rule(
        "Every function name must contain at most seven words.",
        GO_ONLY,
        changed_only=True,
    ),
    "verb-first": Rule(
        "Start every function and method name with a verb, except a "
        "test, a fixture or a property.",
        GO_ONLY,
        changed_only=True,
    ),
    "starts-with-name": Rule(
        "Do not start a doc comment with the name it documents",
        GO_ONLY,
        changed_only=True,
    ),
    "comment-length": Rule(
        "Keep each `//` or `#` comment to at most two sentences.",
        GO_ONLY,
        changed_only=True,
    ),
    "comment-colon": Rule(
        "Use no colon in a `//` or `#` comment, except inside code or a URL.",
        GO_ONLY,
        changed_only=True,
    ),
}

# The skill's filler examples and their other word forms. They count only
# when the code is the sentence's subject, since "The caller is
# responsible for closing it" is a fact the skill asks for.
FILLER = (
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
)
# Verbs that start function names. A word with "re" or "un" in front of
# one of these counts too, such as "rebuild".
VERBS = frozenset(
    """
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
    """.split()
)
# Words a name starts with to ask a question or convert, such as
# "is_empty" or "to_json".
QUESTION_OR_CONVERSION_WORDS = frozenset(
    {"as", "can", "from", "has", "into", "should", "to", "was"}
)
# Go function names the language requires, which need no verb.
GO_REQUIRED_NAMES = frozenset({"init", "main"})
CODE_OR_URL = re.compile(r"`[^`]*`|\w+://\S+")
# Abbreviations whose period does not end a sentence.
ABBREVIATIONS = re.compile(r"\b(?:e\.g|i\.e|etc)\.")
# A colon, except one between digits as in a time.
COLON = re.compile(r":(?!\d)")
# A Go paragraph that Go tools read, whose colon must stay.
GO_DOC_MARKER = re.compile(r"(?:Deprecated|BUG\([^)]*\)):")
WORD = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+")


class RuleMatch(NamedTuple):
    """A rule a comment breaks, before the ``--base`` policy applies."""

    rule: str
    message: str
    sentences: list[str]


def split_sentences(prose: str) -> list[str]:
    """Split prose after ``.``, ``!`` or ``?`` followed by a space."""
    return [s for s in re.split(r"(?<=[.!?])\s+", prose) if s]


def split_name_words(name: str) -> list[str]:
    """Split ``parse_HTTPConfig`` into ``["parse", "HTTP", "Config"]``."""
    return [w for part in name.split("_") for w in WORD.findall(part)]


def is_verb(word: str) -> bool:
    """Return whether a name's first word is a known verb or question word."""
    word = word.lower()
    if word in VERBS or word in QUESTION_OR_CONVERSION_WORDS:
        return True
    return word[:2] in {"re", "un"} and word[2:] in VERBS


def has_name_first(text: str, name: str) -> bool:
    """Return whether ``text`` starts with ``name`` or ``name()``."""
    first_word = text.split(maxsplit=1)[:1]
    return bool(first_word) and first_word[0].strip("`'\"(),.:") in {
        name,
        f"{name}()",
    }


def find_rename_candidates(
    name: str,
    text: str,
    file: str,
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
) -> list[Comment]:
    """Find comments that may be a gone approved comment after a rename.

    A comment matches when its first sentence has the same words as the
    approved text's first sentence, except where both words look like
    identifiers (``Total``, ``io.Reader``, ``max_size``, ``V2``) or
    are the two comments' names. A comment with approved text of its
    own is the owner's other comment, not this one, so it never matches.

    Args:
        text: The gone comment's approved text.

    Returns:
        The matching named comments in ``file``, in file order.
    """
    join_prose = LANGUAGES[Path(file).suffix].join_prose
    approved_words = split_first_sentence(join_prose(text))
    candidates = []
    for comment in comments:
        if comment.file != file or not comment.name or comment.name == name:
            continue
        if approved.get((comment.file, comment.name)):
            continue
        words = split_first_sentence(comment.prose)
        bare_names = {re.split(r"[. ]", n)[-1] for n in (name, comment.name)}
        if (
            approved_words
            and len(words) == len(approved_words)
            and all(
                a == b
                or (
                    looks_like_identifier(a, bare_names)
                    and looks_like_identifier(b, bare_names)
                )
                for a, b in zip(approved_words, words)
            )
        ):
            candidates.append(comment)
    return candidates


def split_first_sentence(prose: str) -> list[str]:
    """Return the words of the first sentence, with their punctuation."""
    first = split_sentences(prose)[:1]
    return first[0].split() if first else []


def looks_like_identifier(word: str, names: set[str]) -> bool:
    """Return whether a prose word looks like an identifier.

    Args:
        word: One word of a sentence, with any punctuation around it.
        names: Names that count as identifiers here.

    Returns:
        True for one of ``names``, or a word with a capital letter, a
        digit, ``_`` or an inner ``.``.
    """
    core = word.strip("`'\",;:()[]!?").rstrip(".")
    return core in names or bool(re.search(r"[A-Z]|[0-9_]|\w\.\w", core))


def find_restated_name(sentences: list[str], name: str) -> list[RuleMatch]:
    """Match a first sentence that contains the name of the code below."""
    if (
        name
        and sentences
        and re.search(rf"\b{re.escape(name)}\b", sentences[0])
    ):
        message = f'first sentence repeats the name "{name}"'
        return [RuleMatch("restated-name", message, sentences[:1])]
    return []


def find_filler(sentences: list[str], name: str) -> list[RuleMatch]:
    """Match each ``FILLER`` phrase in sentences whose subject is the code.

    A sentence is about the code when it starts with "This", the code's
    name, or "Package" and the name.
    """
    subject = r"This\b"
    if name:
        subject = rf"(?:This|{re.escape(name)}|Package {re.escape(name)})\b"
    about_code = [s for s in sentences if re.match(subject, s)]
    matches = []
    for phrase in FILLER:
        whole_words = rf"\b{re.escape(phrase)}\b"
        found = [s for s in about_code if re.search(whole_words, s.lower())]
        if found:
            matches.append(RuleMatch("filler", f'"{phrase}"', found))
    return matches


def find_avoided_words(
    sentences: list[str], avoid: dict[str, str]
) -> list[RuleMatch]:
    """Match each avoided word, as a whole word in any sentence."""
    matches = []
    for word, instead in avoid.items():
        whole_word = rf"\b{re.escape(word.lower())}\b"
        found = [s for s in sentences if re.search(whole_word, s.lower())]
        if found:
            matches.append(
                RuleMatch("avoided-word", f'"{word}": {instead}', found)
            )
    return matches


def find_name_at_start(sentences: list[str], name: str) -> list[RuleMatch]:
    """Match a doc comment whose first word is the name it documents."""
    if name and sentences and has_name_first(sentences[0], name):
        message = f'starts with "{name}"'
        return [RuleMatch("starts-with-name", message, sentences[:1])]
    return []


def list_unmarked_sentences(comment: Comment) -> list[str]:
    """Return a comment's sentences outside its Deprecated and BUG paragraphs.

    Lines with no prose, such as blank comment lines, end a paragraph.
    """
    join_prose = LANGUAGES[Path(comment.file).suffix].join_prose
    paragraphs: list[list[str]] = [[]]
    for line in comment.text.splitlines():
        words = join_prose(line)
        if words:
            paragraphs[-1].append(words)
        elif paragraphs[-1]:
            paragraphs.append([])
    kept = [p for p in paragraphs if p and not GO_DOC_MARKER.match(p[0])]
    return split_sentences(" ".join(" ".join(p) for p in kept))


def find_long_comment(sentences: list[str]) -> list[RuleMatch]:
    """Match a comment of more than two sentences, not counting "e.g."."""
    prose = CODE_OR_URL.sub("code", " ".join(sentences))
    count = len(split_sentences(ABBREVIATIONS.sub("abbreviation", prose)))
    if count > 2:
        return [RuleMatch("comment-length", f"{count} sentences", sentences)]
    return []


def find_colons(sentences: list[str]) -> list[RuleMatch]:
    """Match the sentences with a colon outside code, a URL or a time."""
    found = [s for s in sentences if COLON.search(CODE_OR_URL.sub("code", s))]
    if found:
        return [RuleMatch("comment-colon", "has a colon", found)]
    return []


def find_name_violations(
    functions: list[Function], base_names: set[str] | None
) -> list[Finding]:
    """Return findings for names that break the seven-word or verb rule.

    In Go, a test, ``main``, ``init``, a method that takes no arguments,
    as a getter does, and a name that starts with ``New`` need no verb.

    Args:
        base_names: Function names at ``--base``, or ``None`` without it.
    """
    findings = []
    for function in functions:
        bare_name = function.name.split(".")[-1]
        words = split_name_words(bare_name)
        first_word = words[0] if words else ""
        matches = []
        if len(words) > 7:
            matches.append(("seven-words", f"{len(words)} words", ""))
        needs_verb = not (
            function.is_test
            or function.is_getter
            or bare_name in GO_REQUIRED_NAMES
            or first_word.lower() == "new"
        )
        if needs_verb and first_word and not is_verb(first_word):
            message = f'"{first_word}" is not in the checker\'s verbs'
            matches.append(("verb-first", message, "report only"))
        code = Comment(
            file=function.file,
            line=function.line,
            name=function.name,
            text="",
            prose="",
        )
        for rule, message, note in matches:
            reach = RULES[rule]
            if Path(function.file).suffix not in reach.suffixes:
                continue
            if reach.changed_only and function.name in (base_names or ()):
                continue
            findings.append(Finding(code, rule, message, note))
    return findings


def find_gone_approvals(
    files: set[str],
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
) -> list[Finding]:
    """Find approved comments, or their files, that are gone.

    A cut comment that stays gone is what the owner asked for, so only
    entries with text count.
    """
    current = {(c.file, c.name) for c in comments}
    findings = []
    for (file, name), text in sorted(approved.items()):
        if not text:
            continue
        gone = Comment(file=file, line=0, name=name, text="", prose="")
        if file not in files:
            findings.append(Finding(gone, "approved", "approved file is gone"))
        elif (file, name) not in current:
            message = "approved comment is gone"
            candidates = find_rename_candidates(
                name, text, file, comments, approved
            )
            for candidate in candidates:
                pair = shlex.quote(f"{name}={candidate.name}")
                message += (
                    f"; possibly now {candidate.name} at {candidate.line}"
                )
                message += f" (--approve --rename {pair})"
            findings.append(Finding(gone, "approved", message))
    return findings


def find_changed_approval(
    comment: Comment, approved: dict[tuple[str, str], str]
) -> Finding | None:
    """Find a comment whose text differs from what the owner approved."""
    approved_text = approved[(comment.file, comment.name)]
    if approved_text == comment.text:
        return None
    if not approved_text:
        return Finding(comment, "approved", "the owner cut this comment")
    return Finding(comment, "approved", "approved text changed; restore it")


def find_rule_violations(
    files: set[str],
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
    avoid: dict[str, str],
    base_sentences: set[tuple[str, str]] | None,
) -> list[Finding]:
    """Check every comment against the ``RULES``.

    Args:
        files: Paths of the checked files, relative to the repository.
        approved: Approved text by ``(file, name)``; ``""`` means the
            owner cut the comment.
        avoid: Words not to use, each with what to say instead.
        base_sentences: ``(suffix, sentence)`` for each comment sentence
            at ``--base`` in the checked languages, or ``None`` without it.

    Returns:
        Gone approved comments, then findings in file order.
    """
    findings = find_gone_approvals(files, comments, approved)
    for comment in comments:
        key = (comment.file, comment.name)
        is_approved = bool(comment.name) and key in approved
        if is_approved:
            changed = find_changed_approval(comment, approved)
            # The owner's text must come back as it was, so other rules
            # would only ask the agent to edit it.
            if changed:
                findings.append(changed)
                continue
        sentences = split_sentences(comment.prose)
        # Prose names code without its qualifier: "Cart.Total" is written
        # "Total", and "const Red" is written "Red".
        name = re.split(r"[. ]", comment.name)[-1]
        matches = [] if is_approved else find_restated_name(sentences, name)
        matches += find_avoided_words(sentences, avoid)
        matches += find_filler(sentences, name)
        matches += find_name_at_start(sentences, name)
        unmarked = list_unmarked_sentences(comment)
        matches += find_long_comment(unmarked)
        matches += find_colons(unmarked)
        suffix = Path(comment.file).suffix
        unchanged = base_sentences is not None and all(
            (suffix, s) in base_sentences for s in sentences
        )
        for match in matches:
            reach = RULES[match.rule]
            if suffix not in reach.suffixes or (
                reach.changed_only and unchanged
            ):
                continue
            # A sentence present at --base may be a person's words, so it
            # never fails the run, and the agent is not asked to cut it.
            present_at_base = base_sentences is not None and all(
                (suffix, s) in base_sentences for s in match.sentences
            )
            if present_at_base and match.rule == "restated-name":
                continue
            note = ""
            if is_approved:
                note = "approved: report only"
            elif comment.header:
                note = "header: report only"
            elif present_at_base:
                note = "existing: report only"
            findings.append(Finding(comment, match.rule, match.message, note))
    return findings
