import { sha256Json } from './canonical.mjs';
import { hashReceipt, verifyReceiptChain } from './receipt.mjs';
import { validateJsonSchema } from './schema.mjs';

const ISOLATION = ['contract_only', 'process_isolated', 'os_enforced'];

export function isSafeRelativePath(value) {
  if (typeof value !== 'string' || value.length === 0 || value.length > 1024) return false;
  if (value.startsWith('/') || value.includes('\\') || value.includes(':') || value.includes('\0')) return false;
  const parts = value.split('/');
  return parts.every((part) => part !== '' && part !== '.' && part !== '..');
}

function pathsOverlap(first, second) {
  return first === second || first.startsWith(`${second}/`) || second.startsWith(`${first}/`);
}

function rejectDuplicatePaths(entries, label, errors) {
  const seen = new Set();
  for (const entry of entries) {
    if (seen.has(entry.path)) errors.push(`${label} contains duplicate path ${entry.path}`);
    seen.add(entry.path);
  }
}

function validateScoreSemantics(document, errors) {
  rejectDuplicatePaths(document.inputs, 'score.inputs', errors);
}

function validateLeaseSemantics(document, errors) {
  if (Date.parse(document.expiresAt) <= Date.parse(document.createdAt)) {
    errors.push('lease expiresAt must be later than createdAt');
  }

  const mounts = Object.values(document.mounts);
  for (let first = 0; first < mounts.length; first += 1) {
    for (let second = first + 1; second < mounts.length; second += 1) {
      if (pathsOverlap(mounts[first], mounts[second])) {
        errors.push('input, work and output mounts must not overlap');
      }
    }
  }
}

function validateReceiptSemantics(document, errors) {
  errors.push(...verifyReceiptChain(document.events).errors);
  if (document.candidate !== null) {
    rejectDuplicatePaths(document.candidate.sources, 'candidate.sources', errors);
    rejectDuplicatePaths(document.candidate.artifacts, 'candidate.artifacts', errors);
    rejectDuplicatePaths(document.candidate.evidence, 'candidate.evidence', errors);
  }
  if (document.receiptSha256 !== hashReceipt(document)) {
    errors.push('receiptSha256 is invalid');
  }
}

export function validateDocument(document) {
  const schemaResult = validateJsonSchema(document);
  if (!schemaResult.ok) return schemaResult;

  const errors = [];
  if (document.schema === 'aya.score/v0') validateScoreSemantics(document, errors);
  if (document.schema === 'aya.lease/v0') validateLeaseSemantics(document, errors);
  if (document.schema === 'aya.receipt/v0') validateReceiptSemantics(document, errors);
  return { ok: errors.length === 0, errors };
}

function addDocumentErrors(label, document, errors) {
  const result = validateDocument(document);
  errors.push(...result.errors.map((error) => `${label}: ${error}`));
}

export function validateAdmission(score, lease, capability) {
  const errors = [];
  addDocumentErrors('score', score, errors);
  addDocumentErrors('lease', lease, errors);
  addDocumentErrors('capability', capability, errors);
  if (errors.length > 0) return { ok: false, errors };

  const scoreDigest = sha256Json(score);
  if (lease.scoreSha256 !== scoreDigest) {
    errors.push('lease.scoreSha256 does not match the admitted score');
  }

  const scoreRequired = ISOLATION.indexOf(score.isolationRequired);
  const leaseRequired = ISOLATION.indexOf(lease.isolationRequired);
  const available = ISOLATION.indexOf(capability.isolationLevel);

  if (leaseRequired < scoreRequired) {
    errors.push(`lease isolation ${lease.isolationRequired} downgrades score requirement ${score.isolationRequired}`);
  }
  if (available < leaseRequired) {
    errors.push(`runtime isolation ${capability.isolationLevel} does not satisfy ${lease.isolationRequired}`);
  }
  if (!capability.networkModes.includes(lease.network)) {
    errors.push(`runtime does not support network mode ${lease.network}`);
  }

  return { ok: errors.length === 0, errors };
}

function sameSourceSet(scoreInputs, receiptSources, errors) {
  rejectDuplicatePaths(scoreInputs, 'score.inputs', errors);
  rejectDuplicatePaths(receiptSources, 'candidate.sources', errors);
  const expected = new Map(scoreInputs.map((item) => [item.path, item.sha256]));
  const actual = new Map(receiptSources.map((item) => [item.path, item]));

  if (expected.size !== actual.size) errors.push('candidate source set does not match score inputs');

  for (const [path, digest] of expected) {
    const source = actual.get(path);
    if (!source) {
      errors.push(`candidate is missing source verification for ${path}`);
      continue;
    }
    if (source.beforeSha256 !== digest) errors.push(`source ${path} beforeSha256 does not match score input`);
    if (source.afterSha256 !== source.beforeSha256) errors.push(`source ${path} changed during the workcell`);
  }
}

function isUnderMount(path, mount) {
  return path.startsWith(`${mount}/`);
}

export function validateReceiptAgainstContext(score, lease, receipt) {
  const errors = [];
  addDocumentErrors('score', score, errors);
  addDocumentErrors('lease', lease, errors);
  addDocumentErrors('receipt', receipt, errors);
  if (errors.length > 0) return { ok: false, errors };

  const scoreDigest = sha256Json(score);
  if (lease.scoreSha256 !== scoreDigest) errors.push('lease.scoreSha256 does not match score');
  if (receipt.scoreSha256 !== scoreDigest) errors.push('receipt.scoreSha256 does not match score');
  if (receipt.leaseSha256 !== sha256Json(lease)) errors.push('receipt.leaseSha256 does not match lease');

  for (const input of score.inputs) {
    if (!isUnderMount(input.path, lease.mounts.input)) {
      errors.push(`score input ${input.path} is outside input mount ${lease.mounts.input}`);
    }
  }

  if (receipt.status === 'candidate_ready') {
    sameSourceSet(score.inputs, receipt.candidate.sources, errors);
    for (const source of receipt.candidate.sources) {
      if (!isUnderMount(source.path, lease.mounts.input)) {
        errors.push(`candidate source ${source.path} is outside input mount ${lease.mounts.input}`);
      }
    }

    const evidenceKinds = new Set(receipt.candidate.evidence.map((item) => item.kind));
    for (const required of score.evidenceRequired) {
      if (!evidenceKinds.has(required)) errors.push(`candidate is missing required evidence kind ${required}`);
    }

    for (const item of [...receipt.candidate.artifacts, ...receipt.candidate.evidence]) {
      if (!isUnderMount(item.path, lease.mounts.output)) {
        errors.push(`candidate path ${item.path} is outside output mount ${lease.mounts.output}`);
      }
    }
  }

  return { ok: errors.length === 0, errors };
}
