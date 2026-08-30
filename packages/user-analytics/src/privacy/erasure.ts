import { ConsentManager } from './consent';

export interface AnalyticsEventStore {
  deleteUserEvents(anonymousId: string): Promise<number>;
  deleteUserSessions(anonymousId: string): Promise<number>;
}

export class DataErasureManager {
  constructor(
    private consentManager: ConsentManager,
    private store: AnalyticsEventStore
  ) {}

  /**
   * Executes a GDPR / CCPA "Right to be Forgotten" erasure request.
   * Completely purges all event records, sessions, and consent records for the pseudonymized ID.
   */
  public async executeRightToBeForgotten(anonymousId: string): Promise<{
    success: boolean;
    anonymousId: string;
    eventsDeleted: number;
    sessionsDeleted: number;
    consentDeleted: boolean;
    timestamp: string;
  }> {
    const eventsDeleted = await this.store.deleteUserEvents(anonymousId);
    const sessionsDeleted = await this.store.deleteUserSessions(anonymousId);
    const consentDeleted = this.consentManager.deleteConsent(anonymousId);

    return {
      success: true,
      anonymousId,
      eventsDeleted,
      sessionsDeleted,
      consentDeleted,
      timestamp: new Date().toISOString(),
    };
  }
}
