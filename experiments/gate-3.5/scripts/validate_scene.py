#!/usr/bin/env python3
"""Deterministic technical checks against the saved benchmark candidate."""

import hashlib
import json
import os
import re
from pathlib import Path

import bpy


def sha256_file(file_path: Path) -> str:
    digest = hashlib.sha256()
    with file_path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


root = Path(os.environ["GATE35_ARM_ROOT"]).resolve()
candidate = Path(os.environ.get("GATE35_CANDIDATE_PATH", root / "work" / "candidate.blend")).resolve()
if not candidate.is_file():
    raise RuntimeError(f"candidate does not exist: {candidate}")

bpy.ops.wm.open_mainfile(filepath=str(candidate))
objects = list(bpy.data.objects)
meshes = [obj for obj in objects if obj.type == "MESH"]
cameras = [obj for obj in objects if obj.type == "CAMERA"]
collections = sorted(collection.name for collection in bpy.data.collections)

def indexed_objects(pattern: str):
    indexed = []
    for obj in objects:
        match = re.search(pattern, obj.name.lower())
        if match:
            indexed.append((int(match.group(1)), obj))
    return indexed


projector_indexed = indexed_objects(r"^projector_(?:p)?(\d+)(?:\b|_)")
frustum_indexed = indexed_objects(r"^frustum_(?:p)?(\d+)(?:\b|_)")
sector_indexed = indexed_objects(r"^sector_(?:s)?(\d+)(?:\b|\s|__)")
projector_ids = sorted({identifier for identifier, _ in projector_indexed})
frustum_ids = sorted({identifier for identifier, _ in frustum_indexed})
sector_ids = sorted({identifier for identifier, _ in sector_indexed})
projectors = [obj for _, obj in projector_indexed]
frustums = [obj for _, obj in frustum_indexed]
sectors = [obj for _, obj in sector_indexed]
surfaces = [obj for obj in meshes if any(term in obj.name.lower() for term in ("surface", "screen"))]
surface_dimensions = [list(map(float, obj.dimensions)) for obj in surfaces]
approx_surface = any(
    5.5 <= max(dimensions) <= 6.5
    and any(2.5 <= value <= 3.5 for value in dimensions)
    for dimensions in surface_dimensions
)
initial_renders = sorted(str(file_path.relative_to(root)) for file_path in root.glob("renders/initial/*.png"))
final_renders = sorted(str(file_path.relative_to(root)) for file_path in root.glob("renders/final/*.png"))
checks = {
    "surfaceApproximately6x3m": approx_surface,
    "exactlyProjectorIds1To3": projector_ids == [1, 2, 3],
    "exactlyFrustumIds1To3": frustum_ids == [1, 2, 3],
    "exactlySectorIds1To6": sector_ids == [1, 2, 3, 4, 5, 6],
    "cameraConfigured": len(cameras) >= 1 and bpy.context.scene.camera is not None,
    "organizedCollections": len(collections) >= 4,
    "legibleMaterials": len(bpy.data.materials) >= 5,
    "threeInitialRenders": len(initial_renders) >= 3,
    "threeFinalRenders": len(final_renders) >= 3,
}
result = {
    "blender": bpy.app.version_string,
    "candidatePath": str(candidate),
    "candidateSha256": sha256_file(candidate),
    "objects": len(objects),
    "meshes": len(meshes),
    "materials": len(bpy.data.materials),
    "cameras": len(cameras),
    "collections": collections,
    "projectorIds": projector_ids,
    "frustumIds": frustum_ids,
    "sectorIds": sector_ids,
    "projectorNames": sorted(obj.name for obj in projectors),
    "frustumNames": sorted(obj.name for obj in frustums),
    "sectorNames": sorted(obj.name for obj in sectors),
    "surfaceNames": sorted(obj.name for obj in surfaces),
    "surfaceDimensions": surface_dimensions,
    "initialRenders": initial_renders,
    "finalRenders": final_renders,
    "checks": checks,
    "passed": all(checks.values()),
}
output = root / "evidence" / "technical-validation.json"
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps(result, sort_keys=True))
