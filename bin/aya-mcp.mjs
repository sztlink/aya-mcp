#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import { sha256Json } from '../src/canonical.mjs';
import { validateDocument, validateReceiptAgainstContext } from '../src/validate.mjs';

async function readJson(file) {
  return JSON.parse(await readFile(file, 'utf8'));
}

function usage() {
  console.error('Usage:');
  console.error('  aya-mcp validate <document.json>');
  console.error('  aya-mcp digest <document.json>');
  console.error('  aya-mcp verify-receipt <receipt.json> <score.json> <lease.json>');
  process.exitCode = 2;
}

function report(result, label) {
  if (!result.ok) {
    for (const error of result.errors) console.error(`error: ${error}`);
    process.exitCode = 1;
  } else {
    console.log(`valid: ${label}`);
  }
}

const [, , command, ...files] = process.argv;

try {
  if (command === 'validate' && files.length === 1) {
    const document = await readJson(files[0]);
    report(validateDocument(document), document.schema);
  } else if (command === 'digest' && files.length === 1) {
    console.log(sha256Json(await readJson(files[0])));
  } else if (command === 'verify-receipt' && files.length === 3) {
    const [receipt, score, lease] = await Promise.all(files.map(readJson));
    report(validateReceiptAgainstContext(score, lease, receipt), 'aya.receipt/v0 with score and lease context');
  } else {
    usage();
  }
} catch (error) {
  console.error(`error: ${error.message}`);
  process.exitCode = 1;
}
