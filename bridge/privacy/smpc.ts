/**
 * Secure Multi-Party Computation (SMPC) Protocol Engine
 *
 * Implements Shamir's Secret Sharing (threshold scheme) and Additive Secret Sharing
 * for private summation and distributed aggregation across audit relayer nodes.
 */

export interface SecretShare {
  index: number;
  value: bigint;
}

export class SmpcEngine {
  // Prime modulus for finite field operations (Mersenne prime 2^61 - 1)
  private readonly PRIME: bigint = 2305843009213693951n;

  /**
   * Split a secret into N additive shares such that sum(shares) % PRIME = secret
   */
  public generateAdditiveShares(secret: bigint, numParties: number): bigint[] {
    const shares: bigint[] = [];
    let runningSum = 0n;

    for (let i = 0; i < numParties - 1; i++) {
      // Random share in [0, PRIME)
      const randomShare = BigInt(Math.floor(Math.random() * 1000000000));
      shares.push(randomShare);
      runningSum = (runningSum + randomShare) % this.PRIME;
    }

    const lastShare = (secret - runningSum + this.PRIME * 100n) % this.PRIME;
    shares.push(lastShare);

    return shares;
  }

  /**
   * Reconstruct additive secret from shares
   */
  public reconstructAdditiveShares(shares: bigint[]): bigint {
    let sum = 0n;
    for (const share of shares) {
      sum = (sum + share) % this.PRIME;
    }
    return sum;
  }

  /**
   * Shamir's (k, n) polynomial secret sharing scheme
   */
  public generateShamirShares(secret: bigint, threshold: number, numParties: number): SecretShare[] {
    // Construct random polynomial f(x) = secret + a1*x + a2*x^2 + ... + a_(k-1)*x^(k-1)
    const coefficients: bigint[] = [secret];
    for (let i = 1; i < threshold; i++) {
      coefficients.push(BigInt(Math.floor(Math.random() * 1000000) + 1));
    }

    const shares: SecretShare[] = [];
    for (let x = 1; x <= numParties; x++) {
      let y = 0n;
      let xPower = 1n;
      const xBig = BigInt(x);

      for (const coeff of coefficients) {
        y = (y + coeff * xPower) % this.PRIME;
        xPower = (xPower * xBig) % this.PRIME;
      }

      shares.push({ index: x, value: y });
    }

    return shares;
  }

  /**
   * Reconstruct secret using Lagrange interpolation from k shares
   */
  public reconstructShamirSecret(shares: SecretShare[]): bigint {
    let secret = 0n;

    for (let i = 0; i < shares.length; i++) {
      const xi = BigInt(shares[i].index);
      const yi = shares[i].value;

      let numerator = 1n;
      let denominator = 1n;

      for (let j = 0; j < shares.length; j++) {
        if (i !== j) {
          const xj = BigInt(shares[j].index);
          numerator = (numerator * (0n - xj + this.PRIME)) % this.PRIME;
          denominator = (denominator * (xi - xj + this.PRIME)) % this.PRIME;
        }
      }

      const lagrangeBasis = (numerator * this.modInverse(denominator, this.PRIME)) % this.PRIME;
      secret = (secret + yi * lagrangeBasis) % this.PRIME;
    }

    return secret;
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
