import { ContinuousSession, TrustTier } from './types';

export class ContinuousVerificationManager {
  private activeSessions: Map<string, ContinuousSession> = new Map();
  private maxIdleSeconds: number;

  constructor(maxIdleSeconds: number = 900) {
    this.maxIdleSeconds = maxIdleSeconds;
  }

  public createSession(
    sessionId: string,
    principal: string,
    deviceId: string,
    trustTier: TrustTier,
    spiffeId?: string,
    maxLifetimeSeconds: number = 28800
  ): ContinuousSession {
    const now = Math.floor(Date.now() / 1000);
    const session: ContinuousSession = {
      sessionId,
      principal,
      deviceId,
      spiffeId,
      dynamicRiskScore: 0,
      trustTier,
      startedAt: now,
      lastHeartbeatAt: now,
      maxLifetimeSeconds,
      isRevoked: false,
    };
    this.activeSessions.set(sessionId, session);
    return session;
  }

  public evaluateSession(sessionId: string, newRiskFactor: number = 0): { valid: boolean; reason?: string } {
    const session = this.activeSessions.get(sessionId);
    if (!session) {
      return { valid: false, reason: 'Session not found' };
    }

    if (session.isRevoked) {
      return { valid: false, reason: 'Session has been revoked' };
    }

    const now = Math.floor(Date.now() / 1000);
    if (now - session.startedAt > session.maxLifetimeSeconds) {
      return { valid: false, reason: 'Session max lifetime exceeded' };
    }

    if (now - session.lastHeartbeatAt > this.maxIdleSeconds) {
      return { valid: false, reason: 'Session heartbeat idle timeout' };
    }

    session.dynamicRiskScore = Math.min(100, session.dynamicRiskScore + newRiskFactor);
    if (session.dynamicRiskScore >= 80) {
      session.isRevoked = true;
      return { valid: false, reason: 'Dynamic risk threshold breached' };
    }

    session.lastHeartbeatAt = now;
    return { valid: true };
  }

  public revokeSession(sessionId: string): void {
    const session = this.activeSessions.get(sessionId);
    if (session) {
      session.isRevoked = true;
    }
  }
}
