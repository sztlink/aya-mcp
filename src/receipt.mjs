import { sha256Json } from './canonical.mjs';

export const ZERO_SHA256 = '0'.repeat(64);

export function hashReceiptEvent(event) {
  const { eventSha256: _ignored, ...unsigned } = event;
  return sha256Json(unsigned);
}

export function appendReceiptEvent(events, event) {
  const previousSha256 = events.length === 0
    ? ZERO_SHA256
    : events.at(-1).eventSha256;

  const unsigned = {
    index: events.length,
    at: event.at,
    kind: event.kind,
    summary: event.summary,
    previousSha256
  };

  return [...events, { ...unsigned, eventSha256: sha256Json(unsigned) }];
}

export function verifyReceiptChain(events) {
  const errors = [];

  events.forEach((event, index) => {
    const expectedPrevious = index === 0 ? ZERO_SHA256 : events[index - 1].eventSha256;
    if (event.index !== index) errors.push(`events[${index}].index must equal ${index}`);
    if (event.previousSha256 !== expectedPrevious) {
      errors.push(`events[${index}].previousSha256 does not match the previous event`);
    }
    if (event.eventSha256 !== hashReceiptEvent(event)) {
      errors.push(`events[${index}].eventSha256 is invalid`);
    }
  });

  return { ok: errors.length === 0, errors };
}

export function hashReceipt(receipt) {
  const { receiptSha256: _ignored, ...unsigned } = receipt;
  return sha256Json(unsigned);
}

export function sealReceipt(receipt) {
  return { ...receipt, receiptSha256: hashReceipt(receipt) };
}

export function verifyReceiptDigest(receipt) {
  return receipt.receiptSha256 === hashReceipt(receipt);
}
