#!/usr/bin/env python3
"""Atomic, credential-free Agent lease and handoff coordination."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import re
import secrets
import subprocess
import sys
from typing import Any


LEASE_MARKER = "agent-lease:v1"
HANDOFF_MARKER = "agent-handoff:v1"
AGENT_RE = re.compile(r"^[A-Za-z0-9_-]+$")
CAP_RE = re.compile(r"^[A-Za-z0-9_.-]+$")
SLUG_RE = re.compile(r"^[a-z0-9-]+$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")


class StateError(RuntimeError):
    pass


def run(*args: str, cwd: pathlib.Path | None = None, input_text: str | None = None,
        check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def output(*args: str, cwd: pathlib.Path | None = None, input_text: str | None = None) -> str:
    return run(*args, cwd=cwd, input_text=input_text).stdout.strip()


def repo_root() -> pathlib.Path:
    return pathlib.Path(output("git", "rev-parse", "--show-toplevel")).resolve()


def utc_now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0)


def iso(value: dt.datetime) -> str:
    return value.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(value: str) -> dt.datetime:
    return dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)


def parse_caps(value: str) -> list[str]:
    caps = value.split(",")
    if not caps or any(not CAP_RE.fullmatch(cap) for cap in caps) or len(set(caps)) != len(caps):
        raise StateError("capabilities must be unique comma-separated non-secret tags")
    return caps


def encode_state(marker: str, data: dict[str, Any]) -> str:
    return f"{marker}\n{json.dumps(data, sort_keys=True, separators=(',', ':'))}\n"


def parse_state(message: str, marker: str) -> dict[str, Any]:
    lines = message.rstrip("\n").splitlines()
    if len(lines) != 2 or lines[0] != marker:
        raise StateError(f"invalid {marker} state envelope")
    try:
        data = json.loads(lines[1])
    except json.JSONDecodeError as exc:
        raise StateError(f"invalid {marker} JSON") from exc
    if not isinstance(data, dict):
        raise StateError(f"invalid {marker} state object")
    return data


def capsule_field(text: str, prefix: str) -> str:
    values = [line[len(prefix):] for line in text.splitlines() if line.startswith(prefix)]
    if len(values) != 1 or not values[0]:
        raise StateError(f"handoff capsule must contain exactly one {prefix.strip()} field")
    return values[0]


def parse_capsule(text: str) -> dict[str, Any]:
    first_line = text.splitlines()[0] if text.splitlines() else ""
    match = re.fullmatch(r"# Agent handoff: Issue ([1-9][0-9]*)", first_line)
    if not match:
        raise StateError("handoff capsule has an invalid Issue heading")
    credentials = capsule_field(text, "- Credentials included: ")
    if credentials != "no":
        raise StateError("handoff capsule must not include credentials")
    return {
        "agent_id": capsule_field(text, "- Source agent ID: "),
        "branch": capsule_field(text, "- Branch: "),
        "capabilities": parse_caps(capsule_field(text, "- Capabilities used: ")),
        "credentials_included": credentials,
        "issue": int(match.group(1)),
    }


def validate_issue(value: str) -> int:
    if not value.isdigit() or int(value) <= 0:
        raise StateError("issue-number must be a positive integer")
    return int(value)


def validate_agent(value: str) -> str:
    if not AGENT_RE.fullmatch(value):
        raise StateError("agent-id contains invalid characters")
    return value


def issue_data(issue: int) -> dict[str, Any]:
    return json.loads(output("gh", "issue", "view", str(issue), "--json", "state,labels"))


def required_caps(data: dict[str, Any]) -> set[str]:
    return {
        label["name"].removeprefix("cap:")
        for label in data["labels"]
        if label["name"].startswith("cap:")
    }


def validate_issue_for_lease(issue: int, caps: list[str]) -> dict[str, Any]:
    data = issue_data(issue)
    labels = {label["name"] for label in data["labels"]}
    if data["state"] != "OPEN":
        raise StateError(f"issue #{issue} is not open")
    terminal = labels & {"status:blocked", "status:review", "status:done"}
    if terminal:
        raise StateError(f"issue #{issue} cannot be leased while labeled {sorted(terminal)[0]}")
    missing = required_caps(data) - set(caps)
    if missing:
        raise StateError(f"agent is missing required capabilities: {','.join(sorted(missing))}")
    return data


def state_ref(kind: str, issue: int) -> str:
    return f"refs/heads/agent-{kind}/issue-{issue}"


def remote_ref_sha(ref: str) -> str | None:
    text = output("git", "ls-remote", "--refs", "origin", ref)
    if not text:
        return None
    parts = text.splitlines()
    if len(parts) != 1:
        raise StateError(f"ambiguous remote ref: {ref}")
    sha, returned_ref = parts[0].split("\t", 1)
    if returned_ref != ref or not SHA_RE.fullmatch(sha):
        raise StateError(f"invalid remote ref response: {ref}")
    return sha


def read_remote_state(ref: str, marker: str) -> tuple[str | None, dict[str, Any] | None]:
    sha = remote_ref_sha(ref)
    if sha is None:
        return None, None
    run("git", "fetch", "--no-tags", "origin", ref)
    message = output("git", "show", "-s", "--format=%B", sha)
    return sha, parse_state(message, marker)


def build_state_commit(marker: str, data: dict[str, Any], parent: str | None) -> str:
    tree = output("git", "mktree", input_text="")
    args = ["git", "commit-tree", tree]
    if parent:
        args.extend(["-p", parent])
    return output(*args, input_text=encode_state(marker, data))


def push_state_updates(updates: list[tuple[str, str, str | None]]) -> None:
    args = ["git", "push", "--porcelain"]
    if len(updates) > 1:
        args.append("--atomic")
    args.extend(f"--force-with-lease={ref}:{expected or ''}" for ref, _, expected in updates)
    args.append("origin")
    args.extend(f"{commit}:{ref}" for ref, commit, _ in updates)
    pushed = run(*args, check=False)
    if pushed.returncode != 0:
        refs = ",".join(ref for ref, _, _ in updates)
        raise StateError(f"atomic state update lost a race for {refs}: {pushed.stderr.strip()}")
    for ref, commit, _ in updates:
        if remote_ref_sha(ref) != commit:
            raise StateError(f"remote state verification failed for {ref}")


def create_state_commit(ref: str, marker: str, data: dict[str, Any], expected: str | None) -> str:
    commit = build_state_commit(marker, data, expected)
    push_state_updates([(ref, commit, expected)])
    return commit


def update_status(issue: int, target: str) -> None:
    data = issue_data(issue)
    labels = {label["name"] for label in data["labels"]}
    for old in {"status:ready", "status:claimed", "status:active", "status:handoff"} - {target}:
        if old in labels:
            run("gh", "issue", "edit", str(issue), "--remove-label", old)
    if target not in labels:
        run("gh", "issue", "edit", str(issue), "--add-label", target)


def add_comment(issue: int, marker: str, data: dict[str, Any]) -> None:
    body = f"<!-- {marker} -->\n```json\n{json.dumps(data, indent=2, sort_keys=True)}\n```"
    run("gh", "issue", "comment", str(issue), "--body", body)


def acquire_lease(issue: int, agent: str, caps: list[str], ttl: int) -> tuple[str, dict[str, Any]]:
    validate_issue_for_lease(issue, caps)
    ref = state_ref("leases", issue)
    old_sha, old = read_remote_state(ref, LEASE_MARKER)
    now = utc_now()
    if old:
        if old.get("issue") != issue:
            raise StateError("lease state Issue mismatch")
        active = old.get("state") == "active" and parse_iso(str(old["expires_at"])) > now
        if active and old.get("agent_id") != agent:
            raise StateError(f"issue #{issue} is leased by {old.get('agent_id')} until {old.get('expires_at')}")
    data = {
        "agent_id": agent,
        "capabilities": caps,
        "expires_at": iso(now + dt.timedelta(minutes=ttl)),
        "issue": issue,
        "nonce": secrets.token_hex(16),
        "started_at": iso(now),
        "state": "active",
    }
    sha = create_state_commit(ref, LEASE_MARKER, data, old_sha)
    update_status(issue, "status:active")
    add_comment(issue, LEASE_MARKER, {**data, "state_commit": sha, "credentials_shared": False})
    return sha, data


def current_lease(issue: int, agent: str) -> tuple[str, dict[str, Any]]:
    sha, data = read_remote_state(state_ref("leases", issue), LEASE_MARKER)
    if not sha or not data:
        raise StateError(f"issue #{issue} has no lease")
    if data.get("issue") != issue or data.get("agent_id") != agent or data.get("state") != "active":
        raise StateError("handoff publisher does not own the active lease")
    if parse_iso(str(data["expires_at"])) <= utc_now():
        raise StateError("active lease has expired")
    return sha, data


def release_lease(issue: int, lease_sha: str, lease: dict[str, Any], reason: str) -> str:
    released = {**lease, "released_at": iso(utc_now()), "release_reason": reason, "state": "released"}
    return create_state_commit(state_ref("leases", issue), LEASE_MARKER, released, lease_sha)


def resolve_handoff(issue: int) -> tuple[str | None, dict[str, Any] | None]:
    sha, data = read_remote_state(state_ref("handoffs", issue), HANDOFF_MARKER)
    if data and data.get("issue") != issue:
        raise StateError("handoff state Issue mismatch")
    return sha, data


def verify_handoff_source(root: pathlib.Path, issue: int, handoff: dict[str, Any]) -> str:
    branch = str(handoff.get("branch", ""))
    commit = str(handoff.get("commit", ""))
    if not branch.startswith(f"codex/issue-{issue}-") or not SHA_RE.fullmatch(commit):
        raise StateError("handoff branch or commit is invalid")
    remote_branch_ref = f"refs/heads/{branch}"
    tip = remote_ref_sha(remote_branch_ref)
    if not tip:
        raise StateError("handoff source branch is missing")
    run("git", "fetch", "--no-tags", "origin", remote_branch_ref, cwd=root)
    ancestor = run("git", "merge-base", "--is-ancestor", commit, tip, cwd=root, check=False)
    if ancestor.returncode != 0:
        raise StateError("handoff commit is not reachable from the declared remote branch")
    capsule_path = f".handoffs/issue-{issue}.md"
    capsule = run("git", "show", f"{commit}:{capsule_path}", cwd=root, check=False)
    if capsule.returncode != 0 or not capsule.stdout.strip():
        raise StateError("handoff commit does not contain its capsule")
    capsule_state = parse_capsule(capsule.stdout)
    expected = {
        "agent_id": handoff.get("agent_id"),
        "branch": branch,
        "capabilities": handoff.get("capabilities"),
        "credentials_included": "no",
        "issue": issue,
    }
    if capsule_state != expected:
        raise StateError("handoff capsule identity does not match authoritative state")
    return commit


def command_lease(args: argparse.Namespace) -> None:
    issue = validate_issue(args.issue)
    agent = validate_agent(args.agent)
    caps = parse_caps(args.capabilities)
    sha, data = acquire_lease(issue, agent, caps, args.ttl)
    print(f"leased issue #{issue} to {agent} until {data['expires_at']} ({sha})")


def command_takeover(args: argparse.Namespace) -> None:
    issue = validate_issue(args.issue)
    agent = validate_agent(args.agent)
    if not SLUG_RE.fullmatch(args.slug):
        raise StateError("slug must contain lowercase letters, digits, or hyphens")
    caps = parse_caps(args.capabilities)
    root = repo_root()
    lease_sha, lease = acquire_lease(issue, agent, caps, args.ttl)
    stamp = utc_now().strftime("%Y%m%d%H%M%S")
    suffix = lease["nonce"][:8]
    branch = f"codex/issue-{issue}-{args.slug}-{agent}-{stamp}-{suffix}"
    worktree = root.parent / f"wlg-agent-{issue}-{agent}-{stamp}-{suffix}"
    created = False
    handoff: dict[str, Any] | None = None
    try:
        _, handoff = resolve_handoff(issue)
        if handoff:
            start = verify_handoff_source(root, issue, handoff)
        else:
            run("git", "fetch", "--no-tags", "origin", "main", cwd=root)
            start = output("git", "rev-parse", "origin/main", cwd=root)
        run("git", "worktree", "add", "-b", branch, str(worktree), start, cwd=root)
        created = True
        if handoff:
            run(str(worktree / "scripts/ci/verify-handoffs.sh"), cwd=worktree)
    except Exception:
        if created:
            run("git", "worktree", "remove", "--force", str(worktree), cwd=root, check=False)
            run("git", "branch", "-D", branch, cwd=root, check=False)
        try:
            released_sha = release_lease(issue, lease_sha, lease, "takeover_failed")
            if remote_ref_sha(state_ref("leases", issue)) == released_sha:
                update_status(issue, "status:handoff" if handoff else "status:ready")
        except Exception as cleanup_error:
            print(f"warning: lease rollback failed: {cleanup_error}", file=sys.stderr)
        raise
    print(f"worktree: {worktree}\nbranch: {branch}\nstart: {start}")


def command_handoff(args: argparse.Namespace) -> None:
    issue = validate_issue(args.issue)
    agent = validate_agent(args.agent)
    caps = parse_caps(args.capabilities)
    root = repo_root()
    os.chdir(root)
    validate_issue_for_lease(issue, caps)
    _, lease = current_lease(issue, agent)
    if set(caps) != set(lease.get("capabilities", [])):
        raise StateError("handoff capabilities do not match the active lease")
    branch = output("git", "branch", "--show-current", cwd=root)
    if not branch.startswith(f"codex/issue-{issue}-"):
        raise StateError(f"branch {branch} does not belong to issue #{issue}")
    capsule = root / ".handoffs" / f"issue-{issue}.md"
    if not capsule.is_file() or not capsule.stat().st_size:
        raise StateError(f"missing handoff capsule: {capsule}")
    capsule_state = parse_capsule(capsule.read_text(encoding="utf-8"))
    if capsule_state["issue"] != issue or capsule_state["agent_id"] != agent:
        raise StateError("handoff capsule Issue or Agent does not match the active lease")
    if capsule_state["branch"] != branch or set(capsule_state["capabilities"]) != set(caps):
        raise StateError("handoff capsule branch or capabilities do not match the active lease")
    run(str(root / "scripts/ci/verify.sh"), cwd=root)
    if output("git", "status", "--short", cwd=root):
        raise StateError("handoff requires a clean worktree; commit all resumable state first")
    commit = output("git", "rev-parse", "HEAD", cwd=root)
    run("git", "push", "-u", "origin", branch, cwd=root)
    if remote_ref_sha(f"refs/heads/{branch}") != commit:
        raise StateError("remote working branch tip does not match the local checkpoint")
    lease_sha, lease = current_lease(issue, agent)
    if set(caps) != set(lease.get("capabilities", [])):
        raise StateError("renewed handoff capabilities do not match the active lease")
    old_handoff_sha, _ = resolve_handoff(issue)
    handoff = {
        "agent_id": agent,
        "branch": branch,
        "capabilities": caps,
        "capsule": f".handoffs/issue-{issue}.md",
        "commit": commit,
        "issue": issue,
        "lease_nonce": lease["nonce"],
        "released_at": iso(utc_now()),
    }
    handoff_ref = state_ref("handoffs", issue)
    lease_ref = state_ref("leases", issue)
    handoff_sha = build_state_commit(HANDOFF_MARKER, handoff, old_handoff_sha)
    released = {**lease, "released_at": iso(utc_now()), "release_reason": "handoff_published", "state": "released"}
    released_sha = build_state_commit(LEASE_MARKER, released, lease_sha)
    push_state_updates([
        (handoff_ref, handoff_sha, old_handoff_sha),
        (lease_ref, released_sha, lease_sha),
    ])
    update_status(issue, "status:handoff")
    add_comment(issue, HANDOFF_MARKER, {**handoff, "state_commit": handoff_sha, "credentials_shared": False})
    print(f"published handoff for issue #{issue} at {commit} ({handoff_sha})")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    sub = result.add_subparsers(dest="command", required=True)
    lease = sub.add_parser("lease")
    lease.add_argument("issue")
    lease.add_argument("agent")
    lease.add_argument("capabilities")
    lease.add_argument("--ttl", type=int, default=120, choices=range(15, 1441), metavar="MINUTES")
    lease.set_defaults(func=command_lease)
    takeover = sub.add_parser("takeover")
    takeover.add_argument("issue")
    takeover.add_argument("agent")
    takeover.add_argument("slug")
    takeover.add_argument("capabilities")
    takeover.add_argument("--ttl", type=int, default=120, choices=range(15, 1441), metavar="MINUTES")
    takeover.set_defaults(func=command_takeover)
    handoff = sub.add_parser("handoff")
    handoff.add_argument("issue")
    handoff.add_argument("agent")
    handoff.add_argument("capabilities")
    handoff.set_defaults(func=command_handoff)
    return result


def main() -> int:
    try:
        args = parser().parse_args()
        args.func(args)
    except (StateError, subprocess.CalledProcessError, ValueError, KeyError) as exc:
        if isinstance(exc, subprocess.CalledProcessError):
            detail = exc.stderr.strip() or str(exc)
        else:
            detail = str(exc)
        print(f"error: {detail}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
