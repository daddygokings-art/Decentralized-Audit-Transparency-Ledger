import { DevicePosture, TrustTier } from './types';

export class DeviceTrustEvaluator {
  public static calculatePostureScore(posture: Omit<DevicePosture, 'postureScore' | 'verifiedAt'>): DevicePosture {
    if (!posture.isUncompromised) {
      return {
        ...posture,
        postureScore: 0,
        verifiedAt: Math.floor(Date.now() / 1000),
      };
    }

    let score = 20; // Base score for uncompromised OS
    if (posture.hasHardwareTpm) score += 30;
    if (posture.isDiskEncrypted) score += 25;
    if (posture.isEdrActive) score += 25;

    return {
      ...posture,
      postureScore: Math.min(100, score),
      verifiedAt: Math.floor(Date.now() / 1000),
    };
  }

  public static deriveTrustTier(postureScore: number, dynamicRiskScore: number): TrustTier {
    if (postureScore < 40 || dynamicRiskScore >= 70) {
      return TrustTier.Untrusted;
    } else if (postureScore < 60 || dynamicRiskScore >= 50) {
      return TrustTier.Low;
    } else if (postureScore < 80 || dynamicRiskScore >= 30) {
      return TrustTier.Medium;
    } else if (postureScore < 95 || dynamicRiskScore >= 10) {
      return TrustTier.High;
    } else {
      return TrustTier.VerifiedZeroTrust;
    }
  }
}
