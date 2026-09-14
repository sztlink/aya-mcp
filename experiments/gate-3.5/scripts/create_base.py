#!/usr/bin/env python3
"""Create the identical synthetic source scene for both benchmark arms."""

import os
from pathlib import Path

import bpy

output = Path(os.environ["GATE35_BASE_BLEND"]).resolve()
output.parent.mkdir(parents=True, exist_ok=True)
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)
for collection in list(bpy.data.collections):
    if collection.name != "Collection":
        bpy.data.collections.remove(collection)
base = bpy.data.collections.get("Collection")
base.name = "BASE_ROOM"


def cube(name, location, scale):
    bpy.ops.mesh.primitive_cube_add(location=location)
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return obj


floor = cube("Room_Floor", (0, 0, -0.05), (6.0, 5.0, 0.05))
wall = cube("Room_Back_Wall", (0, 4.95, 2.0), (6.0, 0.05, 2.0))
for obj in (floor, wall):
    obj["source_fixture"] = True

bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0, 0, 1.5))
reference = bpy.context.object
reference.name = "Installation_Origin"
reference["brief_surface_width_m"] = 6.0
reference["brief_surface_height_m"] = 3.0
reference["brief_projectors"] = 3
reference["brief_sectors"] = 6

scene = bpy.context.scene
scene["gate35_fixture"] = "aya-mcp-blender-value-proof-v1"
scene["brief"] = (
    "Build a legible 6x3m projection installation with three projectors, "
    "frustums, six sectors, cameras, materials and visual evidence."
)
scene.unit_settings.system = "METRIC"
scene.unit_settings.length_unit = "METERS"
bpy.ops.wm.save_as_mainfile(filepath=str(output), check_existing=False)
print(f"saved {output}")
