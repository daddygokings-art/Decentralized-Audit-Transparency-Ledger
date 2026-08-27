import {
  ExecutionContext,
  ExecutionSummary,
  RunbookDefinition,
  RunbookStepDefinition,
  StepResult,
  StepStatus,
} from '../types';

export class RunbookRunner {
  public static async execute(
    def: RunbookDefinition,
    options: {
      dryRun?: boolean;
      operator?: string;
      contractAddress?: string;
      params?: Record<string, unknown>;
    } = {}
  ): Promise<ExecutionSummary> {
    const startTime = new Date().toISOString();
    const ctx: ExecutionContext = {
      runbookId: def.id,
      dryRun: options.dryRun ?? false,
      operator: options.operator || 'system-ops-runner',
      contractAddress: options.contractAddress || 'CCAU_MOCK_CONTRACT_ADDRESS_01',
      params: options.params || {},
      logs: [],
      stepOutputs: new Map(),
    };

    ctx.logs.push(`[${startTime}] Starting runbook "${def.name}" (dryRun=${ctx.dryRun})`);

    const executedSteps: RunbookStepDefinition[] = [];
    const stepResults: Array<{ stepId: number; stepName: string; status: StepStatus; message: string }> = [];
    let isFailed = false;

    for (const step of def.steps) {
      ctx.logs.push(`[Step ${step.id}] Executing "${step.name}"...`);

      try {
        let res: StepResult;
        if (ctx.dryRun) {
          res = { success: true, message: `[DRY-RUN] Simulated step "${step.name}" successfully.` };
        } else {
          res = await Promise.race([
            step.action(ctx),
            new Promise<StepResult>((_, reject) =>
              setTimeout(() => reject(new Error(`Step timed out after ${step.timeoutSeconds}s`)), step.timeoutSeconds * 1000)
            ),
          ]);
        }

        ctx.stepOutputs.set(step.id, res);

        if (res.success) {
          stepResults.push({ stepId: step.id, stepName: step.name, status: 'PASSED', message: res.message });
          executedSteps.push(step);
        } else {
          isFailed = true;
          stepResults.push({
            stepId: step.id,
            stepName: step.name,
            status: 'FAILED',
            message: res.message || res.error || 'Step failed',
          });
          break;
        }
      } catch (err: unknown) {
        isFailed = true;
        const errorMsg = err instanceof Error ? err.message : String(err);
        stepResults.push({
          stepId: step.id,
          stepName: step.name,
          status: 'FAILED',
          message: errorMsg,
        });
        break;
      }
    }

    let finalStatus: ExecutionSummary['status'] = 'COMPLETED';

    if (isFailed) {
      ctx.logs.push(`[Failure] Triggering automated rollback for ${executedSteps.length} executed steps...`);
      finalStatus = 'ROLLED_BACK';

      for (const step of executedSteps.reverse()) {
        if (step.rollback) {
          try {
            ctx.logs.push(`[Rollback Step ${step.id}] Rolling back "${step.name}"...`);
            if (!ctx.dryRun) {
              await step.rollback(ctx);
            }
            const existing = stepResults.find((s) => s.stepId === step.id);
            if (existing) existing.status = 'ROLLED_BACK';
          } catch (rbErr: unknown) {
            const rbMsg = rbErr instanceof Error ? rbErr.message : String(rbErr);
            ctx.logs.push(`[Rollback Error] Failed to rollback step ${step.id}: ${rbMsg}`);
          }
        }
      }
    }

    const endTime = new Date().toISOString();
    ctx.logs.push(`[${endTime}] Runbook execution finished with status: ${finalStatus}`);

    return {
      runbookId: def.id,
      runbookName: def.name,
      status: finalStatus,
      dryRun: ctx.dryRun,
      operator: ctx.operator,
      startTime,
      endTime,
      totalSteps: def.steps.length,
      passedSteps: stepResults.filter((s) => s.status === 'PASSED').length,
      stepResults,
    };
  }
}
