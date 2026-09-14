import { readFileSync } from 'node:fs';
import Ajv2020 from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';

const SCHEMA_FILES = new Map([
  ['aya.score/v0', 'score.schema.json'],
  ['aya.lease/v0', 'lease.schema.json'],
  ['aya.capability-report/v0', 'capability-report.schema.json'],
  ['aya.receipt/v0', 'receipt.schema.json']
]);

const ajv = new Ajv2020({ allErrors: true, strict: true });
addFormats(ajv);

const validators = new Map();
for (const [name, file] of SCHEMA_FILES) {
  const url = new URL(`../spec/v0/schemas/${file}`, import.meta.url);
  const schema = JSON.parse(readFileSync(url, 'utf8'));
  validators.set(name, ajv.compile(schema));
}

export function validateJsonSchema(document) {
  const validator = validators.get(document?.schema);
  if (!validator) return { ok: false, errors: [`unsupported schema: ${String(document?.schema)}`] };
  if (validator(document)) return { ok: true, errors: [] };
  return {
    ok: false,
    errors: validator.errors.map((error) => `${error.instancePath || '/'} ${error.message}`)
  };
}

export function compiledSchemaNames() {
  return [...validators.keys()];
}
