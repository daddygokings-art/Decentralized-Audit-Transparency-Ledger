import { SpiffeIdentity } from './types';

export class SpiffeIdentityValidator {
  private allowedTrustDomains: Set<string>;

  constructor(allowedDomains: string[] = ['auditledger.org', 'internal.stellar.org']) {
    this.allowedTrustDomains = new Set(allowedDomains);
  }

  public parseSpiffeId(spiffeUri: string): SpiffeIdentity | null {
    try {
      if (!spiffeUri.startsWith('spiffe://')) {
        return null;
      }
      const withoutScheme = spiffeUri.substring(9);
      const slashIdx = withoutScheme.indexOf('/');
      if (slashIdx === -1) {
        return null;
      }
      const trustDomain = withoutScheme.substring(0, slashIdx);
      const workloadPath = withoutScheme.substring(slashIdx);

      if (!this.allowedTrustDomains.has(trustDomain)) {
        return null;
      }

      return {
        trustDomain,
        workloadPath,
        spiffeId: spiffeUri,
        principal: workloadPath.replace(/^\//, '').replace(/\//g, ':'),
        issuedAt: Math.floor(Date.now() / 1000),
        expiresAt: Math.floor(Date.now() / 1000) + 3600,
      };
    } catch {
      return null;
    }
  }

  public validateWorkloadIdentity(identity: SpiffeIdentity): boolean {
    const now = Math.floor(Date.now() / 1000);
    if (now > identity.expiresAt || now < identity.issuedAt) {
      return false;
    }
    return this.allowedTrustDomains.has(identity.trustDomain);
  }
}
