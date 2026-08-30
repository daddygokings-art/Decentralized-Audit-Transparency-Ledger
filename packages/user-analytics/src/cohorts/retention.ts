import { CohortRetentionResult, UserBehaviorEvent } from '../types';

export class CohortRetentionAnalyzer {
  public static calculateRetention(
    events: UserBehaviorEvent[],
    cohortStart: number,
    cohortEnd: number,
    intervalType: 'daily' | 'weekly' | 'monthly' = 'daily',
    intervalsCount = 7,
    now: number = Date.now()
  ): CohortRetentionResult {
    const INTERVAL_MS =
      intervalType === 'daily'
        ? 24 * 60 * 60 * 1000
        : intervalType === 'weekly'
        ? 7 * 24 * 60 * 60 * 1000
        : 30 * 24 * 60 * 60 * 1000;

    // Find users whose first event falls in [cohortStart, cohortEnd]
    const userFirstSeen: Map<string, number> = new Map();
    for (const ev of events) {
      const curr = userFirstSeen.get(ev.anonymousId);
      if (curr === undefined || ev.timestamp < curr) {
        userFirstSeen.set(ev.anonymousId, ev.timestamp);
      }
    }

    const cohortUsers = new Set<string>();
    for (const [userId, firstSeen] of userFirstSeen.entries()) {
      if (firstSeen >= cohortStart && firstSeen <= cohortEnd) {
        cohortUsers.add(userId);
      }
    }

    const cohortSize = cohortUsers.size;
    const intervals: CohortRetentionResult['intervals'] = [];

    for (let i = 0; i < intervalsCount; i++) {
      const intervalStart = cohortStart + i * INTERVAL_MS;
      const intervalEnd = intervalStart + INTERVAL_MS;

      const activeUsersInInterval = new Set<string>();
      for (const ev of events) {
        if (cohortUsers.has(ev.anonymousId) && ev.timestamp >= intervalStart && ev.timestamp < intervalEnd) {
          activeUsersInInterval.add(ev.anonymousId);
        }
      }

      const activeCount = i === 0 ? cohortSize : activeUsersInInterval.size;
      const ratePct = cohortSize > 0 ? Number(((activeCount / cohortSize) * 100).toFixed(2)) : 0;

      intervals.push({
        intervalNumber: i,
        label: `${intervalType.charAt(0).toUpperCase() + intervalType.slice(1)} ${i}`,
        activeUsers: activeCount,
        retentionRatePct: ratePct,
      });
    }

    const latestRetention = intervals.length > 1 ? intervals[intervals.length - 1].retentionRatePct : 100;
    const churnRatePct = Number((100 - latestRetention).toFixed(2));

    return {
      cohortId: `cohort_${new Date(cohortStart).toISOString().slice(0, 10)}`,
      periodType: intervalType,
      cohortSize,
      intervals,
      churnRatePct,
    };
  }
}
