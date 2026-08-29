/**
 * User Analytics and Product Insights Types
 * GDPR / CCPA compliant with consent management and pseudonymization.
 */

export type ConsentCategory = 'necessary' | 'analytics' | 'performance' | 'marketing';

export interface ConsentPreferences {
  anonymousId: string;
  optedIn: boolean;
  categories: ConsentCategory[];
  dntHeaderHonored: boolean;
  updatedAt: string;
}

export interface UserBehaviorEvent {
  eventId: string;
  anonymousId: string;
  sessionId: string;
  eventName: string;
  timestamp: number;
  properties: Record<string, any>;
  context?: {
    userAgent?: string;
    locale?: string;
    network?: string;
    referrer?: string;
    ipHash?: string;
  };
}

export interface FunnelStep {
  step: string;
  eventName: string;
  requiredProperties?: Record<string, any>;
}

export interface FunnelDefinition {
  id: string;
  name: string;
  steps: FunnelStep[];
  maxConversionWindowHours?: number;
}

export interface FunnelStepResult {
  step: string;
  eventName: string;
  usersEntered: number;
  usersCompleted: number;
  conversionRatePct: number;
  dropoffPct: number;
  avgTimeToConvertSec: number;
}

export interface FunnelAnalysisResult {
  funnelId: string;
  funnelName: string;
  totalUsersEntered: number;
  totalUsersCompleted: number;
  overallConversionRatePct: number;
  stepResults: FunnelStepResult[];
  biggestDropoffStep: string;
}

export interface FeatureAdoptionMetric {
  featureName: string;
  totalEvents: number;
  uniqueUsers: number;
  adoptionRatePct: number;
  dau: number;
  mau: number;
  stickinessDauToMau: number;
  powerUsers: number; // users with > 10 interactions
  avgTimeToFirstUseHours: number;
}

export interface CohortInterval {
  intervalNumber: number;
  label: string;
  activeUsers: number;
  retentionRatePct: number;
}

export interface CohortRetentionResult {
  cohortId: string;
  periodType: 'daily' | 'weekly' | 'monthly';
  cohortSize: number;
  intervals: CohortInterval[];
  churnRatePct: number;
}
