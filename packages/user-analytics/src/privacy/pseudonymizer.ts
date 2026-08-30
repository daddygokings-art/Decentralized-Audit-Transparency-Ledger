import { createHmac, randomBytes } from 'crypto';

export class Pseudonymizer {
  private salt: string;

  constructor(salt?: string) {
    this.salt = salt || process.env.ANALYTICS_SALT || randomBytes(32).toString('hex');
  }

  /**
   * Hashes a raw user identifier or wallet address with HMAC-SHA256.
   * Produces an irreversible, privacy-compliant pseudonymized ID.
   */
  public pseudonymize(identifier: string): string {
    if (!identifier) return '';
    const hmac = createHmac('sha256', this.salt);
    hmac.update(identifier.trim().toLowerCase());
    const hash = hmac.digest('hex').substring(0, 32);
    return `anon_${hash}`;
  }

  /**
   * Anonymizes IP addresses by zeroing out host octets / interface IDs.
   */
  public anonymizeIp(ip: string): string {
    if (!ip) return '';
    if (ip.includes('.')) {
      // IPv4: mask last octet (e.g. 192.168.1.0)
      const parts = ip.split('.');
      if (parts.length === 4) {
        return `${parts[0]}.${parts[1]}.${parts[2]}.0`;
      }
    } else if (ip.includes(':')) {
      // IPv6: mask host portion
      const parts = ip.split(':');
      if (parts.length >= 3) {
        return `${parts[0]}:${parts[1]}:${parts[2]}::`;
      }
    }
    return this.pseudonymize(ip);
  }
}
