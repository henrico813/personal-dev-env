"""Check a comment against the RULES, and find gone approvals."""

from __future__ import annotations

import re
import shlex
from pathlib import Path
from typing import NamedTuple

from .languages import LANGUAGES
from .records import Comment, Finding

# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "approved": (
        "On a later pass over unchanged behavior, edit existing prose only "
        "to fix a specific false or unsupported claim, an omission that "
        "makes it misleading for safe use, or an applicable convention "
        "violation; otherwise leave it byte-for-byte unchanged."
    ),
    "restated-name": (
        "Prose derived directly from a name, signature, single statement, "
        "or a test's name, inputs, and assertions is a restatement and "
        "must not be added."
    ),
    "avoided-word": (
        "Explain project-specific terms when a new contributor would not "
        "know them."
    ),
    "filler": "Avoid vague filler such as:",
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


class RuleMatch(NamedTuple):
    """A rule a comment breaks, before the ``--base`` policy applies."""

    rule: str
    message: str
    sentences: list[str]


def split_sentences(prose: str) -> list[str]:
    """Split prose after ``.``, ``!`` or ``?`` followed by a space."""
    return [s for s in re.split(r"(?<=[.!?])\s+", prose) if s]


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
        for match in matches:
            # A sentence present at --base may be a person's words, so it
            # never fails the run, and the agent is not asked to cut it.
            present_at_base = base_sentences is not None and all(
                (Path(comment.file).suffix, s) in base_sentences
                for s in match.sentences
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
