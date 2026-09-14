#!/usr/bin/env node

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const nativeVersion = '2026-07-28';
const legacyVersion = '2025-11-25';
const clientInfo = { name: 'aya-mcp-smoke', version: '0.0.0' };
const servers = [
  {
    role: 'public',
    binary: path.join(root, 'target/debug/aya-public-mcp'),
    tools: ['aya_public_status', 'aya_score_validate'],
    statusTool: 'aya_public_status',
  },
  {
    role: 'worker',
    binary: path.join(root, 'target/debug/aya-worker-mcp'),
    tools: ['aya_score_read', 'aya_worker_status'],
    statusTool: 'aya_worker_status',
  },
];

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(() => resolve(false), milliseconds));

function openClient(server) {
  const child = spawn(server.binary, [], { stdio: ['pipe', 'pipe', 'pipe'] });
  const lines = createInterface({ input: child.stdout });
  const pending = new Map();
  let stderr = '';
  let nextId = 1;
  let exited = false;

  const failPending = (error) => {
    for (const { reject, timeout } of pending.values()) {
      clearTimeout(timeout);
      reject(error);
    }
    pending.clear();
  };

  const exitPromise = new Promise((resolve) => {
    child.once('exit', (code, signal) => {
      exited = true;
      failPending(
        new Error(
          `${server.role} exited before replying: code=${code} signal=${signal}; stderr: ${stderr}`,
        ),
      );
      resolve(true);
    });
  });

  child.once('error', (error) => {
    failPending(new Error(`${server.role} failed to start: ${error.message}`));
  });

  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });

  lines.on('line', (line) => {
    try {
      const message = JSON.parse(line);
      const settle = pending.get(message.id);
      if (settle) {
        pending.delete(message.id);
        clearTimeout(settle.timeout);
        if (message.jsonrpc !== '2.0') {
          settle.reject(new Error(`${server.role} emitted a non-JSON-RPC 2.0 response`));
        } else {
          settle.resolve(message);
        }
      }
    } catch (error) {
      failPending(new Error(`${server.role} emitted malformed JSON: ${error.message}`));
    }
  });

  return {
    request(method, params = {}) {
      return new Promise((resolve, reject) => {
        const id = nextId++;
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`${server.role} ${method} timed out; stderr: ${stderr}`));
        }, 5_000);
        pending.set(id, { reject, resolve, timeout });
        child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      });
    },
    notify(method, params) {
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    },
    async close() {
      if (exited) return;
      child.kill('SIGTERM');
      if (await Promise.race([exitPromise, delay(2_000)])) return;
      child.kill('SIGKILL');
      if (!(await Promise.race([exitPromise, delay(2_000)]))) {
        throw new Error(`${server.role} did not exit after SIGKILL`);
      }
    },
  };
}

async function assertSurface(client, server, meta = {}) {
  const listed = await client.request('tools/list', meta);
  assert.equal(listed.error, undefined);
  assert.deepEqual(
    listed.result.tools.map(({ name }) => name).sort(),
    server.tools,
  );

  const status = await client.request('tools/call', {
    name: server.statusTool,
    arguments: {},
    ...meta,
  });
  assert.equal(status.error, undefined);
  assert.equal(status.result.isError, false);
  assert.equal(status.result.structuredContent.isolation_level, 'contract_only');
  if (server.role === 'worker') {
    assert.equal(status.result.structuredContent.execution_enabled, false);
  } else {
    assert.equal(status.result.structuredContent.runtime_available, false);
  }
}

async function probeNative(server) {
  const client = openClient(server);
  const requestMeta = {
    _meta: {
      'io.modelcontextprotocol/protocolVersion': nativeVersion,
      'io.modelcontextprotocol/clientInfo': clientInfo,
      'io.modelcontextprotocol/clientCapabilities': {},
    },
  };

  try {
    const discovered = await client.request('server/discover', requestMeta);
    assert.equal(discovered.error, undefined);
    assert.equal(discovered.result.resultType, 'complete');
    assert(discovered.result.supportedVersions.includes(nativeVersion));
    await assertSurface(client, server, requestMeta);
    return `${server.role} ${nativeVersion} native discover: ${server.tools.join(', ')}`;
  } finally {
    await client.close();
  }
}

async function probeLegacy(server) {
  const client = openClient(server);

  try {
    const initialized = await client.request('initialize', {
      protocolVersion: legacyVersion,
      capabilities: {},
      clientInfo,
    });
    assert.equal(initialized.error, undefined);
    assert.equal(initialized.result.protocolVersion, legacyVersion);
    client.notify('notifications/initialized');
    await assertSurface(client, server);
    return `${server.role} ${legacyVersion} legacy initialize: ${server.tools.join(', ')}`;
  } finally {
    await client.close();
  }
}

for (const server of servers) {
  console.log(await probeNative(server));
  console.log(await probeLegacy(server));
}
