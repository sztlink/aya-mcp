#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const publicBinary = path.join(root, 'target/debug/aya-public-mcp');
const score = JSON.parse(
  await readFile(path.join(root, 'spec/v0/fixtures/valid/synthetic-workcell-score.json')),
);
const protocolMeta = {
  _meta: {
    'io.modelcontextprotocol/protocolVersion': '2026-07-28',
    'io.modelcontextprotocol/clientInfo': { name: 'aya-e2e', version: '0.0.0' },
    'io.modelcontextprotocol/clientCapabilities': {},
  },
};
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

function startPublic(base, scenario) {
  const child = spawn(publicBinary, [], {
    env: {
      ...process.env,
      AYA_WORKCELL_BASE: base,
      AYA_SYNTHETIC_SCENARIO: scenario,
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  child.stdout.setEncoding('utf8');
  let buffer = '';
  let stderr = '';
  let nextId = 1;
  const pending = new Map();
  let exited = false;
  const exitPromise = new Promise((resolve) => {
    child.once('exit', (code, signal) => {
      exited = true;
      for (const { reject, timer } of pending.values()) {
        clearTimeout(timer);
        reject(new Error(`Public MCP exited: code=${code} signal=${signal}; ${stderr}`));
      }
      pending.clear();
      resolve();
    });
  });
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });
  child.stdout.on('data', (chunk) => {
    buffer += chunk;
    for (;;) {
      const newline = buffer.indexOf('\n');
      if (newline < 0) break;
      const line = buffer.slice(0, newline);
      buffer = buffer.slice(newline + 1);
      const message = JSON.parse(line);
      const waiter = pending.get(message.id);
      if (waiter) {
        pending.delete(message.id);
        clearTimeout(waiter.timer);
        waiter.resolve(message);
      }
    }
  });

  return {
    request(method, params = {}) {
      return new Promise((resolve, reject) => {
        const id = nextId++;
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`Public MCP ${method} timed out; ${stderr}`));
        }, 25_000);
        pending.set(id, { reject, resolve, timer });
        child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      });
    },
    async call(name, args) {
      const response = await this.request('tools/call', {
        name,
        arguments: args,
        ...protocolMeta,
      });
      assert.equal(response.error, undefined, JSON.stringify(response.error));
      assert.equal(response.result.isError, false);
      return response.result.structuredContent;
    },
    async close() {
      if (exited) return;
      child.stdin.end();
      await Promise.race([exitPromise, sleep(2_000)]);
      if (!exited) child.kill('SIGKILL');
      await exitPromise;
    },
  };
}

async function waitForTerminal(client, workcellId) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const status = await client.call('aya_workcell_status', { workcell_id: workcellId });
    if (!['running', 'cancelling', 'finalizing', 'finalizing_cancel'].includes(status.status)) {
      return status;
    }
    await sleep(50);
  }
  throw new Error(`workcell ${workcellId} did not reach a terminal status`);
}

async function successProof(base) {
  const client = startPublic(base, 'success');
  try {
    const discovered = await client.request('server/discover', protocolMeta);
    assert(discovered.result.supportedVersions.includes('2026-07-28'));
    const request = await client.call('aya_workcell_request', { score });
    assert.equal(request.accepted, true);
    assert.equal(request.isolation_level, 'contract_only');
    const status = await waitForTerminal(client, request.workcell_id);
    assert.equal(status.status, 'candidate_ready');
    assert.equal(status.final_lifecycle_state, 'expired');
    assert.equal(status.process_tree_reaped, true);
    const review = await client.call('aya_candidate_review', {
      workcell_id: request.workcell_id,
    });
    assert.equal(review.ready, true);
    assert.equal(review.promotion_available, false);
    assert.equal(review.receipt.status, 'candidate_ready');

    const workcell = path.join(base, request.workcell_id);
    const source = await readFile(path.join(workcell, 'input/source.scene.json'));
    const sourceDigest = createHash('sha256').update(source).digest('hex');
    assert.equal(sourceDigest, score.inputs[0].sha256);
    const candidate = JSON.parse(
      await readFile(path.join(workcell, 'output/candidate.scene.json')),
    );
    assert.deepEqual(candidate, { camera: 'locked', scale: 1, tension: 3 });
    assert.deepEqual(
      JSON.parse(await readFile(path.join(workcell, 'output/receipt.json'))),
      review.receipt,
    );
    return request.workcell_id;
  } finally {
    await client.close();
  }
}

async function shutdownProof(base) {
  const client = startPublic(base, 'timeout');
  let workcellId;
  try {
    await client.request('server/discover', protocolMeta);
    const request = await client.call('aya_workcell_request', { score });
    assert.equal(request.accepted, true);
    workcellId = request.workcell_id;
  } finally {
    await client.close();
  }
  const workcell = path.join(base, workcellId);
  const receipt = JSON.parse(await readFile(path.join(workcell, 'output/receipt.json')));
  assert.equal(receipt.status, 'cancelled');
  assert.equal(receipt.candidate, null);
  await assert.rejects(readFile(path.join(workcell, 'output/candidate.scene.json')));
  return workcellId;
}

async function cancellationProof(base) {
  const client = startPublic(base, 'timeout');
  try {
    await client.request('server/discover', protocolMeta);
    const request = await client.call('aya_workcell_request', { score });
    assert.equal(request.accepted, true);
    const cancelled = await client.call('aya_workcell_cancel', {
      workcell_id: request.workcell_id,
    });
    assert.equal(cancelled.found, true);
    assert.equal(cancelled.cancellation_requested, true);
    const status = await waitForTerminal(client, request.workcell_id);
    assert.equal(status.status, 'cancelled');
    assert.equal(status.final_lifecycle_state, 'expired');
    assert.equal(status.process_tree_reaped, true);
    assert.deepEqual(status.errors, []);
    const review = await client.call('aya_candidate_review', {
      workcell_id: request.workcell_id,
    });
    assert.equal(review.ready, true);
    assert.equal(review.receipt.status, 'cancelled');
    assert.equal(review.receipt.candidate, null);
    assert(
      review.receipt.events.some(({ summary }) => summary.includes('workcell cancelled')),
      'Worker cancellation event is missing',
    );
    assert(
      review.receipt.events.some(({ summary }) =>
        summary.includes('Cancellation accepted before Public publication'),
      ),
      'Public cancellation event is missing',
    );
    const workcell = path.join(base, request.workcell_id);
    await assert.rejects(readFile(path.join(workcell, 'output/candidate.scene.json')));
    return request.workcell_id;
  } finally {
    await client.close();
  }
}

const base = await mkdtemp(path.join(tmpdir(), 'aya-mcp-e2e-'));
try {
  const successId = await successProof(base);
  const cancelledId = await cancellationProof(base);
  const shutdownId = await shutdownProof(base);
  console.log(`success: ${successId}`);
  console.log(`cancelled: ${cancelledId}`);
  console.log(`shutdown-cancelled: ${shutdownId}`);
  console.log('Score -> Public MCP -> Worker MCP -> fake DCC -> Receipt -> expiry: ok');
} finally {
  await rm(base, { recursive: true, force: true });
}
