#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

import { sha256Json } from '../../../src/canonical.mjs';
import { appendReceiptEvent, sealReceipt } from '../../../src/receipt.mjs';
import { validateDocument, validateReceiptAgainstContext } from '../../../src/validate.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const evidenceRoot = path.resolve(process.argv[2] ?? '/home/aya/tmp/aya-mcp-gate35-evidence');
const resultRoot = path.join(evidenceRoot, 'results');
const receiptRoot = path.join(resultRoot, 'arm-b-receipt');

async function sha256File(filePath) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest('hex');
}

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, 'utf8'));
}

async function readJsonLines(filePath) {
  const entries = [];
  const lines = readline.createInterface({ input: createReadStream(filePath), crlfDelay: Infinity });
  for await (const line of lines) {
    if (line.trim()) entries.push(JSON.parse(line));
  }
  return entries;
}

async function copyInto(source, relativeDestination) {
  const destination = path.join(receiptRoot, relativeDestination);
  await mkdir(path.dirname(destination), { recursive: true });
  await copyFile(source, destination);
  return destination;
}

async function summarizeArm(arm) {
  const agentRoot = path.join(evidenceRoot, 'agent', arm);
  const remoteRoot = path.join(evidenceRoot, 'remote', arm);
  const agentEvents = await readJsonLines(path.join(agentRoot, 'agent.jsonl'));
  const bridgeEvents = await readJsonLines(path.join(remoteRoot, 'logs', 'bridge-mcp.jsonl'));
  const workerEvents = arm === 'B'
    ? await readJsonLines(path.join(remoteRoot, 'logs', 'aya-worker.jsonl'))
    : [];

  const usage = {
    modelCalls: 0,
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    reasoningTokens: 0,
    totalTokensIncludingCache: 0,
    reportedCostUsd: 0
  };
  const agentTools = {};
  let agentToolFailures = 0;
  let model = null;
  let provider = null;

  for (const event of agentEvents) {
    if (event.type === 'message_end' && event.message?.role === 'assistant' && event.message.usage) {
      const current = event.message.usage;
      usage.modelCalls += 1;
      usage.inputTokens += current.input ?? 0;
      usage.outputTokens += current.output ?? 0;
      usage.cacheReadTokens += current.cacheRead ?? 0;
      usage.cacheWriteTokens += current.cacheWrite ?? 0;
      usage.reasoningTokens += current.reasoning ?? 0;
      usage.totalTokensIncludingCache += current.totalTokens ?? 0;
      usage.reportedCostUsd += current.cost?.total ?? 0;
      model ??= event.message.model ?? null;
      provider ??= event.message.provider ?? null;
    }
    if (event.type === 'tool_execution_start') {
      const name = event.toolName ?? 'unknown';
      agentTools[name] = (agentTools[name] ?? 0) + 1;
    }
    if (event.type === 'tool_execution_end' && event.isError === true) agentToolFailures += 1;
  }

  const initialDirectory = path.join(remoteRoot, 'renders', 'initial');
  const finalDirectory = path.join(remoteRoot, 'renders', 'final');
  const initialRenders = (await readdir(initialDirectory)).filter((name) => name.endsWith('.png')).sort();
  const finalRenders = (await readdir(finalDirectory)).filter((name) => name.endsWith('.png')).sort();
  const validation = await readJson(path.join(remoteRoot, 'evidence', 'technical-validation.json'));
  const sourceSha256 = await sha256File(path.join(remoteRoot, 'input', 'source.blend'));
  const initialHashLines = (await readFile(path.join(remoteRoot, 'logs', 'initial-sha256.txt'), 'utf8')).trim().split('\n');
  const initialSourceSha256 = initialHashLines[0].trim().split(/\s+/)[0];
  const initialCandidateSha256 = initialHashLines[1].trim().split(/\s+/)[0];
  const candidateSha256 = await sha256File(path.join(remoteRoot, 'work', 'candidate.blend'));
  const agentExitCode = Number((await readFile(path.join(agentRoot, 'exit-code.txt'), 'utf8')).trim());

  const gatewayTools = {};
  const helperTools = {};
  for (const event of bridgeEvents) {
    gatewayTools[event.gatewayTool] = (gatewayTools[event.gatewayTool] ?? 0) + 1;
    if (event.helperTool) helperTools[event.helperTool] = (helperTools[event.helperTool] ?? 0) + 1;
  }

  return {
    arm,
    path: arm === 'A' ? 'agent -> bridge -> Blender' : 'agent -> AYA Worker -> same bridge -> Blender',
    provider,
    model,
    thinking: 'xhigh',
    wallTimeSeconds: Number((Number(await readFile(path.join(agentRoot, 'wall-ms.txt'), 'utf8')) / 1000).toFixed(3)),
    humanSetupSeconds: 0,
    humanInterventionsDuringRun: 0,
    agentTools: {
      calls: Object.values(agentTools).reduce((sum, count) => sum + count, 0),
      byName: agentTools,
      failedCalls: agentToolFailures
    },
    modelUsage: { ...usage, reportedCostUsd: Number(usage.reportedCostUsd.toFixed(7)) },
    bridge: {
      calls: bridgeEvents.length,
      successfulCalls: bridgeEvents.filter((event) => event.ok).length,
      failedCalls: bridgeEvents.filter((event) => !event.ok).length,
      durationSeconds: Number(bridgeEvents.reduce((sum, event) => sum + event.durationSeconds, 0).toFixed(6)),
      gatewayTools,
      helperTools,
      rawPythonBpyExecutions: bridgeEvents.filter((event) => event.gatewayTool === 'invoke_blender_tool' && event.helperTool === 'draft_script').length
    },
    ayaWorker: arm === 'B' ? {
      calls: workerEvents.length,
      successfulCalls: workerEvents.filter((event) => event.bridgeExitCode === 0).length,
      durationSeconds: Number(workerEvents.reduce((sum, event) => sum + event.durationSeconds, 0).toFixed(6)),
      incrementalDurationOverBridgeSeconds: Number((workerEvents.reduce((sum, event) => sum + event.durationSeconds, 0) - bridgeEvents.reduce((sum, event) => sum + event.durationSeconds, 0)).toFixed(6)),
      durationScope: 'partial lower bound; original timer excluded Python startup and the first source hash',
      sourcePreservedEveryCall: workerEvents.every((event) => event.sourcePreserved === true),
      processLaunches: new Set(workerEvents.map((event) => event.pid)).size
    } : null,
    inspectCorrectCycles: initialRenders.length >= 3 && finalRenders.length >= 3 ? 1 : 0,
    retriesOrRecoveries: agentToolFailures,
    crashes: agentExitCode === 0 ? 0 : 1,
    agentExitCode,
    renders: { initial: initialRenders, final: finalRenders, total: initialRenders.length + finalRenders.length },
    custody: {
      sourceSha256,
      initialSourceSha256,
      initialCandidateSha256,
      sourcePreserved: sourceSha256 === initialSourceSha256,
      candidateSha256,
      candidateStartedAsSourceCopy: initialCandidateSha256 === initialSourceSha256,
      candidateChangedFromInitialCopy: candidateSha256 !== initialCandidateSha256
    },
    technicalValidation: {
      ok: validation.passed === true && validation.candidateSha256 === candidateSha256,
      validatedCandidateSha256: validation.candidateSha256,
      checks: validation.checks,
      counts: {
        objects: validation.objects,
        meshes: validation.meshes,
        materials: validation.materials,
        cameras: validation.cameras
      }
    }
  };
}

