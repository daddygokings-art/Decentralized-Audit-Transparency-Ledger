import { CapabilityGrant, NetworkSegment, TrustTier } from './types';

export class LeastPrivilegeAuthorizer {
  public static authorize(
    grant: CapabilityGrant,
    requestedCapability: string,
    destinationSegment: NetworkSegment,
    callerTrustTier: TrustTier
  ): boolean {
    const now = Math.floor(Date.now() / 1000);

    if (now >= grant.expiresAt) {
      return false;
    }

    if (callerTrustTier < grant.requiredTrustTier) {
      return false;
    }

    if (grant.targetSegment !== destinationSegment && grant.targetSegment !== NetworkSegment.ApplicationCore) {
      return false;
    }

    return grant.allowedCapabilities.includes(requestedCapability) || grant.allowedCapabilities.includes('*');
  }
}
