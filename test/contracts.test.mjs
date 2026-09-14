import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { canonicalize, sha256Json } from '../src/canonical.mjs';
import { appendReceiptEvent, hashReceipt, sealReceipt } from '../src/receipt.mjs';
import { compiledSchemaNames } from '../src/schema.mjs';
import { isSafeRelativePath, validateAdmission, validateDocument, validateReceiptAgainstContext } from '../src/validate.mjs';

const fixture = async (path) => JSON.parse(await readFile(new URL(`../spec/v0/fixtures/${path}`, import.meta.url), 'utf8'));

test('all v0 JSON Schemas compile', () => {
  assert.deepEqual(compiledSchemaNames().sort(), [
    'aya.capability-report/v0',
    'aya.lease/v0',
    'aya.receipt/v0',
    'aya.score/v0'
  ]);
});

test('valid v0 fixtures pass schema and semantic validation', async () => {
  for (const path of ['valid/score.json', 'valid/lease.json', 'valid/capability-report.json', 'valid/receipt.json']) {
    const result = validateDocument(await fixture(path));
    assert.deepEqual(result, { ok: true, errors: [] }, path);
  }
});

test('RFC 8785 canonicalization is stable for ordering, Unicode and negative zero', () => {
  assert.equal(canonicalize({ b: -0, a: 'é' }), '{"a":"é","b":0}');
  assert.equal(sha256Json({ b: 2, a: 1 }), sha256Json({ a: 1, b: 2 }));
});

test('canonical paths reject traversal, absolute and Windows paths', () => {
  assert.equal(isSafeRelativePath('input/scene.blend'), true);
  for (const path of ['../scene.blend', '/scene.blend', 'C:/scene.blend', 'input\\scene.blend', 'input//scene.blend', './scene.blend', 'input/./scene.blend']) {
    assert.equal(isSafeRelativePath(path), false, path);
  }
});

test('invalid traversal fixture fails JSON Schema validation', async () => {
  const result = validateDocument(await fixture('invalid/score-path-traversal.json'));
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /must match pattern/);
});

test('lease mounts must be non-overlapping', async () => {
  const lease = await fixture('valid/lease.json');
  lease.mounts.work = 'input/work';
  const result = validateDocument(lease);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /must not overlap/);
});

test('admission binds score digest and forbids isolation downgrade', async () => {
  const score = await fixture('valid/score.json');
  const lease = await fixture('valid/lease.json');
  const capability = {
    ...(await fixture('valid/capability-report.json')),
    isolationLevel: 'os_enforced',
    sourceReadOnly: true,
    outputIsolated: true,
    processTreeReaped: true
  };

  assert.equal(validateAdmission(score, lease, capability).ok, true);

  lease.isolationRequired = 'contract_only';
  const downgraded = validateAdmission(score, lease, capability);
  assert.equal(downgraded.ok, false);
  assert.match(downgraded.errors.join('\n'), /downgrades score requirement/);

  lease.scoreSha256 = '9'.repeat(64);
  const rebound = validateAdmission(score, lease, capability);
  assert.match(rebound.errors.join('\n'), /does not match the admitted score/);
});

test('receipt binds sources and all evidence required by the score', async () => {
  const score = await fixture('valid/score.json');
  const lease = await fixture('valid/lease.json');
  const receipt = await fixture('valid/receipt.json');

  assert.equal(validateReceiptAgainstContext(score, lease, receipt).ok, true);

  const incomplete = structuredClone(receipt);
  incomplete.candidate.evidence = incomplete.candidate.evidence.filter((item) => item.kind !== 'after_image');
  incomplete.receiptSha256 = hashReceipt(incomplete);
  const evidenceResult = validateReceiptAgainstContext(score, lease, incomplete);
  assert.match(evidenceResult.errors.join('\n'), /missing required evidence kind after_image/);

  const mutatedSource = structuredClone(receipt);
  mutatedSource.candidate.sources[0].afterSha256 = '8'.repeat(64);
  mutatedSource.receiptSha256 = hashReceipt(mutatedSource);
  const sourceResult = validateReceiptAgainstContext(score, lease, mutatedSource);
  assert.match(sourceResult.errors.join('\n'), /changed during the workcell/);
});

