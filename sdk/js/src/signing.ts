import {
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign as cryptoSign,
  verify as cryptoVerify,
  KeyObject,
} from 'crypto';

// DER prefixes for raw 32-byte Ed25519 keys (RFC 8410).
const ED25519_PRIVATE_DER_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');
const ED25519_PUBLIC_DER_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

export interface Ed25519Keypair {
  publicKey: Buffer;
  privateKey: Buffer;
}

/** Signature payload format used by the contract's `log_event_signed`: pubkey(32) || signature(64). */
export const SIGNATURE_PAYLOAD_LENGTH = 96;

function toBuffer(data: Buffer | Uint8Array): Buffer {
  return Buffer.isBuffer(data) ? data : Buffer.from(data);
}

function rawPrivateKeyToKeyObject(raw: Buffer | Uint8Array): KeyObject {
  const seed = toBuffer(raw);
  if (seed.length !== 32) throw new Error('Ed25519 private key must be 32 bytes');
  const der = Buffer.concat([ED25519_PRIVATE_DER_PREFIX, seed]);
  return createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
}

function rawPublicKeyToKeyObject(raw: Buffer | Uint8Array): KeyObject {
  const pub = toBuffer(raw);
  if (pub.length !== 32) throw new Error('Ed25519 public key must be 32 bytes');
  const der = Buffer.concat([ED25519_PUBLIC_DER_PREFIX, pub]);
  return createPublicKey({ key: der, format: 'der', type: 'spki' });
}

/** Generate a new random Ed25519 keypair as raw 32-byte buffers. */
export function generateKeypair(): Ed25519Keypair {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const pubDer = publicKey.export({ format: 'der', type: 'spki' }) as Buffer;
  const privDer = privateKey.export({ format: 'der', type: 'pkcs8' }) as Buffer;
  return {
    publicKey: Buffer.from(pubDer.subarray(pubDer.length - 32)),
    privateKey: Buffer.from(privDer.subarray(privDer.length - 32)),
  };
}

/** Derive the raw 32-byte public key from a raw 32-byte Ed25519 private key. */
export function derivePublicKey(privateKeyRaw: Buffer | Uint8Array): Buffer {
  const keyObj = rawPrivateKeyToKeyObject(privateKeyRaw);
  const pubKeyObj = createPublicKey(keyObj);
  const der = pubKeyObj.export({ format: 'der', type: 'spki' }) as Buffer;
  return Buffer.from(der.subarray(der.length - 32));
}

/** Sign an arbitrary message with a raw Ed25519 private key. Returns a 64-byte signature. */
export function signMessage(privateKeyRaw: Buffer | Uint8Array, message: Buffer | Uint8Array): Buffer {
  const keyObj = rawPrivateKeyToKeyObject(privateKeyRaw);
  return cryptoSign(null, toBuffer(message), keyObj);
}

/** Verify a raw Ed25519 signature against a message and public key. */
export function verifyMessage(
  publicKeyRaw: Buffer | Uint8Array,
  message: Buffer | Uint8Array,
  signature: Buffer | Uint8Array,
): boolean {
  try {
    const keyObj = rawPublicKeyToKeyObject(publicKeyRaw);
    return cryptoVerify(null, toBuffer(message), keyObj, toBuffer(signature));
  } catch {
    return false;
  }
}

/**
 * Build the 96-byte signature payload (pubkey || signature) expected by the
 * contract's `log_event_signed`. The signed message SHOULD be the event's
 * content-addressed ID.
 */
export function buildSignaturePayload(
  privateKeyRaw: Buffer | Uint8Array,
  message: Buffer | Uint8Array,
): Buffer {
  const publicKey = derivePublicKey(privateKeyRaw);
  const signature = signMessage(privateKeyRaw, message);
  return Buffer.concat([publicKey, signature]);
}

/** Verify a 96-byte signature payload (pubkey || signature) against a message. */
export function verifySignaturePayload(
  payload: Buffer | Uint8Array,
  message: Buffer | Uint8Array,
): boolean {
  const buf = toBuffer(payload);
  if (buf.length !== SIGNATURE_PAYLOAD_LENGTH) return false;
  const publicKey = buf.subarray(0, 32);
  const signature = buf.subarray(32, 96);
  return verifyMessage(publicKey, message, signature);
}

/** Sign a batch of messages with a single private key, producing one signature payload per message. */
export function signBatch(
  privateKeyRaw: Buffer | Uint8Array,
  messages: Array<Buffer | Uint8Array>,
): Buffer[] {
  const publicKey = derivePublicKey(privateKeyRaw);
  return messages.map((message) => {
    const signature = signMessage(privateKeyRaw, message);
    return Buffer.concat([publicKey, signature]);
  });
}

/** Verify a batch of (message, payload) pairs. Returns one boolean per pair, in order. */
export function verifyBatch(
  items: Array<{ message: Buffer | Uint8Array; payload: Buffer | Uint8Array }>,
): boolean[] {
  return items.map(({ message, payload }) => verifySignaturePayload(payload, message));
}