async function createArmBReceipt(metrics) {
  const source = await copyInto(path.join(evidenceRoot, 'remote/B/input/source.blend'), 'input/source.blend');
  const candidate = await copyInto(path.join(evidenceRoot, 'remote/B/work/candidate.blend'), 'output/candidate.blend');
  const validation = await copyInto(path.join(evidenceRoot, 'remote/B/evidence/technical-validation.json'), 'output/evidence/technical-validation.json');
  const workerLog = await copyInto(path.join(evidenceRoot, 'remote/B/logs/aya-worker.jsonl'), 'output/evidence/aya-worker.jsonl');
  const bridgeLog = await copyInto(path.join(evidenceRoot, 'remote/B/logs/bridge-mcp.jsonl'), 'output/evidence/bridge-mcp.jsonl');
  const initialNames = metrics.renders.initial;
  const finalNames = metrics.renders.final;

  for (const name of initialNames) {
    await copyInto(path.join(evidenceRoot, 'remote/B/renders/initial', name), `output/renders/initial/${name}`);
  }
  for (const name of finalNames) {
    await copyInto(path.join(evidenceRoot, 'remote/B/renders/final', name), `output/renders/final/${name}`);
  }
  await mkdir(path.join(receiptRoot, 'work'), { recursive: true });

  const sourceDigest = await sha256File(source);
  const initialSourceDigest = metrics.custody.initialSourceSha256;
  const candidateDigest = await sha256File(candidate);
  const workerEvents = await readJsonLines(workerLog);
  const bridgeEvents = await readJsonLines(bridgeLog);
  const startedAt = (await readFile(path.join(evidenceRoot, 'agent/B/started-at.txt'), 'utf8')).trim().replace(',', '.');
  const finishedAt = (await readFile(path.join(evidenceRoot, 'agent/B/finished-at.txt'), 'utf8')).trim().replace(',', '.');
  const leaseExpiry = new Date(Date.parse(startedAt) + 1200 * 1000).toISOString();
  const sealedAt = new Date().toISOString();
  const draftExecutions = bridgeEvents.filter((event) => event.gatewayTool === 'invoke_blender_tool' && event.helperTool === 'draft_script');
  if (draftExecutions.length < 3) throw new Error('Arm B evidence does not contain at least three bpy executions');
  const candidateHashPath = path.join(receiptRoot, 'output/evidence/candidate.sha256.txt');
  await writeFile(candidateHashPath, `${candidateDigest}  candidate.blend\n`);

  const score = {
    schema: 'aya.score/v0',
    scoreId: 'gate35-real-blender-arm-b',
    origin: {
      author: 'Felipe Sztutman',
      gesture: 'Testar empiricamente se o AYA MCP agrega valor ao delegar trabalho real e autônomo no Blender.'
    },
    intent: 'Criar um candidato técnico Blender sintético a partir de uma fonte preservada, com superfície 6 x 3 m, seis setores, três projetores, três frustums, dimensões, legenda e três vistas úteis.',
    inputs: [{ path: 'input/source.blend', sha256: initialSourceDigest }],
    invariants: [
      'Não sobrescrever a fonte.',
      'Manter exatamente três projetores físicos rotulados.',
      'Manter exatamente três frustums visíveis e coloridos.',
      'Produzir três renders antes e três depois de uma revisão visual.'
    ],
    variationSpace: [
      'Composição espacial e acabamento técnico.',
      'Materiais, iluminação, tipografia e enquadramentos.',
      'Geometria dos projetores e visualização dos frustums.'
    ],
    evidenceRequired: ['before_image', 'after_image', 'scene_summary', 'artifact_hash'],
    budget: { wallTimeSeconds: 1200, maxAttempts: 20 },
    isolationRequired: 'contract_only'
  };
  const scoreSha256 = sha256Json(score);
  const lease = {
    schema: 'aya.lease/v0',
    leaseId: 'gate35-real-blender-arm-b',
    scoreSha256,
    workerId: 'gate35-4090-worker',
    createdAt: new Date(startedAt).toISOString(),
    expiresAt: leaseExpiry,
    isolationRequired: 'contract_only',
    network: 'loopback',
    allowRawCode: true,
    mounts: { input: 'input', work: 'work', output: 'output' }
  };

  let events = [];
  for (const event of bridgeEvents) {
    const invoked = event.gatewayTool === 'invoke_blender_tool';
    const helper = event.helperTool ? `/${event.helperTool}` : '';
    events = appendReceiptEvent(events, {
      at: event.at,
      kind: invoked ? 'execute' : 'inspect',
      summary: `${event.gatewayTool}${helper} completed with ok=${event.ok}.`
    });
  }
  events = appendReceiptEvent(events, { at: new Date(finishedAt).toISOString(), kind: 'capture', summary: `${initialNames.length} initial and ${finalNames.length} final PNG renders were preserved.` });
  events = appendReceiptEvent(events, { at: new Date(finishedAt).toISOString(), kind: 'candidate', summary: 'Candidato e evidências prontos para review externo ao Worker.' });

  const artifacts = [
    { path: 'output/candidate.blend', sha256: candidateDigest },
    ...(await Promise.all(finalNames.map(async (name) => ({
      path: `output/renders/final/${name}`,
      sha256: await sha256File(path.join(receiptRoot, 'output/renders/final', name))
    })))),
    { path: 'output/evidence/aya-worker.jsonl', sha256: await sha256File(workerLog) },
    { path: 'output/evidence/bridge-mcp.jsonl', sha256: await sha256File(bridgeLog) }
  ];
  const evidence = [
    ...(await Promise.all(initialNames.map(async (name) => ({
      kind: 'before_image',
      path: `output/renders/initial/${name}`,
      sha256: await sha256File(path.join(receiptRoot, 'output/renders/initial', name))
    })))),
    ...(await Promise.all(finalNames.map(async (name) => ({
      kind: 'after_image',
      path: `output/renders/final/${name}`,
      sha256: await sha256File(path.join(receiptRoot, 'output/renders/final', name))
    })))),
    { kind: 'scene_summary', path: 'output/evidence/technical-validation.json', sha256: await sha256File(validation) },
    { kind: 'artifact_hash', path: 'output/evidence/candidate.sha256.txt', sha256: await sha256File(candidateHashPath) }
  ];

  const receipt = sealReceipt({
    schema: 'aya.receipt/v0',
    receiptId: 'gate35-real-blender-arm-b',
    scoreSha256,
    leaseSha256: sha256Json(lease),
    status: 'candidate_ready',
    events,
    candidate: {
      sources: [{ path: 'input/source.blend', beforeSha256: initialSourceDigest, afterSha256: sourceDigest }],
      artifacts,
      evidence
    },
    sealedAt
  });

  const contextResult = validateReceiptAgainstContext(score, lease, receipt);
  const individualResults = {
    score: validateDocument(score),
    lease: validateDocument(lease),
    receipt: validateDocument(receipt)
  };
  const fileErrors = [];
  for (const item of [...receipt.candidate.artifacts, ...receipt.candidate.evidence]) {
    const actual = await sha256File(path.join(receiptRoot, item.path));
    if (actual !== item.sha256) fileErrors.push(`${item.path}: hash mismatch`);
  }
  if (await sha256File(source) !== receipt.candidate.sources[0].afterSha256) {
    fileErrors.push('input/source.blend: final source hash mismatch');
  }
  if (metrics.custody.initialSourceSha256 !== receipt.candidate.sources[0].beforeSha256) {
    fileErrors.push('input/source.blend: recorded initial source hash mismatch');
  }
  if (!workerEvents.every((event) => event.sourceBeforeSha256 === initialSourceDigest && event.sourceAfterSha256 === initialSourceDigest && event.sourcePreserved === true)) {
    fileErrors.push('output/evidence/aya-worker.jsonl: source custody sequence mismatch');
  }
  const technicalValidation = await readJson(validation);
  if (technicalValidation.candidateSha256 !== candidateDigest || technicalValidation.passed !== true) {
    fileErrors.push('output/evidence/technical-validation.json: sealed candidate binding failed');
  }

  await writeFile(path.join(receiptRoot, 'score.json'), `${JSON.stringify(score, null, 2)}\n`);
  await writeFile(path.join(receiptRoot, 'lease.json'), `${JSON.stringify(lease, null, 2)}\n`);
  await writeFile(path.join(receiptRoot, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  const validationResult = {
    ok: contextResult.ok && fileErrors.length === 0 && Object.values(individualResults).every((result) => result.ok),
    contextualValidation: contextResult,
    documentValidation: individualResults,
    fileErrors,
    note: 'Receipt contextual selado pós-execução a partir dos logs e artefatos preservados do Worker experimental.'
  };
  await writeFile(path.join(receiptRoot, 'receipt-validation.json'), `${JSON.stringify(validationResult, null, 2)}\n`);
  if (!validationResult.ok) throw new Error(JSON.stringify(validationResult, null, 2));
  return validationResult;
}

if (!existsSync(path.join(evidenceRoot, 'remote/A'))) {
  throw new Error(`Evidence root is incomplete: ${evidenceRoot}`);
}

await mkdir(resultRoot, { recursive: true });
const armA = await summarizeArm('A');
const armB = await summarizeArm('B');
const metrics = {
  schema: 'aya.gate35.metrics/v0',
  generatedAt: new Date().toISOString(),
  environment: {
    host: '4090 Render Server',
    blender: '5.1.2',
    bridge: 'CallMeJones Blender Agent Bridge 0.5.6',
    bridgeSourceModified: false,
    model: 'openai-codex/gpt-5.6-terra',
    thinking: 'xhigh',
    sourceSha256: armA.custody.sourceSha256,
    syntheticOnly: true,
    isolationClaim: 'contract_only',
    sharedApparatusSetup: {
      observedWallSeconds: 521.586,
      interval: 'temporary root creation to Arm A agent start',
      humanAttentionSeconds: null,
      note: 'Human attention was not separately instrumented; this is a protocol limitation.'
    }
  },
  measurementProvenance: {
    wallTime: 'agent/{arm}/wall-ms.txt, measured around the non-interactive Pi process',
    tokensAndModel: 'assistant message_end events in agent/{arm}/agent.jsonl',
    bridgeCalls: 'remote/{arm}/logs/bridge-mcp.jsonl',
    workerCustody: 'remote/B/logs/aya-worker.jsonl plus initial and final source rehash',
    technicalValidation: 'candidate reopened from disk by Blender and bound by candidate SHA-256',
    rendersAndCycle: 'three initial and three final PNGs plus the agent final account',
    interventionAndThinking: 'operator observation and frozen invocation record; not independently derivable from MCP logs',
    bridgeSourceModified: 'release ZIP installed without patching; recorded by operator'
  },
  arms: { A: armA, B: armB },
  deltasBMinusA: {
    wallTimeSeconds: Number((armB.wallTimeSeconds - armA.wallTimeSeconds).toFixed(3)),
    bridgeCalls: armB.bridge.calls - armA.bridge.calls,
    agentToolCalls: armB.agentTools.calls - armA.agentTools.calls,
    inputTokens: armB.modelUsage.inputTokens - armA.modelUsage.inputTokens,
    outputTokens: armB.modelUsage.outputTokens - armA.modelUsage.outputTokens,
    totalTokensIncludingCache: armB.modelUsage.totalTokensIncludingCache - armA.modelUsage.totalTokensIncludingCache,
    humanInterventions: armB.humanInterventionsDuringRun - armA.humanInterventionsDuringRun
  }
};
await writeFile(path.join(resultRoot, 'metrics.json'), `${JSON.stringify(metrics, null, 2)}\n`);
const receiptValidation = await createArmBReceipt(armB);
console.log(JSON.stringify({ metrics: path.join(resultRoot, 'metrics.json'), receiptRoot, receiptValidation }, null, 2));