test('contextual receipt verification binds the lease to the score', async () => {
  const score = await fixture('valid/score.json');
  const lease = await fixture('valid/lease.json');
  const receipt = await fixture('valid/receipt.json');
  lease.scoreSha256 = '9'.repeat(64);
  receipt.leaseSha256 = sha256Json(lease);
  receipt.receiptSha256 = hashReceipt(receipt);
  const result = validateReceiptAgainstContext(score, lease, receipt);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /lease.scoreSha256 does not match score/);
});

test('duplicate source paths are rejected before source-set comparison', async () => {
  const receipt = await fixture('valid/receipt.json');
  receipt.candidate.sources.push({
    ...receipt.candidate.sources[0],
    afterSha256: '8'.repeat(64)
  });
  receipt.receiptSha256 = hashReceipt(receipt);
  const result = validateDocument(receipt);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /duplicate path/);
});

test('score inputs and receipt sources must be below the lease input mount', async () => {
  const score = await fixture('valid/score.json');
  const lease = await fixture('valid/lease.json');
  const receipt = await fixture('valid/receipt.json');

  score.inputs[0].path = 'work/synthetic-scene.blend';
  lease.scoreSha256 = sha256Json(score);
  receipt.scoreSha256 = sha256Json(score);
  receipt.leaseSha256 = sha256Json(lease);
  receipt.candidate.sources[0].path = score.inputs[0].path;
  receipt.receiptSha256 = hashReceipt(receipt);

  const result = validateReceiptAgainstContext(score, lease, receipt);
  assert.equal(result.ok, false);
  assert.match(result.errors.join('\n'), /outside input mount/);
});

test('receipt digest covers candidate and event chain', async () => {
  const receipt = await fixture('valid/receipt.json');
  receipt.candidate.artifacts[0].sha256 = '8'.repeat(64);
  const candidateMutation = validateDocument(receipt);
  assert.match(candidateMutation.errors.join('\n'), /receiptSha256 is invalid/);

  const rewritten = await fixture('valid/receipt.json');
  let events = [];
  events = appendReceiptEvent(events, { at: rewritten.events[0].at, kind: 'inspect', summary: 'Rewritten history' });
  events = appendReceiptEvent(events, { at: rewritten.events[1].at, kind: 'candidate', summary: rewritten.events[1].summary });
  rewritten.events = events;
  const eventRewrite = validateDocument(rewritten);
  assert.match(eventRewrite.errors.join('\n'), /receiptSha256 is invalid/);

  const selfConsistentRewrite = sealReceipt(rewritten);
  assert.equal(validateDocument(selfConsistentRewrite).ok, true);
});

test('malformed receipt event returns validation errors instead of throwing', async () => {
  const receipt = await fixture('valid/receipt.json');
  receipt.events[0] = null;
  assert.doesNotThrow(() => validateDocument(receipt));
  assert.equal(validateDocument(receipt).ok, false);
});

test('public and worker tool surfaces are disjoint', async () => {
  const publicSurface = JSON.parse(await readFile(new URL('../spec/v0/tools/public-mcp.json', import.meta.url), 'utf8'));
  const workerSurface = JSON.parse(await readFile(new URL('../spec/v0/tools/worker-mcp.json', import.meta.url), 'utf8'));
  const overlap = publicSurface.tools.filter((tool) => workerSurface.tools.includes(tool));
  assert.deepEqual(overlap, []);
  assert.equal(publicSurface.tools.some((tool) => /execute|call|discover/.test(tool)), false);
  assert.equal(workerSurface.tools.includes('aya_execute'), true);
});
