#!/usr/bin/env python3
"""Experimental Gate 3.5 Worker wrapper around one installed Blender MCP."""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ALLOWED_GATEWAYS = {
    "blender_bridge_status",
    "blender_tool_catalog",
    "search_blender_tools",
    "get_blender_tool_schema",
    "invoke_blender_tool",
}


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


parser = argparse.ArgumentParser()
parser.add_argument("--root", required=True)
parser.add_argument("--client", required=True)
parser.add_argument("--server", required=True)
parser.add_argument("--tool", required=True)
parser.add_argument("--arguments", default="{}")
parser.add_argument("--timeout-seconds", type=float, default=1200.0)
args = parser.parse_args()
started = time.monotonic()
if args.timeout_seconds <= 0:
    raise SystemExit("timeout must be positive")
if args.tool not in ALLOWED_GATEWAYS:
    raise SystemExit(f"gateway not allowed in Gate 3.5 Worker: {args.tool}")
root = Path(args.root).resolve()
manifest = json.loads((root / "workcell.json").read_text(encoding="utf-8"))
source = (root / manifest["sourcePath"]).resolve()
if source != root and root not in source.parents:
    raise SystemExit("source path escapes the workcell root")
expected = manifest["sourceSha256"]
before = digest(source)
if before != expected:
    raise SystemExit("source custody failed before Blender MCP call")
remaining = args.timeout_seconds - (time.monotonic() - started)
if remaining <= 0:
    raise SystemExit("worker call deadline expired before bridge invocation")
completed = subprocess.run(
    [
        sys.executable,
        args.client,
        "--server",
        args.server,
        "--tool",
        args.tool,
        "--arguments",
        args.arguments,
        "--log",
        str(root / "logs" / "bridge-mcp.jsonl"),
        "--timeout-seconds",
        str(remaining),
    ],
    text=True,
    capture_output=True,
    timeout=remaining + 5,
)
after = digest(source)
entry = {
    "at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "pid": os.getpid(),
    "gatewayTool": args.tool,
    "helperTool": json.loads(args.arguments).get("name"),
    "durationSeconds": round(time.monotonic() - started, 6),
    "sourceBeforeSha256": before,
    "sourceAfterSha256": after,
    "sourcePreserved": before == expected and after == expected,
    "bridgeExitCode": completed.returncode,
}
with (root / "logs" / "aya-worker.jsonl").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(entry, separators=(",", ":"), sort_keys=True) + "\n")
if after != expected:
    print("source custody failed after Blender MCP call", file=sys.stderr)
    sys.exit(3)
sys.stdout.write(completed.stdout)
sys.stderr.write(completed.stderr)
sys.exit(completed.returncode)
