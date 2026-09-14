# Gate 3.5 Blender benchmark briefing

Starting from the provided synthetic Blender scene, build a clear technical visualization of a projection installation.

Requirements:

1. Preserve the original source `.blend`. Work only in the candidate copy already opened in Blender.
2. Build one principal projection surface approximately 6 m wide by 3 m high.
3. Organize the scene into meaningfully named collections.
4. Represent exactly three physical projectors as recognizable projector bodies.
5. Represent a visible projection frustum for each projector.
6. Divide the principal surface into six identifiable technical sectors. The sectors are subdivisions of one surface, not six artistic cameras.
7. Configure at least one useful camera, readable materials, lighting and metric units.
8. Produce at least three initial conference renders from useful viewpoints under `renders/initial/`.
9. Inspect the actual initial PNG renders visually. Diagnose at least one concrete visual or technical weakness.
10. Correct the scene autonomously based on that diagnosis.
11. Produce at least three corrected conference renders under `renders/final/`.
12. Save the completed candidate to the exact candidate `.blend` path supplied by the arm protocol.
13. Finish with a concise account of what you inspected, what you corrected and any manual work still remaining.

The result should be useful as a legible spatial/technical study, not merely satisfy object counts. Do not add client data, external assets or unrelated content.

Budget per arm:

- maximum 25 Blender MCP calls;
- maximum 20 minutes wall time;
- no human intervention during execution;
- use the installed Blender Agent Bridge only for Blender inspection, mutation and rendering;
- broad trusted `bpy`/Python through the bridge is allowed.
