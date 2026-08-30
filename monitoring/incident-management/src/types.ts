export type SeverityLevel = 'SEV-1' | 'SEV-2' | 'SEV-3' | 'SEV-4' | 'SEV-5';

export type IncidentStatus = 'TRIGGERED' | 'ACKNOWLEDGED' | 'MITIGATED' | 'RESOLVED' | 'CLOSED';

export interface IncidentAlertPayload {
  alertName: string;
  source: string;
  contractAddress?: string;
  severity: SeverityLevel;
  summary: string;
  details: Record<string, unknown>;
  timestamp: string;
  dedupKey?: string;
}

export interface TimelineItem {
  id: string;
  timestamp: string;
  author: string;
  entryType: 'ALERT_FIRED' | 'COMMANDER_ASSIGNED' | 'STATUS_CHANGED' | 'MITIGATION_APPLIED' | 'NOTE' | 'CIRCUIT_BREAKER_TRIPPED' | 'CIRCUIT_BREAKER_RESET' | 'ESCALATED';
  message: string;
  metadata?: Record<string, unknown>;
}

export interface Incident {
  id: string;
  title: string;
  severity: SeverityLevel;
  status: IncidentStatus;
  source: string;
  contractAddress?: string;
  commander?: string;
  reporter: string;
  createdAt: string;
  acknowledgedAt?: string;
  mitigatedAt?: string;
  resolvedAt?: string;
  closedAt?: string;
  circuitBreakerActive: boolean;
  timeline: TimelineItem[];
  pagerDutyIncidentId?: string;
  opsgenieAlertId?: string;
  escalationPolicyId?: string;
  currentEscalationTier: number;
}

export interface OnCallUser {
  id: string;
  name: string;
  email: string;
  phone: string;
  timezone: string;
  role: 'PRIMARY' | 'SECONDARY' | 'SHADOW' | 'LEAD';
}

export interface OnCallShift {
  id: string;
  team: string;
  primary: OnCallUser;
  secondary: OnCallUser;
  startTime: string;
  endTime: string;
  rotationType: 'DAILY' | 'WEEKLY' | 'FOLLOW_THE_SUN';
}

export interface EscalationTierConfig {
  tier: number;
  delayMinutes: number;
  targets: Array<{
    type: 'USER' | 'SCHEDULE' | 'WEBHOOK' | 'TEAM';
    id: string;
    name: string;
  }>;
}

export interface EscalationPolicyConfig {
  id: string;
  name: string;
  description: string;
  tiers: EscalationTierConfig[];
  repeatCount: number;
}

export interface PostmortemActionItem {
  id: string;
  description: string;
  owner: string;
  dueDate: string;
  status: 'TODO' | 'IN_PROGRESS' | 'DONE';
  ticketUrl?: string;
}

export interface PostmortemReport {
  incidentId: string;
  title: string;
  severity: SeverityLevel;
  incidentCommander: string;
  leadInvestigator: string;
  date: string;
  durationMinutes: number;
  timeToAcknowledgeMinutes: number;
  timeToResolveMinutes: number;
  executiveSummary: string;
  impactAnalysis: {
    contractEventsDropped: number;
    financialImpactUsd: number;
    affectedContracts: string[];
    affectedSubsystems: string[];
  };
  rootCauseAnalysis: {
    primaryRootCause: string;
    contributingFactors: string[];
    fiveWhys: string[];
  };
  timelineSummary: TimelineItem[];
  actionItems: PostmortemActionItem[];
  lessonsLearned: {
    whatWentWell: string[];
    whatWentWrong: string[];
    whereWeGotLucky: string[];
  };
}
