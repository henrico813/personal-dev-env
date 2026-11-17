"""Find and open the extra repo's fix PR through the Gitea API.

``sync`` calls ``find_pr`` and ``open_pr`` in place of ``gh`` and
``pde-gh-write`` when the repo's fix PRs are on Gitea. ``find_pr``
returns the fields and values that ``gh pr list`` prints, so the rest
of sync is the same for both. No reply or error holds the token.
"""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request

from skill_fix.repos import GiteaRepo, ToolError

# Gitea's largest page by default. Recently updated PRs come first,
# and a batch's PR stays near the top while its fixes come in.
PAGE_SIZE = 50
TIMEOUT = 30
# The Gitea server runs on this PC, so a proxy from the environment,
# such as a company one, must not carry its requests.
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def find_pr(gitea: GiteaRepo, branch: str) -> dict | None:
    """Return the newest PR from branch, open, closed or merged, or None.

    Raises:
        ToolError: Gitea did not answer or refused, or the token file
            cannot be read.
    """
    query = urllib.parse.urlencode(
        {"state": "all", "sort": "recentupdate", "limit": PAGE_SIZE}
    )
    found = call_api(gitea, "GET", f"pulls?{query}")
    if not isinstance(found, list):
        raise ToolError("Gitea sent something other than a list of PRs")
    mine = [pr for pr in found if pr["head"]["ref"] == branch]
    if not mine:
        return None
    return to_gh_fields(max(mine, key=lambda pr: pr["number"]))


def open_pr(gitea: GiteaRepo, branch: str, title: str, body: str) -> int:
    """Open a PR from branch into main and return its number.

    Raises:
        ToolError: Gitea did not answer or refused, or the token file
            cannot be read.
    """
    pr = call_api(
        gitea,
        "POST",
        "pulls",
        {"base": "main", "head": branch, "title": title, "body": body},
    )
    if not isinstance(pr, dict):
        raise ToolError("Gitea sent something other than a PR")
    return pr["number"]


def to_gh_fields(pr: dict) -> dict:
    """Return a Gitea PR in the fields and values that gh prints.

    Gitea marks a merged PR closed with ``merged`` true, and gives
    ``mergeable`` as true or false.
    """
    state = "MERGED" if pr["merged"] else pr["state"].upper()
    return {
        "number": pr["number"],
        "state": state,
        "headRefOid": pr["head"]["sha"],
        "mergeable": "MERGEABLE" if pr["mergeable"] else "CONFLICTING",
    }


def call_api(
    gitea: GiteaRepo, method: str, path: str, payload: dict | None = None
) -> object:
    """Send one request to the repo's API and return its JSON answer.

    Raises:
        ToolError: Gitea did not answer, answered with an error or
            with something other than JSON, or the token file cannot
            be read.
    """
    data = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(
        f"{gitea.api_url}/{path}",
        data=data,
        method=method,
        headers={
            "Authorization": f"token {read_token(gitea)}",
            "Accept": "application/json",
            "Content-Type": "application/json",
        },
    )
    name = path.split("?")[0]
    try:
        with OPENER.open(request, timeout=TIMEOUT) as response:
            return json.loads(response.read())
    except urllib.error.HTTPError as error:
        raise ToolError(
            f"Gitea {method} {name} failed: HTTP {error.code}"
        ) from error
    except OSError as error:
        raise ToolError(f"Gitea did not answer at {gitea.api_url}") from error
    except ValueError as error:
        raise ToolError(f"Gitea {method} {name} sent no JSON") from error


def read_token(gitea: GiteaRepo) -> str:
    """Return the API token from its file.

    Raises:
        ToolError: The file cannot be read or is empty.
    """
    try:
        token = gitea.token_file.read_text(encoding="utf-8").strip()
    except OSError as error:
        raise ToolError(
            f"cannot read the Gitea token file {gitea.token_file}"
        ) from error
    if not token:
        raise ToolError(f"the Gitea token file {gitea.token_file} is empty")
    return token
