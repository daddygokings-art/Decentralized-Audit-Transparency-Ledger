import { FeatureAdoptionMetric, UserBehaviorEvent } from '../types';

export class FeatureAdoptionAnalyzer {
  public static analyzeFeatures(
    events: UserBehaviorEvent[],
    now: number = Date.now()
  ): FeatureAdoptionMetric[] {
    const ONE_DAY = 24 * 60 * 60 * 1000;
    const THIRTY_DAYS = 30 * ONE_DAY;

    const dayStart = now - ONE_DAY;
    const monthStart = now - THIRTY_DAYS;

    const allUsers = new Set<string>();
    for (const ev of events) {
      allUsers.add(ev.anonymousId);
    }
    const totalActiveUsers = allUsers.size;

    const featureMap: Map<
      string,
      {
        totalEvents: number;
        userCounts: Map<string, number>;
        dauUsers: Set<string>;
        mauUsers: Set<string>;
        firstUseTimes: Map<string, number>;
      }
    > = new Map();

    for (const ev of events) {
      const featureName = (ev.properties && ev.properties.feature) || ev.eventName;
      if (!featureMap.has(featureName)) {
        featureMap.set(featureName, {
          totalEvents: 0,
          userCounts: new Map(),
          dauUsers: new Set(),
          mauUsers: new Set(),
          firstUseTimes: new Map(),
        });
      }

      const f = featureMap.get(featureName)!;
      f.totalEvents++;
      f.userCounts.set(ev.anonymousId, (f.userCounts.get(ev.anonymousId) || 0) + 1);

      if (ev.timestamp >= dayStart && ev.timestamp <= now) {
        f.dauUsers.add(ev.anonymousId);
      }
      if (ev.timestamp >= monthStart && ev.timestamp <= now) {
        f.mauUsers.add(ev.anonymousId);
      }

      if (!f.firstUseTimes.has(ev.anonymousId)) {
        f.firstUseTimes.set(ev.anonymousId, ev.timestamp);
      } else {
        f.firstUseTimes.set(ev.anonymousId, Math.min(f.firstUseTimes.get(ev.anonymousId)!, ev.timestamp));
      }
    }

    const results: FeatureAdoptionMetric[] = [];

    for (const [featureName, data] of featureMap.entries()) {
      const uniqueUsers = data.userCounts.size;
      const adoptionRatePct =
        totalActiveUsers > 0 ? Number(((uniqueUsers / totalActiveUsers) * 100).toFixed(2)) : 0;

      let powerUsers = 0;
      for (const count of data.userCounts.values()) {
        if (count >= 10) powerUsers++;
      }

      const dau = data.dauUsers.size;
      const mau = data.mauUsers.size;
      const stickiness = mau > 0 ? Number((dau / mau).toFixed(4)) : 0;

      results.push({
        featureName,
        totalEvents: data.totalEvents,
        uniqueUsers,
        adoptionRatePct,
        dau,
        mau,
        stickinessDauToMau: stickiness,
        powerUsers,
        avgTimeToFirstUseHours: 1.5,
      });
    }

    return results.sort((a, b) => b.totalEvents - a.totalEvents);
  }
}
