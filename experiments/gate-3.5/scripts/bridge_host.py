#!/usr/bin/env python3
"""Headless host for the unmodified Blender Agent Bridge extension."""

import json
import os
import signal
import sys
import time
from pathlib import Path

import bpy
from bl_ext.gate35.claude_blender import bridge_server, script_runner

root = Path(os.environ["GATE35_ARM_ROOT"]).resolve()
ready_path = root / "logs" / "bridge-ready.json"
stop_path = root / "control" / "stop"
root.joinpath("logs").mkdir(parents=True, exist_ok=True)
root.joinpath("control").mkdir(parents=True, exist_ok=True)
stop_path.unlink(missing_ok=True)
stop_requested = False


def stop_handler(_signum, _frame):
    global stop_requested
    stop_requested = True


signal.signal(signal.SIGTERM, stop_handler)
signal.signal(signal.SIGINT, stop_handler)

trust = script_runner.approve_external_script_trust_window(bpy.context, session=True)
bridge = bridge_server.start_bridge(port=8765, auth_token="")
ready = {
    "blender": bpy.app.version_string,
    "background": bool(bpy.app.background),
    "bridge": bridge,
    "script_trust": trust,
    "pid": os.getpid(),
}
ready_path.write_text(json.dumps(ready, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps(ready, sort_keys=True), flush=True)
if not bridge.get("ok") or not trust.get("ok"):
    sys.exit(2)

try:
    while not stop_requested and not stop_path.exists():
        # Blender background mode has no UI event loop. Pump the extension's
        # documented localhost queue on Blender's main thread.
        bridge_server._process_requests()
        time.sleep(0.01)
finally:
    bridge_server.stop_bridge()
