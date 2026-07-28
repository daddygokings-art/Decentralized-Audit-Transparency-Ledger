import { gzipSync, gunzipSync, brotliCompressSync, brotliDecompressSync } from 'zlib';

export type CompressionAlgorithm = 'none' | 'gzip' | 'brotli';

export interface CompressionConfig {
  /** Algorithm to use when compressing. Default 'none' (disabled). */
  algorithm?: CompressionAlgorithm;
  /** Only compress payloads at or above this size in bytes. Default 0 (always compress if enabled). */
  threshold?: number;
}

export interface CompressionStats {
  algorithm: CompressionAlgorithm;
  originalBytes: number;
  compressedBytes: number;
  ratio: number;
  savedBytes: number;
}

export interface CompressionTotals {
  calls: number;
  totalOriginalBytes: number;
  totalCompressedBytes: number;
  totalSavedBytes: number;
  averageRatio: number;
}

// 1-byte envelope tag so decodeMetadata can auto-detect the algorithm used.
const TAG_NONE = 0x00;
const TAG_GZIP = 0x01;
const TAG_BROTLI = 0x02;

function toBuffer(data: Buffer | Uint8Array): Buffer {
  return Buffer.isBuffer(data) ? data : Buffer.from(data);
}

export function compress(data: Buffer | Uint8Array, algorithm: CompressionAlgorithm): Buffer {
  const input = toBuffer(data);
  switch (algorithm) {
    case 'none':
      return input;
    case 'gzip':
      return gzipSync(input);
    case 'brotli':
      return brotliCompressSync(input);
    default:
      throw new Error(`Unsupported compression algorithm: ${algorithm}`);
  }
}

export function decompress(data: Buffer | Uint8Array, algorithm: CompressionAlgorithm): Buffer {
  const input = toBuffer(data);
  switch (algorithm) {
    case 'none':
      return input;
    case 'gzip':
      return gunzipSync(input);
    case 'brotli':
      return brotliDecompressSync(input);
    default:
      throw new Error(`Unsupported compression algorithm: ${algorithm}`);
  }
}

/** Compress metadata (if configured) and prepend a 1-byte algorithm tag for later auto-decompression. */
export function encodeMetadata(
  data: Buffer | Uint8Array,
  config: CompressionConfig = {},
): { payload: Buffer; stats: CompressionStats } {
  const algorithm = config.algorithm ?? 'none';
  const threshold = config.threshold ?? 0;
  const input = toBuffer(data);

  if (algorithm === 'none' || input.length < threshold) {
    return {
      payload: Buffer.concat([Buffer.from([TAG_NONE]), input]),
      stats: {
        algorithm: 'none',
        originalBytes: input.length,
        compressedBytes: input.length,
        ratio: 1,
        savedBytes: 0,
      },
    };
  }

  const compressed = compress(input, algorithm);
  const tag = algorithm === 'gzip' ? TAG_GZIP : TAG_BROTLI;
  return {
    payload: Buffer.concat([Buffer.from([tag]), compressed]),
    stats: {
      algorithm,
      originalBytes: input.length,
      compressedBytes: compressed.length,
      ratio: input.length === 0 ? 1 : compressed.length / input.length,
      savedBytes: input.length - compressed.length,
    },
  };
}

/** Decode a payload produced by `encodeMetadata`, auto-detecting the algorithm from its tag byte. */
export function decodeMetadata(payload: Buffer | Uint8Array): Buffer {
  const buf = toBuffer(payload);
  if (buf.length === 0) return buf;
  const tag = buf[0];
  const body = buf.subarray(1);
  switch (tag) {
    case TAG_NONE:
      return Buffer.from(body);
    case TAG_GZIP:
      return gunzipSync(body);
    case TAG_BROTLI:
      return brotliDecompressSync(body);
    default:
      throw new Error(`Unknown compression tag: ${tag}`);
  }
}

/** Accumulates compression statistics across many encodeMetadata calls. */
export class CompressionStatsTracker {
  private calls = 0;
  private totalOriginalBytes = 0;
  private totalCompressedBytes = 0;
  private ratioSum = 0;

  record(stats: CompressionStats): void {
    this.calls += 1;
    this.totalOriginalBytes += stats.originalBytes;
    this.totalCompressedBytes += stats.compressedBytes;
    this.ratioSum += stats.ratio;
  }

  totals(): CompressionTotals {
    return {
      calls: this.calls,
      totalOriginalBytes: this.totalOriginalBytes,
      totalCompressedBytes: this.totalCompressedBytes,
      totalSavedBytes: this.totalOriginalBytes - this.totalCompressedBytes,
      averageRatio: this.calls === 0 ? 1 : this.ratioSum / this.calls,
    };
  }

  reset(): void {
    this.calls = 0;
    this.totalOriginalBytes = 0;
    this.totalCompressedBytes = 0;
    this.ratioSum = 0;
  }
}
