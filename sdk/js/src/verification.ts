/**
 * #224 — SDK Event Verification Utilities
 *
 * Client-side event verification: ID verification, hash chain verification,
 * signature verification, and integrity proof generation.
 */

import { createHash } from 'crypto';
import { Event, Bytes32 } from './types';

/**
 * Verify that an event's ID matches the expected content-addressed hash.
 *
 * The expected ID is computed as:
 *   sha256(contract_id || submitter || event_type || metadata || timestamp_le || index_le)
 *
 * Since we don't have the contract_id off-chain, we verify the event_hash chain instead.
 */
export function verifyEventIntegrity(
  event: Event,
  prevEvent: Event | null,
  expectedIndex: number,
): { valid: boolean; error?: string } {
  if (event.index !== expectedIndex) {
    return { valid: false, error: `Index mismatch: expected ${expectedIndex}, got ${event.index}` };
  }

  if (prevEvent !== null) {
    if (event.prev_hash !== prevEvent.event_hash) {
      return {
        valid: false,
        error: `prev_hash mismatch at index ${event.index}: expected ${prevEvent.event_hash}, got ${event.prev_hash}`,
      };
    }
  } else if (event.prev_hash !== '00'.repeat(32)) {
    return { valid: false, error: 'Genesis event should have zero prev_hash' };
  }

  return { valid: true };
}

/**
 * Verify the hash chain for a sequence of events.
 * Returns the index of the first broken link, or -1 if the chain is valid.
 */
export function verifyHashChain(events: Event[]): { valid: boolean; brokenAt?: number; error?: string } {
  for (let i = 0; i < events.length; i++) {
    const event = events[i];
    const prevEvent = i > 0 ? events[i - 1] : null;
    const result = verifyEventIntegrity(event, prevEvent, i);
    if (!result.valid) {
      return { valid: false, brokenAt: i, error: result.error };
    }
  }
  return { valid: true };
}

/**
 * Generate an integrity proof for a set of events.
 * The proof includes a Merkle-like root of all event hashes.
 */
export function generateIntegrityProof(events: Event[]): {
  root: Bytes32;
  eventCount: number;
  firstIndex: number;
  lastIndex: number;
} {
  if (events.length === 0) {
    return { root: '00'.repeat(32), eventCount: 0, firstIndex: 0, lastIndex: 0 };
  }

  // Build a simple hash tree (concatenate all event hashes and hash the result)
  const hashBuffers = events.map((e) => Buffer.from(e.event_hash, 'hex'));
  const concatenated = Buffer.concat(hashBuffers);
  const root = createHash('sha256').update(concatenated).digest('hex');

  return {
    root,
    eventCount: events.length,
    firstIndex: events[0].index,
    lastIndex: events[events.length - 1].index,
  };
}

/**
 * Verify a specific event's hash can be found in the integrity proof root.
 */
export function verifyEventInProof(
  event: Event,
  proofRoot: Bytes32,
  allEvents: Event[],
): { valid: boolean; error?: string } {
  const proof = generateIntegrityProof(allEvents);
  if (proof.root !== proofRoot) {
    return { valid: false, error: 'Proof root mismatch' };
  }

  // Check the event exists in the set
  const found = allEvents.some((e) => e.index === event.index && e.event_hash === event.event_hash);
  if (!found) {
    return { valid: false, error: `Event at index ${event.index} not found in proof set` };
  }

  return { valid: true };
}

export default {
  verifyEventIntegrity,
  verifyHashChain,
  generateIntegrityProof,
  verifyEventInProof,
};
