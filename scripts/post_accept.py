#!/usr/bin/env python3
"""Read-only post-run acceptance prototype.

Reads a 1agent run journal, checks that every recorded write/edit artifact still
matches its recorded sha256, checks tool-call/result pairing and answer evidence,
and emits a delivery manifest. This does not change runtime behavior, the ledger,
or the answer; it measures whether post-run deterministic acceptance is feasible.

Run one journal:

    python3 scripts/post_accept.py --journal <run.jsonl> --json

Batch a Phase 4 results file:

    python3 scripts/post_accept.py --results docs/evidence/.../results.json --json

Self-check:

    python3 scripts/post_accept.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

ARTIFACT_TOOLS = {"write_file", "edit_file"}


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str | None:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError:
        return None


def load_events(path: Path) -> tuple[list[dict], list[str]]:
    events: list[dict] = []
    errors: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as error:
        return [], [f"cannot read journal: {error}"]
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            errors.append(f"line {number}: invalid JSON: {error}")
            continue
        event = record.get("event")
        if not isinstance(event, dict):
            errors.append(f"line {number}: missing event object")
            continue
        if not isinstance(event.get("type"), str):
            errors.append(f"line {number}: missing event type")
            continue
        events.append(event)
    return events, errors


def event_data(event: dict) -> dict:
    data = event.get("data")
    return data if isinstance(data, dict) else {}


def pairing_checks(events: list[dict]) -> list[dict]:
    calls: list[str] = []
    observations: list[str] = []
    for event in events:
        kind = event.get("type")
        data = event_data(event)
        if kind == "tool_called":
            call = data.get("call") or {}
            if isinstance(call, dict) and isinstance(call.get("id"), str):
                calls.append(call["id"])
        elif kind == "tool_observed":
            observation = data.get("observation") or {}
            if isinstance(observation, dict) and isinstance(observation.get("call_id"), str):
                observations.append(observation["call_id"])
    missing = sorted(set(calls) - set(observations))
    extra = sorted(set(observations) - set(calls))
    duplicates = sorted(
        call_id for call_id in set(calls) if calls.count(call_id) > 1
    )
    return [
        {
            "id": "journal:tool_calls_paired",
            "passed": not missing and not extra and not duplicates,
            "detail": {
                "calls": len(calls),
                "observations": len(observations),
                "missing_observations": missing,
                "extra_observations": extra,
                "duplicate_calls": duplicates,
            },
        }
    ]


def collect_artifacts(events: list[dict]) -> tuple[list[dict], list[str], bool, bool]:
    """Return the last recorded write/edit artifact per path.

    Also reports an unrecorded write/edit observation and whether a terminal
    command may have written without a recorded deliverable.
    """
    artifacts: dict[str, dict] = {}
    reasons: list[str] = []
    unrecorded_write = False
    terminal_write_possible = False
    for index, event in enumerate(events):
        if event.get("type") != "tool_observed":
            continue
        observation = event_data(event).get("observation") or {}
        if not isinstance(observation, dict):
            continue
        if observation.get("status") != "ok":
            continue
        tool = observation.get("tool")
        metadata = observation.get("metadata") or {}
        if not isinstance(metadata, dict):
            metadata = {}
        if tool in ARTIFACT_TOOLS:
            path = metadata.get("path")
            digest = metadata.get("sha256") or metadata.get("sha256_after")
            if isinstance(path, str) and isinstance(digest, str):
                artifacts[path] = {
                    "path": path,
                    "recorded_sha256": digest,
                    "recorded_bytes": metadata.get("bytes")
                    or metadata.get("bytes_after"),
                    "tool": tool,
                    "seq": index,
                }
            else:
                unrecorded_write = True
                reasons.append(f"{tool} observation has no path/sha256")
        elif tool == "terminal_exec":
            # New journals record write=true|false. Older journals default to
            # a possible write because the command may have changed files.
            if metadata.get("write") != "false":
                terminal_write_possible = True
    return list(artifacts.values()), reasons, unrecorded_write, terminal_write_possible


def collect_answer(events: list[dict]) -> dict | None:
    answer = None
    for event in events:
        if event.get("type") == "answer_produced":
            candidate = event_data(event).get("answer")
            if isinstance(candidate, dict):
                answer = candidate
    return answer


def answer_sources(answer: dict) -> list[str]:
    sources: list[str] = []
    claims = answer.get("claims")
    if isinstance(claims, list):
        for claim in claims:
            if not isinstance(claim, dict):
                continue
            for source in claim.get("sources") or []:
                if isinstance(source, str):
                    sources.append(source)
    evidence = answer.get("evidence")
    if isinstance(evidence, list):
        for item in evidence:
            if isinstance(item, dict):
                source = item.get("source") or item.get("path")
                if isinstance(source, str):
                    sources.append(source)
            elif isinstance(item, str):
                sources.append(item)
    return sorted(set(sources))


def build_manifest(journal: Path) -> dict:
    events, errors = load_events(journal)
    started = next((event for event in events if event.get("type") == "run_started"), None)
    answer = collect_answer(events)
    artifacts, artifact_reasons, unrecorded_write, terminal_write_possible = collect_artifacts(events)
    unpinned_write = not artifacts and terminal_write_possible
    request = (event_data(started).get("request") if started else {}) or {}
    objective = request.get("objective") if isinstance(request, dict) else None
    run_id = journal.stem
    for event in events:
        if event.get("type") == "run_started":
            candidate = event_data(event).get("run_id")
            if isinstance(candidate, str) and candidate:
                run_id = candidate
            break

    checks: list[dict] = [
        {
            "id": "journal:parsed",
            "passed": not errors,
            "detail": errors[:20],
        },
        {
            "id": "journal:run_started",
            "passed": started is not None and bool(objective),
            "detail": {"objective": objective},
        },
    ]
    checks.extend(pairing_checks(events))

    artifact_rows: list[dict] = []
    for artifact in artifacts:
        path = Path(artifact["path"])
        current = sha256_file(path)
        row = {
            "path": artifact["path"],
            "recorded_sha256": artifact["recorded_sha256"],
            "current_sha256": current,
            "exists": path.is_file(),
            "match": current == artifact["recorded_sha256"],
            "tool": artifact["tool"],
        }
        artifact_rows.append(row)
        checks.append(
            {
                "id": f"artifact:{artifact['path']}",
                "passed": bool(row["exists"] and row["match"]),
                "detail": row,
            }
        )

    if unrecorded_write:
        checks.append(
            {
                "id": "journal:recorded_write_artifact",
                "passed": False,
                "detail": {
                    "reason": "a write/edit observation has no recorded path/sha256",
                    "reasons": artifact_reasons,
                },
            }
        )
    if unpinned_write:
        checks.append(
            {
                "id": "journal:terminal_write_unpinned",
                "passed": False,
                "detail": {
                    "reason": "a terminal command may have written, but no write/edit artifact was recorded",
                },
            }
        )

    answer_checks: list[dict] = []
    if answer is None:
        answer_checks.append(
            {
                "id": "answer:present",
                "passed": False,
                "detail": "no answer_produced event",
            }
        )
        sources: list[str] = []
    else:
        conclusion = answer.get("conclusion")
        status = answer.get("status")
        claims = answer.get("claims") if isinstance(answer.get("claims"), list) else []
        verified = [
            claim
            for claim in claims
            if isinstance(claim, dict) and claim.get("status") == "verified"
        ]
        sources = answer_sources(answer)
        # Claim sources may be tool references such as
        # `verify_artifact:call_...`; only absolute paths are filesystem checks.
        missing_sources = [
            source
            for source in sources
            if source.startswith("/") and not Path(source).exists()
        ]
        answer_checks.extend(
            [
                {
                    "id": "answer:present",
                    "passed": isinstance(conclusion, str) and bool(conclusion.strip()),
                    "detail": {"status": status, "conclusion_chars": len(conclusion or "")},
                },
                {
                    "id": "answer:claims",
                    "passed": bool(claims) and bool(verified),
                    "detail": {"claims": len(claims), "verified": len(verified)},
                },
                {
                    "id": "answer:sources_exist",
                    "passed": not missing_sources,
                    "detail": {"sources": sources, "missing": missing_sources},
                },
            ]
        )
    checks.extend(answer_checks)

    proof_ok = all(
        check["passed"]
        for check in checks
        if check["id"]
        not in {
            "answer:claims",
            "answer:sources_exist",
            "journal:terminal_write_unpinned",
        }
    )
    answer_ok = all(check["passed"] for check in answer_checks)
    if not proof_ok:
        decision = "rejected"
    elif unpinned_write or not answer_ok:
        decision = "conditional"
    else:
        decision = "accepted"

    manifest = {
        "schema": "1agent.post-accept/v1",
        "journal": str(journal),
        "run_id": run_id,
        "objective": objective,
        "decision": decision,
        "proof_ok": proof_ok,
        "answer_ok": answer_ok,
        "unpinned_write": unpinned_write,
        "checks": checks,
        "artifacts": artifact_rows,
        "answer": {
            "status": answer.get("status") if answer else None,
            "claims": len(answer.get("claims") or []) if answer else 0,
            "sources": sources,
        },
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }
    canonical = json.dumps(
        {key: manifest[key] for key in ("schema", "run_id", "objective", "decision", "proof_ok", "answer_ok", "artifacts")},
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    )
    manifest["manifest_sha256"] = sha256_bytes(canonical.encode("utf-8"))
    return manifest


def batch(results_path: Path) -> dict:
    results = json.loads(results_path.read_text(encoding="utf-8"))
    records = results.get("records") or []
    manifests: list[dict] = []
    missing = 0
    for record in records:
        journal = Path(record.get("journal") or "")
        if not journal.is_file():
            missing += 1
            continue
        manifest = build_manifest(journal)
        manifest["arm"] = record.get("arm")
        manifest["case"] = record.get("case")
        manifest["run"] = record.get("run")
        manifest["runtime_status"] = record.get("answer_status")
        manifests.append(manifest)

    decisions: dict[str, int] = {}
    by_arm: dict[str, dict[str, int]] = {}
    by_case: dict[str, dict[str, int]] = {}
    failure_reasons: dict[str, int] = {}
    for manifest in manifests:
        decision = manifest["decision"]
        decisions[decision] = decisions.get(decision, 0) + 1
        for bucket, key in ((by_arm, manifest.get("arm")), (by_case, manifest.get("case"))):
            if key is None:
                continue
            counts = bucket.setdefault(key, {})
            counts[decision] = counts.get(decision, 0) + 1
        for check in manifest["checks"]:
            if not check["passed"]:
                failure_reasons[check["id"]] = failure_reasons.get(check["id"], 0) + 1
    return {
        "schema": "1agent.post-accept-batch/v1",
        "results": str(results_path),
        "journals": len(manifests),
        "missing_journals": missing,
        "decisions": decisions,
        "by_arm": by_arm,
        "by_case": by_case,
        "failure_reasons": failure_reasons,
        "manifests": manifests,
    }


def self_test() -> int:
    with tempfile.TemporaryDirectory(prefix="post-accept-") as temp:
        root = Path(temp)
        artifact = root / "out.txt"
        artifact.write_text("evidence", encoding="utf-8")
        digest = sha256_file(artifact)
        started = {
            "event": {
                "type": "run_started",
                "data": {
                    "run_id": "run-1",
                    "request": {"objective": "write out.txt", "permissions": {"allow_write": True}},
                },
            }
        }
        call = {
            "event": {
                "type": "tool_called",
                "data": {"call": {"id": "c1", "name": "write_file", "arguments": {"path": str(artifact)}}},
            }
        }
        observation = {
            "event": {
                "type": "tool_observed",
                "data": {
                    "observation": {
                        "call_id": "c1",
                        "tool": "write_file",
                        "status": "ok",
                        "metadata": {"path": str(artifact), "sha256": digest, "bytes": "8"},
                    }
                },
            }
        }
        answer = {
            "event": {
                "type": "answer_produced",
                "data": {
                    "answer": {
                        "status": "completed",
                        "conclusion": "wrote evidence",
                        "claims": [
                            {
                                "id": "c1",
                                "text": "out.txt is evidence",
                                "status": "verified",
                                "sources": [str(artifact)],
                            }
                        ],
                        "evidence": [{"source": str(artifact)}],
                    }
                },
            }
        }
        stopped = {"event": {"type": "run_stopped", "data": {"reason": "completed"}}}
        journal = root / "run-1.jsonl"
        journal.write_text(
            "".join(json.dumps(event, ensure_ascii=False) + "\n" for event in (started, call, observation, answer, stopped)),
            encoding="utf-8",
        )
        accepted = build_manifest(journal)
        assert accepted["decision"] == "accepted", accepted
        assert accepted["artifacts"][0]["match"] is True

        artifact.write_text("tampered", encoding="utf-8")
        rejected = build_manifest(journal)
        assert rejected["decision"] == "rejected", rejected
        assert rejected["artifacts"][0]["match"] is False

        artifact.write_text("evidence", encoding="utf-8")
        journal.write_text(
            "".join(
                json.dumps(event, ensure_ascii=False) + "\n"
                for event in (started, call, answer, stopped)
            ),
            encoding="utf-8",
        )
        missing_observation = build_manifest(journal)
        assert missing_observation["decision"] == "rejected", missing_observation

        no_claims = json.loads(json.dumps(answer))
        no_claims["event"]["data"]["answer"]["claims"] = []
        journal.write_text(
            "".join(
                json.dumps(event, ensure_ascii=False) + "\n"
                for event in (started, call, observation, no_claims, stopped)
            ),
            encoding="utf-8",
        )
        conditional = build_manifest(journal)
        assert conditional["decision"] == "conditional", conditional

        directory_source = json.loads(json.dumps(answer))
        directory_source["event"]["data"]["answer"]["claims"][0]["sources"] = [str(root)]
        journal.write_text(
            "".join(
                json.dumps(event, ensure_ascii=False) + "\n"
                for event in (started, call, observation, directory_source, stopped)
            ),
            encoding="utf-8",
        )
        directory_ok = build_manifest(journal)
        assert directory_ok["decision"] == "accepted", directory_ok

        terminal_call = {
            "event": {
                "type": "tool_called",
                "data": {"call": {"id": "t1", "name": "terminal_exec", "arguments": {"command": "ls"}}},
            }
        }
        terminal_observation = {
            "event": {
                "type": "tool_observed",
                "data": {
                    "observation": {
                        "call_id": "t1",
                        "tool": "terminal_exec",
                        "status": "ok",
                        "metadata": {"write": "true"},
                    }
                },
            }
        }
        journal.write_text(
            "".join(
                json.dumps(event, ensure_ascii=False) + "\n"
                for event in (started, terminal_call, terminal_observation, answer, stopped)
            ),
            encoding="utf-8",
        )
        unpinned = build_manifest(journal)
        assert unpinned["decision"] == "conditional", unpinned
        assert unpinned["unpinned_write"] is True, unpinned
    print("post_accept self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--journal", type=Path)
    parser.add_argument("--results", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if args.results:
        output = batch(args.results)
    elif args.journal:
        output = build_manifest(args.journal)
    else:
        parser.error("one of --journal, --results or --self-test is required")

    text = json.dumps(output, ensure_ascii=False, indent=2)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    if args.json or not args.out:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
