/**
 * Additive Homomorphic Encryption (Paillier Cryptosystem Simulator)
 *
 * Implements key generation, encryption, homomorphic addition:
 *   E(m1) * E(m2) mod n^2 = E(m1 + m2 mod n)
 * and scalar multiplication without decryption.
 */

export interface PaillierPublicKey {
  n: bigint;
  nSquared: bigint;
  g: bigint;
}

export interface PaillierPrivateKey {
  lambda: bigint;
  mu: bigint;
}

export interface PaillierKeyPair {
  publicKey: PaillierPublicKey;
  privateKey: PaillierPrivateKey;
}

export class HomomorphicEncryptionEngine {
  /**
   * Generate lightweight demo Paillier key pair
   */
  public generateKeyPair(p: bigint = 61n, q: bigint = 53n): PaillierKeyPair {
    const n = p * q;
    const nSquared = n * n;
    const lambda = ((p - 1n) * (q - 1n)); // lcm(p-1, q-1) simplified for coprime primes
    const g = n + 1n; // Standard Paillier simplification where L(g^lambda mod n^2) = lambda
    const mu = this.modInverse(lambda, n);

    return {
      publicKey: { n, nSquared, g },
      privateKey: { lambda, mu },
    };
  }

  /**
   * Encrypt a plaintext metric m in [0, n)
   */
  public encrypt(m: bigint, pk: PaillierPublicKey, r: bigint = 7n): bigint {
    const gm = this.modPow(pk.g, m, pk.nSquared);
    const rn = this.modPow(r, pk.n, pk.nSquared);
    return (gm * rn) % pk.nSquared;
  }

  /**
   * Homomorphically add two ciphertexts: c1 * c2 mod n^2
   */
  public addCiphertexts(c1: bigint, c2: bigint, pk: PaillierPublicKey): bigint {
    return (c1 * c2) % pk.nSquared;
  }

  /**
   * Multiply encrypted ciphertext by a plaintext scalar: c^k mod n^2 = E(k * m)
   */
  public multiplyScalar(c: bigint, k: bigint, pk: PaillierPublicKey): bigint {
    return this.modPow(c, k, pk.nSquared);
  }

  /**
   * Decrypt a ciphertext
   */
  public decrypt(c: bigint, pk: PaillierPublicKey, sk: PaillierPrivateKey): bigint {
    const u = this.modPow(c, sk.lambda, pk.nSquared);
    const l = (u - 1n) / pk.n;
    return (l * sk.mu) % pk.n;
  }

  private modPow(base: bigint, exp: bigint, mod: bigint): bigint {
    let res = 1n;
    let b = base % mod;
    let e = exp;

    while (e > 0n) {
      if (e % 2n === 1n) res = (res * b) % mod;
      b = (b * b) % mod;
      e /= 2n;
    }

    return res;
  }

  private modInverse(a: bigint, m: bigint): bigint {
    let [m0, x0, x1] = [m, 0n, 1n];
    let aVal = a % m;
    if (m === 1n) return 0n;

    while (aVal > 1n) {
      const q = aVal / m0;
      let t = m0;
      m0 = aVal % m0;
      aVal = t;
      t = x0;
      x0 = x1 - q * x0;
      x1 = t;
    }

    if (x1 < 0n) x1 += m;
    return x1;
  }
}
