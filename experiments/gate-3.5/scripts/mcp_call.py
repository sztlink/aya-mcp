#!/usr/bin/env python3
"""One-shot stdio MCP client for the installed Blender Agent Bridge."""

import argparse
import json
import select
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def read_response(process, request_id, deadline):
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f"MCP response {request_id} exceeded the call deadline")
        ready, _, _ = select.select([process.stdout], [], [], remaining)
        if not ready:
            raise TimeoutError(f"MCP response {request_id} exceeded the call deadline")
        line = process.stdout.readline()
        if not line:
            stderr = process.stderr.read()
            raise RuntimeError(f"MCP server exited before response {request_id}: {stderr}")
        message = json.loads(line)
        if message.get("id") == request_id:
            return message


def send(process, payload):
    process.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
    process.stdin.flush()


parser = argparse.ArgumentParser()
parser.add_argument("--server", required=True)
parser.add_argument("--tool", required=True)
parser.add_argument("--arguments", default="{}")
parser.add_argument("--log", required=True)
parser.add_argument("--bridge-url", default="http://127.0.0.1:8765")
parser.add_argument("--timeout-seconds", type=float, default=1200.0)
args = parser.parse_args()
if args.timeout_seconds <= 0:
    raise SystemExit("timeout must be positive")
tool_arguments = json.loads(args.arguments)
started = time.monotonic()
deadline = started + args.timeout_seconds
process = subprocess.Popen(
    [sys.executable, args.server, "--bridge-url", args.bridge_url],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=1,
)
response = None
error = None
try:
    send(
        process,
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "aya-gate35-client", "version": "0.1.0"},
            },
        },
    )
    initialized = read_response(process, 1, deadline)
    if "error" in initialized:
        raise RuntimeError(f"MCP initialize failed: {initialized['error']}")
    send(process, {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
    send(
        process,
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": args.tool, "arguments": tool_arguments},
        },
    )
    response = read_response(process, 2, deadline)
    if "error" in response:
        raise RuntimeError(f"MCP tool failed: {response['error']}")
    result = response["result"]
    if result.get("isError") is True:
        raise RuntimeError(f"MCP tool returned isError: {result.get('content', [])}")
    print(json.dumps(result, separators=(",", ":"), sort_keys=True))
except Exception as exc:
    error = f"{type(exc).__name__}: {exc}"
    print(json.dumps({"error": error}, separators=(",", ":")), file=sys.stderr)
finally:
    try:
        process.stdin.close()
    except Exception:
        pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)

elapsed = time.monotonic() - started
entry = {
    "at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "gatewayTool": args.tool,
    "helperTool": tool_arguments.get("name") if isinstance(tool_arguments, dict) else None,
    "durationSeconds": round(elapsed, 6),
    "ok": error is None,
    "error": error,
}
log_path = Path(args.log)
log_path.parent.mkdir(parents=True, exist_ok=True)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(entry, separators=(",", ":"), sort_keys=True) + "\n")
if error:
    sys.exit(1)
