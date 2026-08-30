/**
 * Zero-Trust Architecture Types & Interfaces
 */

export enum TrustTier {
  Untrusted = 0,
  Low = 1,
  Medium = 2,
  High = 3,
  VerifiedZeroTrust = 4,
}

export enum NetworkSegment {
  PublicEdge = 'public-edge',
  DMZ = 'dmz',
  ApplicationCore = 'application-core',
  SecureVault = 'secure-vault',
  ConsensusEngine = 'consensus-engine',
}

export interface SpiffeIdentity {
  trustDomain: string;
  workloadPath: string;
  spiffeId: string; // e.g. "spiffe://auditledger.org/ns/prod/sa/relayer"
  principal: string;
  issuedAt: number;
  expiresAt: number;
}

export interface DevicePosture {
  deviceId: string;
  platform: 'linux' | 'macos' | 'windows' | 'ios' | 'android';
  hasHardwareTpm: boolean;
  isDiskEncrypted: boolean;
  isEdrActive: boolean;
  isUncompromised: boolean;
  osVersion: string;
  postureScore: number;
  verifiedAt: number;
}

export interface ContinuousSession {
  sessionId: string;
  principal: string;
  spiffeId?: string;
  deviceId: string;
  dynamicRiskScore: number;
  trustTier: TrustTier;
  startedAt: number;
  lastHeartbeatAt: number;
  maxLifetimeSeconds: number;
  isRevoked: boolean;
}

export interface CapabilityGrant {
  grantId: string;
  grantee: string;
  allowedCapabilities: string[];
  targetSegment: NetworkSegment;
  requiredTrustTier: TrustTier;
  expiresAt: number;
  grantedBy: string;
}

export interface ZeroTrustContext {
  identity?: SpiffeIdentity;
  device?: DevicePosture;
  session: ContinuousSession;
  activeCapabilities: string[];
  currentSegment: NetworkSegment;
}
