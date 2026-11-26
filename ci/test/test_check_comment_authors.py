"""Run the comment author check against real git repositories with a linked worktree."""

import os
import subprocess
import sys
import tempfile
import unittest

TOOL = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..",
    "check_comment_authors.py",
)
# an empty global config keeps the user's git settings, such as commit signing, out of the tests
GIT_ENV = {
    **os.environ,
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_AUTHOR_NAME": "Test",
    "GIT_AUTHOR_EMAIL": "test@example.com",
    "GIT_COMMITTER_NAME": "Test",
    "GIT_COMMITTER_EMAIL": "test@example.com",
}


class TestCheckCommentAuthors(unittest.TestCase):
    """Each test gets a repository with a tracked AGENTS.md and a linked worktree on a branch, as wt switch --create gives an agent."""

    def setUp(self):
        """Make the repository, its first commit, and the linked worktree the tests run in."""
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        # HOME points at a test folder whose rule file names a fake model and reason, so no test reads the real rule file.
        home = os.path.join(folder.name, "home")
        self.env = {**GIT_ENV, "HOME": home}
        self.rule_path = os.path.join(home, ".config", "pde", "comment-author-rule.md")
        self.write(home, ".config/pde/comment-author-rule.md", "Model: test-model\nReason: test reason\n")
        self.main = os.path.join(folder.name, "main")
        self.worktree = os.path.join(folder.name, "branch")
        os.mkdir(self.main)
        self.run_git(self.main, "init", "-q", "-b", "main")
        self.write(self.main, "AGENTS.md", "# Rules\n")
        self.run_git(self.main, "add", "AGENTS.md")
        self.run_git(self.main, "commit", "-q", "-m", "init")
        self.run_git(self.main, "worktree", "add", "-q", "-b", "feat", self.worktree)
        self.base = self.run_git(self.worktree, "rev-parse", "HEAD").strip()

    def run_git(self, folder, *args):
        """Run git in folder and return its output."""
        return subprocess.run(
            ["git", *args],
            cwd=folder,
            env=self.env,
            check=True,
            capture_output=True,
            text=True,
        ).stdout

    def write(self, folder, path, text):
        """Write a file under folder, making its parent folders."""
        full_path = os.path.join(folder, path)
        os.makedirs(os.path.dirname(full_path), exist_ok=True)
        with open(full_path, "w", encoding="utf-8") as file:
            file.write(text)

    def read(self, path):
        """Return the text of a file in the worktree."""
        with open(os.path.join(self.worktree, path), encoding="utf-8") as file:
            return file.read()

    def commit(self, path, text):
        """Commit one file with this text on the worktree's branch."""
        self.write(self.worktree, path, text)
        self.run_git(self.worktree, "add", path)
        self.run_git(self.worktree, "commit", "-q", "-m", f"add {path}")

    def run_tool(self, *args, stdin="", folder=None):
        """Run the check script in the worktree, as an agent runs it."""
        return subprocess.run(
            [sys.executable, TOOL, *args],
            cwd=folder or self.worktree,
            env=self.env,
            input=stdin,
            capture_output=True,
            text=True,
        )

    def test_setup_adds_rule_without_dirtying_status(self):
        """setup adds the local rule while git status stays clean.

        Build and ship scripts refuse a dirty worktree, so setup must not dirty it."""
        result = self.run_tool("setup")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.run_git(self.worktree, "status", "--porcelain"), "")
        agents = self.read("AGENTS.md")
        self.assertIn("Model: test-model", agents)
        self.assertIn("Reason: test reason", agents)

    def test_setup_refuses_uncommitted_agents_edit(self):
        """setup refuses an uncommitted AGENTS.md edit.

        skip-worktree would hide the edit, so it would never be committed."""
        self.write(self.worktree, "AGENTS.md", "# Rules\nuser rule\n")

        result = self.run_tool("setup")

        self.assertEqual(result.returncode, 2)
        status = self.run_git(self.worktree, "status", "--porcelain")
        self.assertEqual(status, " M AGENTS.md\n")

    def test_setup_refuses_symlinked_agents_file(self):
        """setup refuses a symlinked AGENTS.md.

        Writing through the link puts the rule in the tracked target, which a commit would publish."""
        self.write(self.worktree, "R.md", "# Rules\n")
        agents_path = os.path.join(self.worktree, "AGENTS.md")
        os.remove(agents_path)
        os.symlink("R.md", agents_path)
        self.run_git(self.worktree, "add", "-A")
        self.run_git(self.worktree, "commit", "-q", "-m", "link AGENTS.md")

        result = self.run_tool("setup")

        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.run_git(self.worktree, "status", "--porcelain"), "")

    def test_setup_twice_adds_rule_once(self):
        """A second setup run leaves one copy of the rule in AGENTS.md

        Agents re-run setup, and a doubled rule would clutter every session's instructions."""
        self.run_tool("setup")

        result = self.run_tool("setup")

        self.assertEqual(result.returncode, 0, result.stderr)
        agents = self.read("AGENTS.md")
        self.assertEqual(agents.count("Reason: test reason"), 1, agents)

    def test_setup_refuses_missing_rule_file(self):
        """Setup exits 2, names the rule file and changes nothing when that file is missing

        Without the file the rule names no model, and a half set up worktree would hide the failure."""
        os.remove(self.rule_path)

        result = self.run_tool("setup")

        self.assertEqual(result.returncode, 2)
        self.assertIn(self.rule_path, result.stderr)
        self.assertEqual(self.read("AGENTS.md"), "# Rules\n")
        self.assertEqual(self.run_git(self.worktree, "status", "--porcelain"), "")
        flags = self.run_git(self.worktree, "ls-files", "-v", "AGENTS.md")
        self.assertTrue(flags.startswith("H "), flags)

    def test_setup_refuses_incomplete_rule_file(self):
        """Setup exits 2 and changes nothing when the rule file lacks a Model line, lacks a Reason line, or has an empty Model value

        A rule with no model or no reason tells agents nothing about who writes comments or why."""
        cases = (
            ("no-model-line", "Reason: test reason\n"),
            ("no-reason-line", "Model: test-model\n"),
            ("empty-model-value", "Model:\nReason: test reason\n"),
        )
        for name, rule in cases:
            with self.subTest(name):
                self.write(os.path.dirname(self.rule_path), "comment-author-rule.md", rule)

                result = self.run_tool("setup")

                self.assertEqual(result.returncode, 2)
                self.assertEqual(self.read("AGENTS.md"), "# Rules\n")
                self.assertEqual(self.run_git(self.worktree, "status", "--porcelain"), "")
                flags = self.run_git(self.worktree, "ls-files", "-v", "AGENTS.md")
                self.assertTrue(flags.startswith("H "), flags)

    def test_teardown_removes_rule_and_clears_flag(self):
        """teardown gives back the committed AGENTS.md and clears skip-worktree.

        Later AGENTS.md edits must show in git status, so the flag has to be cleared."""
        self.run_tool("setup")

        result = self.run_tool("teardown")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.read("AGENTS.md"), "# Rules\n")
        flags = self.run_git(self.worktree, "ls-files", "-v", "AGENTS.md")
        self.assertTrue(flags.startswith("H "), flags)

    def test_teardown_keeps_edits_outside_the_section(self):
        """teardown keeps an AGENTS.md edit made outside the section.

        The edit must show in git status so the agent can commit it instead of losing it."""
        self.run_tool("setup")
        agents = self.read("AGENTS.md")
        self.write(self.worktree, "AGENTS.md", agents + "user rule\n")

        result = self.run_tool("teardown")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.read("AGENTS.md"), "# Rules\nuser rule\n")
        status = self.run_git(self.worktree, "status", "--porcelain")
        self.assertEqual(status, " M AGENTS.md\n")

    def test_check_flags_comments_in_each_language(self):
        """Each language's comment style is found and only the comment line is flagged.

        A comment by another author that the check misses reaches the pull request unchecked."""
        cases = (
            ("python-comment", "a.py", "x = 1\n# set x\n", 2),
            ("python-trailing", "f.py", "x = 1  # set x\n", 1),
            (
                "python-docstring",
                "b.py",
                'def f():\n    """\n    Return one.\n    """\n    return 1\n',
                3,
            ),
            (
                "shell-shebang",
                "bin/run",
                "#!/usr/bin/env bash\n# run it\nls\n",
                2,
            ),
            ("go", "c.go", "package c\n// C is one.\nvar C = 1\n", 2),
            (
                "go-struct-field",
                "t.go",
                "//go:build linux\n\npackage t\n\ntype T struct {\n"
                "\tName string // the name\n}\n",
                6,
            ),
            ("rust-block", "d.rs", "fn d() {}\n/*\n * d\n */\n", 3),
            ("shell-trailing", "s.sh", "ls  # list\n", 1),
            ("toml-template", "c.toml.tmpl", "a = 1\n# a is one\n", 2),
            ("go-template-comment", "e.toml.tmpl", "a = 1\n{{/* set a */}}\n", 2),
            (
                "go-template-block",
                "f.yml.tmpl",
                "a: 1\n{{- /*\nset a\n*/ -}}\n",
                3,
            ),
            ("dotfile", ".chezmoiremove", "x\n# drop x\n", 2),
            ("chezmoi-dotfile", "dot_zshrc", "x=1\n# set x\n", 2),
            ("dockerfile", "Dockerfile", "FROM scratch\n# base\n", 2),
            (
                "python-shebang",
                "g.py",
                "#!/usr/bin/env python3\nx = 1  # set x\n",
                2,
            ),
            ("lua", "init.lua", "local a = 1\n-- a is one\n", 2),
            ("lua-block", "b.lua", "local a = 1\n--[[\na is one\n]]\n", 3),
            ("non-ascii-path", "caf\u00e9.py", "x = 1\n# set x\n", 2),
        )
        self.run_tool("setup")

        for name, path, text, line in cases:
            with self.subTest(name):
                base = self.run_git(self.worktree, "rev-parse", "HEAD").strip()
                self.commit(path, text)

                result = self.run_tool("check", base)

                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertTrue(result.stdout.startswith("1 "), result.stdout)
                self.assertIn(f"{path}:{line}", result.stdout.splitlines())

    def test_check_flags_only_new_comment_lines(self):
        """Only added or changed comment lines count.

        Otherwise every old comment in a file an agent touches blocks the pull request."""
        self.run_tool("setup")
        self.commit("a.py", "# old\nx = 1\n")
        base = self.run_git(self.worktree, "rev-parse", "HEAD").strip()
        self.commit("a.py", "# old\nx = 1\n# new\n")

        result = self.run_tool("check", base)

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout.splitlines()[1:], ["a.py:3"])

    def test_check_flags_comment_in_renamed_file(self):
        """Check flags a comment added to a renamed file at its new path

        Reading the old path would let comments in moved files pass unchecked."""
        self.run_tool("setup")
        self.commit("r.py", "x = 1\ny = 2\nz = 3\n")
        base = self.run_git(self.worktree, "rev-parse", "HEAD").strip()
        self.run_git(self.worktree, "mv", "r.py", "s.py")
        self.commit("s.py", "x = 1\ny = 2\nz = 3\n# new\n")

        result = self.run_tool("check", base)

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout.splitlines()[1:], ["s.py:4"])

    def test_check_flags_comments_despite_diff_attribute(self):
        """Check still finds added comments when .gitattributes turns off diff output for .py files

        Without hunk headers the check could pass silently and let an unrecorded comment ship."""
        self.run_tool("setup")
        self.commit(".gitattributes", "*.py -diff\n")
        base = self.run_git(self.worktree, "rev-parse", "HEAD").strip()
        self.commit("a.py", "x = 1\n# set x\n")

        result = self.run_tool("check", base)

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout.splitlines()[1:], ["a.py:2"])

    def test_check_ignores_comments_main_changed_later(self):
        """check compares the branch with its merge base.

        A comment main changes after the branch starts must not be blamed on the branch."""
        self.run_tool("setup")
        self.write(self.main, "o.py", "# old\nx = 1\n")
        self.run_git(self.main, "add", "o.py")
        self.run_git(self.main, "commit", "-q", "-m", "add o.py")
        self.run_git(self.worktree, "merge", "-q", "--ff-only", "main")
        self.write(self.main, "o.py", "# changed\nx = 1\n")
        self.run_git(self.main, "commit", "-q", "-am", "change o.py")
        self.commit("a.py", "# mine\n")

        result = self.run_tool("check", "main")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout.splitlines()[1:], ["a.py:1"])

    def test_check_passes_after_comment_is_recorded(self):
        """A recorded comment passes with or without its markers.

        Otherwise agents cannot open their pull requests."""
        self.run_tool("setup")
        self.commit("a.py", 'def f():\n    """Return one."""\n    return 1  # one\n')

        self.run_tool("record", stdin="Return one.\n# one\n")
        result = self.run_tool("check", self.base)

        self.assertEqual(result.returncode, 0, result.stdout)

    def test_check_ignores_markdown_and_python_strings(self):
        """Markdown headings and # lines inside Python strings are not comments.

        The check must not fail on prose or on the section text the script keeps in a Python string."""
        self.run_tool("setup")
        self.commit("notes.md", "# Heading\n")
        self.commit("text.py", 'TEXT = """\n# Heading\n"""\n')

        result = self.run_tool("check", self.base)

        self.assertEqual(result.returncode, 0, result.stdout)

    def test_check_fails_on_unknown_file_types(self):
        """A file whose comment style is unknown fails the check even with no comment hits.

        Its comments must never pass unread."""
        self.run_tool("setup")
        self.commit("x.ini", "a = 1\n")

        result = self.run_tool("check", self.base)

        self.assertEqual(result.returncode, 1, result.stderr)
        lines = result.stdout.splitlines()
        self.assertTrue(lines[0].startswith("0 "), result.stdout)
        self.assertIn("x.ini", lines)

    def test_check_from_subfolder_flags_added_comments(self):
        """check run from a subfolder flags the same comment lines as from the top level.

        Agents often run it from a subfolder, and a silent pass there lets comments through."""
        self.run_tool("setup")
        self.commit("sub/a.py", "x = 1\n# set x\n")
        subfolder = os.path.join(self.worktree, "sub")

        result = self.run_tool("check", self.base, folder=subfolder)

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("sub/a.py:2", result.stdout.splitlines())

    def test_check_fails_on_uncommitted_agents_edit(self):
        """check fails and names AGENTS.md when it has an edit outside the section.

        skip-worktree hides that edit, and removing the worktree would lose it unseen."""
        self.run_tool("setup")
        agents = self.read("AGENTS.md")
        self.write(self.worktree, "AGENTS.md", agents + "user rule\n")

        result = self.run_tool("check", self.base)

        self.assertEqual(result.returncode, 1, result.stderr)
        lines = result.stdout.splitlines()
        named = [line for line in lines if "AGENTS.md" in line]
        self.assertEqual(len(named), 1, result.stdout)
