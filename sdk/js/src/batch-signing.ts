/**
 * #218 — SDK Batch Signing
 *
 * Provides batch signature generation and verification for multiple events.
 * Uses SHA-256 hashing of concatenated event preimages to create a single
 * Ed25519 signature covering an entire batch, reducing per-event signing overhead.
 *
 * ## Usage
 *
 *   import { BatchSigner } from 'audit-ledger-sdk';
 *
 *   const signer = new BatchSigner();
 *   const sig = await signer.signBatch(events, privateKey);
 *   const result = signer.verifyBatch(events, sig);
 */

import { createHash } from 'crypto';
import { SignedEvent, BatchSignature, BatchVerificationResult } from './types';

/**
 * Compute the SHA-256 hash of a single event preimage.
 *
 * Preimage format: `event_type || submitter || metadata || timestamp_le || nonce_le`
 */
function computeEventPreimage(event: SignedEvent): Buffer {
  const parts: Buffer[] = [];
  parts.push(Buffer.from(event.event_type, 'utf8'));
  parts.push(Buffer.from(event.submitter, 'utf8'));
  parts.push(Buffer.from(event.metadata, 'utf8'));

  // Timestamp as 8-byte little-endian
  const tsBuf = Buffer.alloc(8);
  tsBuf.writeUInt32LE(event.timestamp & 0xffffffff, 0);
  tsBuf.writeUInt32LE(Math.floor(event.timestamp / 0x100000000) & 0xffffffff, 4);
  parts.push(tsBuf);

  // Nonce as 4-byte little-endian
  const nonceBuf = Buffer.alloc(4);
  nonceBuf.writeUInt32LE(event.nonce, 0);
  parts.push(nonceBuf);

  return Buffer.concat(parts);
}

/**
 * Compute the SHA-256 hash of a single event.
 */
function computeEventHash(event: SignedEvent): Buffer {
  const preimage = computeEventPreimage(event);
  return createHash('sha256').update(preimage).digest();
}

/**
 * Compute the batch hash: SHA-256 of the concatenation of all individual event hashes.
 * The event hashes are sorted by (submitter, timestamp, nonce) to ensure
 * deterministic ordering regardless of array position.
 */
function computeBatchHash(events: SignedEvent[]): Buffer {
  const sorted = [...events].sort((a, b) => {
    if (a.submitter !== b.submitter) return a.submitter < b.submitter ? -1 : 1;
    if (a.timestamp !== b.timestamp) return a.timestamp - b.timestamp;
    return a.nonce - b.nonce;
  });

  const hashes = sorted.map((e) => computeEventHash(e));
  return createHash('sha256').update(Buffer.concat(hashes)).digest();
}

export class BatchSigner {
  /**
   * Sign a batch of events using an Ed25519 private key (hex-encoded).
   *
   * @param events     The events to sign
   * @param privateKey Hex-encoded Ed25519 private key (64 hex chars)
   * @param publicKey  Hex-encoded Ed25519 public key (64 hex chars)
   * @returns BatchSignature object that can be verified later
   */
  async signBatch(
    events: SignedEvent[],
    privateKey: string,
    publicKey: string,
  ): Promise<BatchSignature> {
    if (events.length === 0) {
      throw new Error('Cannot sign an empty batch');
    }

    const batchHash = computeBatchHash(events);
    const batchHashHex = batchHash.toString('hex');

    // Sign the batch hash using Ed25519
    // In production, this uses the platform's Ed25519 implementation.
    // For Node.js, we use the built-in crypto module.
    const { sign } = await import('crypto');
    const keyBuffer = Buffer.from(privateKey, 'hex');
    const signature = sign(undefined, batchHash, keyBuffer);

    return {
      pubkey: publicKey,
      signature: signature.toString('hex'),
      event_count: events.length,
      batch_hash: batchHashHex,
    };
  }

  /**
   * Verify a batch signature against the events.
   *
   * @param events   The events that were signed
   * @param batchSig The batch signature to verify
   * @returns BatchVerificationResult indicating success or failure
   */
  verifyBatch(events: SignedEvent[], batchSig: BatchSignature): BatchVerificationResult {
    if (events.length === 0) {
      return { valid: false, event_count: 0, batch_hash: '', error: 'No events to verify' };
    }

    if (events.length !== batchSig.event_count) {
      return {
        valid: false,
        event_count: events.length,
        batch_hash: '',
        error: `Event count mismatch: expected ${batchSig.event_count}, got ${events.length}`,
      };
    }

    const recomputedHash = computeBatchHash(events);
    const recomputedHex = recomputedHash.toString('hex');

    if (recomputedHex !== batchSig.batch_hash) {
      return {
        valid: false,
        event_count: events.length,
        batch_hash: recomputedHex,
        error: 'Batch hash mismatch — events may have been tampered with',
      };
    }

    // Verify the Ed25519 signature
    try {
      const { verify } = require('crypto');
      const pubkeyBuffer = Buffer.from(batchSig.pubkey, 'hex');
      const sigBuffer = Buffer.from(batchSig.signature, 'hex');
      const hashBuffer = Buffer.from(batchSig.batch_hash, 'hex');
      const valid = verify(undefined, hashBuffer, pubkeyBuffer, sigBuffer);

      if (!valid) {
        return {
          valid: false,
          event_count: events.length,
          batch_hash: batchSig.batch_hash,
          error: 'Ed25519 signature verification failed',
        };
      }

      return {
        valid: true,
        event_count: events.length,
        batch_hash: batchSig.batch_hash,
      };
    } catch (err) {
      return {
        valid: false,
        event_count: events.length,
        batch_hash: batchSig.batch_hash,
        error: `Signature verification error: ${err instanceof Error ? err.message : String(err)}`,
      };
    }
  }

  /**
   * Compute the batch hash for a set of events (without signing).
   * Useful for manual verification or comparison.
   */
  computeBatchHashHex(events: SignedEvent[]): string {
    return computeBatchHash(events).toString('hex');
  }

  /**
   * Compute the individual event hash for a single event.
   */
  computeEventHashHex(event: SignedEvent): string {
    return computeEventHash(event).toString('hex');
  }
}

export default BatchSigner;
