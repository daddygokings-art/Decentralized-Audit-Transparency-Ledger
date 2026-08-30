export type RunbookType = 'CONTRACT_PAUSE' | 'CAP_INCREASE' | 'SCHEMA_UPDATE' | 'BRIDGE_FAILOVER' | 'CUSTOM';

export type StepStatus = 'PENDING' | 'RUNNING' | 'PASSED' | 'FAILED' | 'SKIPPED' | 'ROLLED_BACK';

export interface RunbookStepDefinition {
  id: number;
  name: string;
  description: string;
  action: (ctx: ExecutionContext) => Promise<StepResult>;
  rollback?: (ctx: ExecutionContext) => Promise<StepResult>;
  isIdempotent: boolean;
  timeoutSeconds: number;
  requiresApproval?: boolean;
}

export interface StepResult {
  success: boolean;
  message: string;
  output?: Record<string, unknown>;
  error?: string;
}

export interface RunbookDefinition {
  id: string;
  name: string;
  version: string;
  type: RunbookType;
  description: string;
  author: string;
  steps: RunbookStepDefinition[];
}

export interface ExecutionContext {
  runbookId: string;
  dryRun: boolean;
  operator: string;
  contractAddress: string;
  params: Record<string, unknown>;
  logs: string[];
  stepOutputs: Map<number, StepResult>;
}

export interface ValidationReport {
  isValid: boolean;
  runbookId: string;
  errors: string[];
  warnings: string[];
  estimatedDurationSeconds: number;
  precheckDetails: Record<string, unknown>;
}

export interface ExecutionSummary {
  runbookId: string;
  runbookName: string;
  status: 'COMPLETED' | 'FAILED' | 'ROLLED_BACK';
  dryRun: boolean;
  operator: string;
  startTime: string;
  endTime: string;
  totalSteps: number;
  passedSteps: number;
  stepResults: Array<{ stepId: number; stepName: string; status: StepStatus; message: string }>;
}
