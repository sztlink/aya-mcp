import { createHash } from 'node:crypto';
import canonicalizeRfc8785 from 'canonicalize';

export function canonicalize(value) {
  const result = canonicalizeRfc8785(value);
  if (typeof result !== 'string') throw new TypeError('Value cannot be represented as canonical JSON');
  return result;
}

export function sha256Text(text) {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

export function sha256Json(value) {
  return sha256Text(canonicalize(value));
}
