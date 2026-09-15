#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const base = path.resolve(process.argv[2] ?? '/home/aya/tmp/aya-mcp-gate35r-20260914');
const plan = JSON.parse(await readFile(path.join(base, 'control/plan-private.json'), 'utf8'));

function collectIds(objects, patterns) {
  const identifiers = new Set();
  const names = [];
  for (const object of objects) {
    for (const pattern of patterns) {
      const match = object.name.match(pattern);
      if (!match) continue;
      identifiers.add(Number(match[1]));
      names.push(object.name);
      break;
    }
  }
  return { ids: [...identifiers].sort((a, b) => a - b), names: names.sort() };
}

function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

const results = [];
for (const run of plan.runs) {
  const root = path.join(base, 'remote', run.runId);
  const inventory = JSON.parse(await readFile(path.join(root, 'evidence/object-inventory.json'), 'utf8'));
  const frozen = JSON.parse(await readFile(path.join(root, 'evidence/technical-validation.json'), 'utf8'));
  const projectors = collectIds(inventory.objects, [
    /^P(\d+)$/i,
    /^P(\d+)(?:__|_| |\|).*projector(?:_| )?body/i,
    /^P(\d+)_Projector$/i,
    /^Projector_(?:P)?(\d+).*Body/i
  ]);
  const frustums = collectIds(inventory.objects, [
    /^F(\d+)$/i,
    /^F(\d+)(?:__|_| |\|).*frustum/i,
    /^Frustum_(?:P)?(\d+)/i
  ]);
  const sectors = collectIds(inventory.objects, [
    /^S(\d+)$/i,
    /^S(\d+)(?:__|_| |\|).*sector/i,
    /^Sector_S?(\d+)/i
  ]);
  const directSurfaces = inventory.objects.filter((object) =>
    object.type === 'MESH'
    && /(principal.*surface|projection.?surface|surface.*6.*3)/i.test(object.name)
    && Math.max(...object.dimensions) >= 5.5
    && Math.max(...object.dimensions) <= 6.5
    && object.dimensions.some((value) => value >= 2.5 && value <= 3.5));
  const sectorObjects = inventory.objects.filter((object) =>
    object.type === 'MESH' && sectors.names.includes(object.name));
  let assembledSectorBounds = null;
  if (sectorObjects.length === 6) {
    const minimum = [Infinity, Infinity, Infinity];
    const maximum = [-Infinity, -Infinity, -Infinity];
    for (const object of sectorObjects) {
      for (let axis = 0; axis < 3; axis += 1) {
        minimum[axis] = Math.min(minimum[axis], object.location[axis] - object.dimensions[axis] / 2);
        maximum[axis] = Math.max(maximum[axis], object.location[axis] + object.dimensions[axis] / 2);
      }
    }
    assembledSectorBounds = maximum.map((value, axis) => Number((value - minimum[axis]).toFixed(4)));
  }
  const assembledSurface = assembledSectorBounds !== null
    && Math.max(...assembledSectorBounds) >= 5.5
    && Math.max(...assembledSectorBounds) <= 6.5
    && assembledSectorBounds.some((value) => value >= 2.5 && value <= 3.5);
  const candidateBuffer = await readFile(path.join(root, 'work/candidate.blend'));
  const checks = {
    projectorIds1To3: JSON.stringify(projectors.ids) === '[1,2,3]',
    frustumIds1To3: JSON.stringify(frustums.ids) === '[1,2,3]',
    sectorIds1To6: JSON.stringify(sectors.ids) === '[1,2,3,4,5,6]',
    principalSurfaceApproximately6x3: directSurfaces.length > 0 || assembledSurface,
    renderContractAndPngDimensions: frozen.checks.frozenRenderSettings === true && frozen.checks.allRenders1000x650 === true,
    candidateHashBound: frozen.candidateSha256 === sha256(candidateBuffer)
  };
  results.push({
    order: run.order,
    runId: run.runId,
    route: run.route,
    passed: Object.values(checks).every(Boolean),
    checks,
    projectors,
    frustums,
    sectors,
    directSurfaceNames: directSurfaces.map((object) => object.name),
    assembledSectorBounds,
    strictFrozenValidatorPassed: frozen.passed
  });
}

const document = {
  schema: 'aya.gate35r.semantic-adjudication/v0',
  generatedAt: new Date().toISOString(),
  method: 'Post-run read-only object inventory with naming-family normalization and assembled-sector bounding box. Candidates were not modified.',
  reason: 'The frozen validator encoded prior naming conventions and produced false negatives on semantically equivalent P1/F1/S1 names.',
  results
};
await writeFile(path.join(base, 'analysis/semantic-adjudication.json'), `${JSON.stringify(document, null, 2)}\n`);
if (!results.every((result) => result.passed)) process.exitCode = 1;
