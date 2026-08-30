import { RunbookDefinition, ValidationReport } from '../types';

export class RunbookValidator {
  public static validate(def: RunbookDefinition, params: Record<string, unknown> = {}): ValidationReport {
    const errors: string[] = [];
    const warnings: string[] = [];

    if (!def.id || !def.name || !def.version) {
      errors.push('Runbook definition is missing required metadata (id, name, version).');
    }

    if (!def.steps || def.steps.length === 0) {
      errors.push('Runbook must define at least one operational step.');
    }

    const stepIds = new Set<number>();
    let totalTimeout = 0;

    for (let i = 0; i < def.steps.length; i++) {
      const step = def.steps[i];
      if (stepIds.has(step.id)) {
        errors.push(`Duplicate step ID detected: ${step.id}`);
      }
      stepIds.add(step.id);

      if (!step.name || !step.action) {
        errors.push(`Step at index ${i} missing name or action executor.`);
      }

      if (!step.isIdempotent) {
        warnings.push(`Step ${step.id} (${step.name}) is marked as non-idempotent; ensure safe rollback exists.`);
      }

      if (!step.rollback) {
        warnings.push(`Step ${step.id} (${step.name}) has no automated rollback procedure.`);
      }

      totalTimeout += step.timeoutSeconds || 30;
    }

    // Type-specific validations
    if (def.type === 'CAP_INCREASE') {
      const newCap = Number(params.newMaxLogs);
      if (isNaN(newCap) || newCap <= 0) {
        warnings.push('Cap increase parameter newMaxLogs is missing or invalid.');
      }
    } else if (def.type === 'SCHEMA_UPDATE') {
      const newVersion = Number(params.newVersion);
      if (isNaN(newVersion) || newVersion <= 0) {
        warnings.push('Schema update parameter newVersion is missing or invalid.');
      }
    } else if (def.type === 'BRIDGE_FAILOVER') {
      if (!params.newRelayerAddress) {
        warnings.push('Bridge failover parameter newRelayerAddress is not specified.');
      }
    }

    return {
      isValid: errors.length === 0,
      runbookId: def.id,
      errors,
      warnings,
      estimatedDurationSeconds: totalTimeout,
      precheckDetails: {
        totalSteps: def.steps.length,
        idempotentStepsCount: def.steps.filter((s) => s.isIdempotent).length,
        rollbackCoveragePercent: Math.round((def.steps.filter((s) => s.rollback).length / def.steps.length) * 100),
      },
    };
  }
}
