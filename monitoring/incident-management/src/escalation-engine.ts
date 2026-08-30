import { EscalationPolicyConfig, Incident } from './types';
import { PagerDutyClient } from './pagerduty';
import { OpsgenieClient } from './opsgenie';
import { OnCallScheduler } from './on-call-scheduler';

export class EscalationEngine {
  private policies: Map<string, EscalationPolicyConfig> = new Map();
  private pdClient: PagerDutyClient;
  private ogClient: OpsgenieClient;
  private scheduler: OnCallScheduler;

  constructor(pdClient: PagerDutyClient, ogClient: OpsgenieClient, scheduler: OnCallScheduler) {
    this.pdClient = pdClient;
    this.ogClient = ogClient;
    this.scheduler = scheduler;
    this.seedDefaultPolicy();
  }

  private seedDefaultPolicy() {
    const defaultPolicy: EscalationPolicyConfig = {
      id: 'policy-default-stellar',
      name: 'Default Contract Event Escalation',
      description: 'Standard 3-tier escalation for ledger & smart contract anomalies',
      repeatCount: 3,
      tiers: [
        {
          tier: 1,
          delayMinutes: 0,
          targets: [{ type: 'SCHEDULE', id: 'audit-ledger-core', name: 'Primary On-Call' }],
        },
        {
          tier: 2,
          delayMinutes: 10,
          targets: [{ type: 'SCHEDULE', id: 'audit-ledger-core', name: 'Secondary On-Call' }],
        },
        {
          tier: 3,
          delayMinutes: 20,
          targets: [{ type: 'TEAM', id: 'lead-engineers', name: 'Engineering Leads' }],
        },
      ],
    };
    this.policies.set(defaultPolicy.id, defaultPolicy);
  }

  public registerPolicy(policy: EscalationPolicyConfig) {
    this.policies.set(policy.id, policy);
  }

  public getPolicy(id: string): EscalationPolicyConfig | undefined {
    return this.policies.get(id) || this.policies.get('policy-default-stellar');
  }

  public async evaluateEscalation(incident: Incident): Promise<{ escalated: boolean; newTier: number; notified: string[] }> {
    if (incident.status !== 'TRIGGERED') {
      return { escalated: false, newTier: incident.currentEscalationTier, notified: [] };
    }

    const policy = this.getPolicy(incident.escalationPolicyId || 'policy-default-stellar');
    if (!policy) return { escalated: false, newTier: incident.currentEscalationTier, notified: [] };

    const createdAt = new Date(incident.createdAt).getTime();
    const elapsedMinutes = (Date.now() - createdAt) / (1000 * 60);

    let targetTier = 1;
    let accumulatedDelay = 0;
    for (const tierConfig of policy.tiers) {
      accumulatedDelay += tierConfig.delayMinutes;
      if (elapsedMinutes >= accumulatedDelay) {
        targetTier = tierConfig.tier;
      }
    }

    if (targetTier > incident.currentEscalationTier) {
      const notified: string[] = [];
      if (targetTier === 1) {
        const primary = this.scheduler.getActivePrimary();
        if (primary) notified.push(primary.name);
      } else if (targetTier === 2) {
        const secondary = this.scheduler.getActiveSecondary();
        if (secondary) notified.push(secondary.name);
      } else {
        notified.push('Lead Engineers Group');
      }

      await this.pdClient.triggerAlert({
        alertName: `Escalation Tier ${targetTier}: ${incident.title}`,
        source: incident.source,
        severity: incident.severity,
        summary: `Incident ${incident.id} escalated to Tier ${targetTier} after ${Math.round(elapsedMinutes)} minutes unacknowledged`,
        details: {
          incidentId: incident.id,
          tier: targetTier,
          unacknowledgedMinutes: elapsedMinutes,
        },
        timestamp: new Date().toISOString(),
        dedupKey: `escalation-${incident.id}-tier-${targetTier}`,
      });

      return { escalated: true, newTier: targetTier, notified };
    }

    return { escalated: false, newTier: incident.currentEscalationTier, notified: [] };
  }
}
