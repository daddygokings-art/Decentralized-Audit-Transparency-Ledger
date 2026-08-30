import { SubmitterActivityRecord, SubmitterKPIs } from '../types';

export class SubmitterMetricsCalculator {
  public static calculate(
    records: SubmitterActivityRecord[],
    now: number = Date.now()
  ): SubmitterKPIs {
    const ONE_DAY = 24 * 60 * 60 * 1000;
    const SEVEN_DAYS = 7 * ONE_DAY;
    const THIRTY_DAYS = 30 * ONE_DAY;

    const dayStart = now - ONE_DAY;
    const weekStart = now - SEVEN_DAYS;
    const monthStart = now - THIRTY_DAYS;

    const dauSet = new Set<string>();
    const wauSet = new Set<string>();
    const mauSet = new Set<string>();
    const priorToDaySet = new Set<string>();
    const countsMap: Map<string, number> = new Map();

    for (const r of records) {
      if (r.timestamp >= dayStart && r.timestamp <= now) {
        dauSet.add(r.submitter);
      }
      if (r.timestamp >= weekStart && r.timestamp <= now) {
        wauSet.add(r.submitter);
      }
      if (r.timestamp >= monthStart && r.timestamp <= now) {
        mauSet.add(r.submitter);
      }
      if (r.timestamp < dayStart && r.timestamp >= monthStart) {
        priorToDaySet.add(r.submitter);
      }

      countsMap.set(r.submitter, (countsMap.get(r.submitter) || 0) + 1);
    }

    const dau = dauSet.size;
    const wau = wauSet.size;
    const mau = mauSet.size;
    const dauToMauRatio = mau > 0 ? Number((dau / mau).toFixed(4)) : 0;

    let newSubmitters24h = 0;
    let returningSubmitters24h = 0;
    for (const submitter of dauSet) {
      if (priorToDaySet.has(submitter)) {
        returningSubmitters24h++;
      } else {
        newSubmitters24h++;
      }
    }

    // 7-day retention: submitters active in [now-14d, now-7d] who returned in [now-7d, now]
    const cohort7dStart = now - 14 * ONE_DAY;
    const cohort7dEnd = now - 7 * ONE_DAY;
    const cohort7d = new Set<string>();
    for (const r of records) {
      if (r.timestamp >= cohort7dStart && r.timestamp < cohort7dEnd) {
        cohort7d.add(r.submitter);
      }
    }
    let retained7d = 0;
    for (const s of cohort7d) {
      if (wauSet.has(s)) retained7d++;
    }
    const retentionRate7d = cohort7d.size > 0 ? Number(((retained7d / cohort7d.size) * 100).toFixed(2)) : 100.0;

    // 30-day retention
    const cohort30dStart = now - 60 * ONE_DAY;
    const cohort30dEnd = now - 30 * ONE_DAY;
    const cohort30d = new Set<string>();
    for (const r of records) {
      if (r.timestamp >= cohort30dStart && r.timestamp < cohort30dEnd) {
        cohort30d.add(r.submitter);
      }
    }
    let retained30d = 0;
    for (const s of cohort30d) {
      if (mauSet.has(s)) retained30d++;
    }
    const retentionRate30d = cohort30d.size > 0 ? Number(((retained30d / cohort30d.size) * 100).toFixed(2)) : 100.0;

    // Gini coefficient of submission volume
    const counts = Array.from(countsMap.values()).sort((a, b) => a - b);
    const n = counts.length;
    let gini = 0;
    if (n > 1) {
      const sum = counts.reduce((acc, c) => acc + c, 0);
      if (sum > 0) {
        let weightedSum = 0;
        for (let i = 0; i < n; i++) {
          weightedSum += (i + 1) * counts[i];
        }
        gini = Number(((2 * weightedSum) / (n * sum) - (n + 1) / n).toFixed(4));
        gini = Math.max(0, Math.min(1, gini));
      }
    }

    const totalCount = counts.reduce((a, b) => a + b, 0);
    const maxCount = counts.length > 0 ? counts[counts.length - 1] : 0;
    const topSubmitterSharePct = totalCount > 0 ? Number(((maxCount / totalCount) * 100).toFixed(2)) : 0;

    return {
      dau,
      wau,
      mau,
      dauToMauRatio,
      retentionRate7d,
      retentionRate30d,
      giniCoefficient: gini,
      newSubmitters24h,
      returningSubmitters24h,
      topSubmitterSharePct,
    };
  }
}
