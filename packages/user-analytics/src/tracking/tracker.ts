import { randomBytes } from 'crypto';
import { UserBehaviorEvent } from '../types';
import { ConsentManager } from '../privacy/consent';
import { AnalyticsEventStore } from '../privacy/erasure';

export class InMemoryAnalyticsStore implements AnalyticsEventStore {
  public events: UserBehaviorEvent[] = [];
  public sessions: Map<string, { sessionId: string; anonymousId: string; startTime: number; lastActivity: number; events: string[] }> = new Map();

  public async saveEvent(event: UserBehaviorEvent): Promise<void> {
    this.events.push(event);
    const session = this.sessions.get(event.sessionId);
    if (session) {
      session.lastActivity = event.timestamp;
      session.events.push(event.eventName);
    }
  }

  public async deleteUserEvents(anonymousId: string): Promise<number> {
    const beforeCount = this.events.length;
    this.events = this.events.filter((e) => e.anonymousId !== anonymousId);
    return beforeCount - this.events.length;
  }

  public async deleteUserSessions(anonymousId: string): Promise<number> {
    let count = 0;
    for (const [id, session] of this.sessions.entries()) {
      if (session.anonymousId === anonymousId) {
        this.sessions.delete(id);
        count++;
      }
    }
    return count;
  }

  public getEvents(): UserBehaviorEvent[] {
    return [...this.events];
  }
}

export class AnalyticsTracker {
  constructor(
    private consentManager: ConsentManager,
    private store: InMemoryAnalyticsStore = new InMemoryAnalyticsStore()
  ) {}

  public getStore(): InMemoryAnalyticsStore {
    return this.store;
  }

  public startSession(anonymousId: string, now: number = Date.now()): string {
    const sessionId = `sess_${randomBytes(12).toString('hex')}`;
    this.store.sessions.set(sessionId, {
      sessionId,
      anonymousId,
      startTime: now,
      lastActivity: now,
      events: [],
    });
    return sessionId;
  }

  public async track(
    anonymousId: string,
    sessionId: string,
    eventName: string,
    properties: Record<string, any> = {},
    context: UserBehaviorEvent['context'] = {},
    now: number = Date.now()
  ): Promise<{ tracked: boolean; reason?: string; eventId?: string }> {
    // Privacy verification: check if user gave analytics consent
    if (!this.consentManager.hasConsent(anonymousId, 'analytics')) {
      return { tracked: false, reason: 'Consent not granted for analytics' };
    }

    const eventId = `evt_${randomBytes(12).toString('hex')}`;
    const event: UserBehaviorEvent = {
      eventId,
      anonymousId,
      sessionId,
      eventName,
      timestamp: now,
      properties,
      context,
    };

    await this.store.saveEvent(event);
    return { tracked: true, eventId };
  }
}
