"""Tests for the one-package-per-line Dockerfile check."""

import contextlib
import io
import os
import tempfile
import unittest

import check_dockerfile_packages


class TestCheckDockerfilePackages(unittest.TestCase):
    """The check as CI runs it, on Dockerfiles given as arguments."""

    def run_check(self, dockerfile_text):
        """Return the exit code and output for a Dockerfile with this text."""
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, "Dockerfile")
            with open(path, "w", encoding="utf-8") as dockerfile:
                dockerfile.write(dockerfile_text)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                exit_code = check_dockerfile_packages.main([path])
        return exit_code, output.getvalue()

    def test_check_fails_on_crowded_install_line(self):
        """Fails and names the file line holding several packages.

        A failing exit code is what stops CI, and the line number and
        package names tell the author which line to split. apk is
        checked too because its subcommand is add, not install.
        """
        cases = (
            ("apt-get install -y bash git", "bash, git"),
            ("apk add --no-cache bash git", "bash, git"),
            ("pip install requests rich", "requests, rich"),
        )
        for install_line, packages in cases:
            with self.subTest(install_line=install_line):
                dockerfile_text = (
                    "FROM debian\n"
                    "RUN true \\\n"
                    f"    && {install_line} \\\n"
                    "    && rm -rf /var/lib/apt/lists/*\n"
                )

                exit_code, output = self.run_check(dockerfile_text)

                self.assertEqual(exit_code, 1)
                self.assertIn(
                    f"Dockerfile:3: {packages} share one line", output
                )

    def test_check_passes_repo_install_styles(self):
        """Passes the one-per-line forms the repository's Dockerfiles use.

        These forms include a retry loop with tabs, flags on the install
        line, one package after npm install, and commands after the
        install. A false finding here would block every CI run.
        """
        dockerfile_text = (
            "FROM ubuntu\n"
            "RUN for attempt in 1 2 3; do \\\n"
            "\t\tapt-get update \\\n"
            "\t\t&& apt-get install -y --no-install-recommends \\\n"
            "\t\t\tbison \\\n"
            "\t\t\tsudo \\\n"
            "\t\t\t&& break; \\\n"
            '\t\tif [ "$attempt" = 3 ]; then exit 1; fi; \\\n'
            "\t\tsleep 60; \\\n"
            "\tdone \\\n"
            "\t&& useradd -m -s /bin/bash testuser\n"
            "RUN npm install -g pi-coding-agent@0.85.1 \\\n"
            "    && HOME=/opt/vibe pi install npm:pi-models-discovery@1.4.0\n"
        )

        exit_code, output = self.run_check(dockerfile_text)

        self.assertEqual(exit_code, 0)
        self.assertEqual(output, "")


if __name__ == "__main__":
    unittest.main()
