import { FunnelDefinition, FunnelAnalysisResult, FunnelStepResult, UserBehaviorEvent } from '../types';

export class FunnelAnalyzer {
  /**
   * Analyzes an event stream against a multi-step funnel definition.
   * Tracks user progression step-by-step in sequential order.
   */
  public static analyze(
    events: UserBehaviorEvent[],
    funnel: FunnelDefinition
  ): FunnelAnalysisResult {
    const { steps, maxConversionWindowHours = 24 } = funnel;
    const maxWindowMs = maxConversionWindowHours * 60 * 60 * 1000;

    if (steps.length === 0) {
      return {
        funnelId: funnel.id,
        funnelName: funnel.name,
        totalUsersEntered: 0,
        totalUsersCompleted: 0,
        overallConversionRatePct: 0,
        stepResults: [],
        biggestDropoffStep: 'none',
      };
    }

    // Group events by user, sorted by timestamp
    const userEventsMap: Map<string, UserBehaviorEvent[]> = new Map();
    for (const ev of events) {
      if (!userEventsMap.has(ev.anonymousId)) {
        userEventsMap.set(ev.anonymousId, []);
      }
      userEventsMap.get(ev.anonymousId)!.push(ev);
    }

    for (const evList of userEventsMap.values()) {
      evList.sort((a, b) => a.timestamp - b.timestamp);
    }

    // Evaluate progression for each user
    const usersAtStep: number[] = new Array(steps.length).fill(0);
    const stepDurationsMs: number[][] = steps.map(() => []);

    for (const [userId, userEvs] of userEventsMap.entries()) {
      let currentStepIdx = 0;
      let lastStepTimestamp = 0;
      let firstStepTimestamp = 0;

      for (const ev of userEvs) {
        if (currentStepIdx >= steps.length) break;

        const targetStep = steps[currentStepIdx];
        if (ev.eventName === targetStep.eventName) {
          // Verify properties if specified
          if (targetStep.requiredProperties) {
            let match = true;
            for (const [k, v] of Object.entries(targetStep.requiredProperties)) {
              if (ev.properties[k] !== v) match = false;
            }
            if (!match) continue;
          }

          if (currentStepIdx === 0) {
            firstStepTimestamp = ev.timestamp;
            lastStepTimestamp = ev.timestamp;
            usersAtStep[currentStepIdx]++;
            currentStepIdx++;
          } else {
            // Check conversion window
            if (ev.timestamp - firstStepTimestamp <= maxWindowMs && ev.timestamp >= lastStepTimestamp) {
              stepDurationsMs[currentStepIdx].push(ev.timestamp - lastStepTimestamp);
              usersAtStep[currentStepIdx]++;
              lastStepTimestamp = ev.timestamp;
              currentStepIdx++;
            }
          }
        }
      }
    }

    const stepResults: FunnelStepResult[] = [];
    let biggestDropoffStep = 'none';
    let maxDropoff = -1;

    for (let i = 0; i < steps.length; i++) {
      const entered = i === 0 ? usersAtStep[0] : usersAtStep[i - 1];
      const completed = usersAtStep[i];
      const convRate = entered > 0 ? Number(((completed / entered) * 100).toFixed(2)) : 0;
      const dropoff = entered > 0 ? Number((((entered - completed) / entered) * 100).toFixed(2)) : 0;

      const durations = stepDurationsMs[i];
      const avgDurationSec =
        durations && durations.length > 0
          ? Number(((durations.reduce((a, b) => a + b, 0) / durations.length) / 1000).toFixed(1))
          : 0;

      if (dropoff > maxDropoff && i > 0) {
        maxDropoff = dropoff;
        biggestDropoffStep = steps[i].step;
      }

      stepResults.push({
        step: steps[i].step,
        eventName: steps[i].eventName,
        usersEntered: entered,
        usersCompleted: completed,
        conversionRatePct: convRate,
        dropoffPct: dropoff,
        avgTimeToConvertSec: avgDurationSec,
      });
    }

    const totalEntered = usersAtStep[0] || 0;
    const totalCompleted = usersAtStep[steps.length - 1] || 0;
    const overallConversionRatePct =
      totalEntered > 0 ? Number(((totalCompleted / totalEntered) * 100).toFixed(2)) : 0;

    return {
      funnelId: funnel.id,
      funnelName: funnel.name,
      totalUsersEntered: totalEntered,
      totalUsersCompleted: totalCompleted,
      overallConversionRatePct,
      stepResults,
      biggestDropoffStep,
    };
  }
}
