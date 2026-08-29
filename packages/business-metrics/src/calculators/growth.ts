import { EventGrowthKPIs } from '../types';

export interface EventRecord {
  timestamp: number;
  eventType: string;
  category?: string;
  bytesCount?: number;
}

export class EventGrowthMetricsCalculator {
  public static calculate(
    events: EventRecord[],
    historicalDailyVolumes: number[] = [],
    now: number = Date.now()
  ): EventGrowthKPIs {
    const ONE_DAY = 24 * 60 * 60 * 1000;
    const ONE_WEEK = 7 * ONE_DAY;
    const ONE_MONTH = 30 * ONE_DAY;

    const dayStart = now - ONE_DAY;
    const prevDayStart = now - 2 * ONE_DAY;
    const weekStart = now - ONE_WEEK;
    const prevWeekStart = now - 2 * ONE_WEEK;
    const monthStart = now - ONE_MONTH;
    const prevMonthStart = now - 2 * ONE_MONTH;

    let todayCount = 0;
    let prevDayCount = 0;
    let thisWeekCount = 0;
    let prevWeekCount = 0;
    let thisMonthCount = 0;
    let prevMonthCount = 0;
    let totalBytes = 0;

    const categoryCounts: Record<string, number> = {};
    const secondBuckets: Map<number, number> = new Map();

    for (const ev of events) {
      totalBytes += ev.bytesCount || 256;
      const cat = ev.category || 'general';
      categoryCounts[cat] = (categoryCounts[cat] || 0) + 1;

      if (ev.timestamp > dayStart && ev.timestamp <= now) {
        todayCount++;
        const sec = Math.floor(ev.timestamp / 1000);
        secondBuckets.set(sec, (secondBuckets.get(sec) || 0) + 1);
      } else if (ev.timestamp >= prevDayStart && ev.timestamp <= dayStart) {
        prevDayCount++;
      }

      if (ev.timestamp > weekStart && ev.timestamp <= now) {
        thisWeekCount++;
      } else if (ev.timestamp >= prevWeekStart && ev.timestamp <= weekStart) {
        prevWeekCount++;
      }

      if (ev.timestamp > monthStart && ev.timestamp <= now) {
        thisMonthCount++;
      } else if (ev.timestamp >= prevMonthStart && ev.timestamp <= monthStart) {
        prevMonthCount++;
      }
    }

    const calcGrowth = (current: number, prev: number): number => {
      if (prev === 0) return current > 0 ? 100.0 : 0.0;
      return Number((((current - prev) / prev) * 100).toFixed(2));
    };

    const dodGrowthPct = calcGrowth(todayCount, prevDayCount);
    const wowGrowthPct = calcGrowth(thisWeekCount, prevWeekCount);
    const momGrowthPct = calcGrowth(thisMonthCount, prevMonthCount);

    const secondsInDay = 86400;
    const eventsPerSecondAvg = Number((todayCount / secondsInDay).toFixed(4));
    let eventsPerSecondPeak = 0;
    for (const count of secondBuckets.values()) {
      if (count > eventsPerSecondPeak) eventsPerSecondPeak = count;
    }

    const totalEvents = events.length;
    const categoryBreakdown: Record<string, { count: number; percentage: number }> = {};
    for (const [cat, count] of Object.entries(categoryCounts)) {
      categoryBreakdown[cat] = {
        count,
        percentage: totalEvents > 0 ? Number(((count / totalEvents) * 100).toFixed(2)) : 0,
      };
    }

    // Statistical Anomaly Detection (Z-score calculation against historical daily volumes)
    let anomalyScore = 0;
    let isAnomaly = false;

    if (historicalDailyVolumes.length >= 5) {
      const mean = historicalDailyVolumes.reduce((a, b) => a + b, 0) / historicalDailyVolumes.length;
      const variance =
        historicalDailyVolumes.reduce((acc, val) => acc + Math.pow(val - mean, 2), 0) /
        historicalDailyVolumes.length;
      const stdDev = Math.sqrt(variance);

      if (stdDev > 0) {
        anomalyScore = Number(((todayCount - mean) / stdDev).toFixed(2));
        isAnomaly = Math.abs(anomalyScore) >= 2.5;
      }
    }

    return {
      totalEvents,
      dodGrowthPct,
      wowGrowthPct,
      momGrowthPct,
      eventsPerSecondAvg,
      eventsPerSecondPeak,
      totalDataFootprintBytes: totalBytes,
      categoryBreakdown,
      anomalyScore,
      isAnomaly,
    };
  }
}
