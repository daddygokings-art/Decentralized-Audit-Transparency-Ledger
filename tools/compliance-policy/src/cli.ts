#!/usr/bin/env node

import { Command } from 'commander';
import * as fs from 'fs';
import * as path from 'path';
import { ComplianceEvaluator } from './evaluator';
import { DriftDetector } from './drift';
import { AuditReporter } from './reporter';
import { PolicyEngine } from './engine';

const program = new Command();

program
  .name('compliance-policy')
  .description('AuditLedger Contract Event Compliance Automation & Drift Detection CLI')
  .version('1.0.0');

program
  .command('check')
  .description('Evaluate contract events against compliance policies')
  .option('-e, --events <file>', 'Path to contract events JSON file', 'policies/compliance/fixtures/sample-events.json')
  .option('-p, --policies <dir>', 'Path to Rego policies directory', 'policies/compliance')
  .action((opts) => {
    const eventsPath = path.resolve(opts.events);
    if (!fs.existsSync(eventsPath)) {
      console.error(`Events file not found: ${eventsPath}`);
      process.exit(1);
    }

    const events = JSON.parse(fs.readFileSync(eventsPath, 'utf8'));
    const evaluator = new ComplianceEvaluator(opts.policies);
    const { policyResults, allViolations, overallScore } = evaluator.evaluateEvents(events);

    console.log(`\n================ COMPLIANCE POLICY EVALUATION ================`);
    console.log(`Total Events Evaluated: ${events.length}`);
    console.log(`Compliance Score: ${overallScore}%`);
    console.log(`Total Violations: ${allViolations.length}`);
    console.log(`--------------------------------------------------------------`);

    for (const res of policyResults) {
      const status = res.compliant ? 'PASS' : 'FAIL';
      console.log(`[${status}] ${res.policy_package} (${res.violations.length} violations)`);
      for (const v of res.violations) {
        console.log(`  - [${v.severity}] ${v.rule_id} (${v.framework}): ${v.message}`);
      }
    }
    console.log(`==============================================================\n`);

    if (allViolations.some(v => v.severity === 'CRITICAL')) {
      process.exit(1);
    }
  });

program
  .command('drift')
  .description('Detect compliance drift against baseline snapshot')
  .option('-e, --events <file>', 'Path to contract events JSON file', 'policies/compliance/fixtures/sample-events.json')
  .option('-b, --baseline <file>', 'Path to baseline snapshot JSON file', 'policies/compliance/fixtures/baseline-snapshot.json')
  .action((opts) => {
    const eventsPath = path.resolve(opts.events);
    const baselinePath = path.resolve(opts.baseline);

    if (!fs.existsSync(eventsPath) || !fs.existsSync(baselinePath)) {
      console.error('Events or baseline file not found.');
      process.exit(1);
    }

    const events = JSON.parse(fs.readFileSync(eventsPath, 'utf8'));
    const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));

    const detector = new DriftDetector();
    const result = detector.detectDrift(events, baseline);

    console.log(`\n================ DRIFT DETECTION REPORT ================`);
    console.log(`Baseline ID: ${result.baseline_id}`);
    console.log(`Drift Detected: ${result.has_drift ? 'YES' : 'NO'}`);
    console.log(`Current Score: ${result.current_score_pct}% | Baseline: ${result.baseline_score_pct}%`);
    console.log(`Score Delta: ${result.score_delta}%`);
    console.log(`Total Findings: ${result.total_findings}`);

    if (result.findings.length > 0) {
      console.log(`\nFindings:`);
      for (const f of result.findings) {
        console.log(`  - [${f.severity}] ${f.drift_id} (${f.category}): ${f.message}`);
      }
    }
    console.log(`========================================================\n`);
  });

program
  .command('report')
  .description('Generate continuous compliance audit report')
  .option('-e, --events <file>', 'Path to contract events JSON file', 'policies/compliance/fixtures/sample-events.json')
  .option('-b, --baseline <file>', 'Path to baseline snapshot JSON file', 'policies/compliance/fixtures/baseline-snapshot.json')
  .option('-o, --output <dir>', 'Output directory for reports', 'docs/compliance/reports')
  .action((opts) => {
    const eventsPath = path.resolve(opts.events);
    const events = JSON.parse(fs.readFileSync(eventsPath, 'utf8'));
    const evaluator = new ComplianceEvaluator();
    const reporter = new AuditReporter();

    const report = evaluator.generateReportObject(events);

    if (opts.baseline && fs.existsSync(path.resolve(opts.baseline))) {
      const baseline = JSON.parse(fs.readFileSync(path.resolve(opts.baseline), 'utf8'));
      const detector = new DriftDetector();
      report.drift = detector.detectDrift(events, baseline);
    }

    const { jsonPath, mdPath } = reporter.saveReport(report, path.resolve(opts.output));
    console.log(`Audit report generated:`);
    console.log(`  JSON: ${jsonPath}`);
    console.log(`  MD:   ${mdPath}`);
  });

program
  .command('test-policies')
  .description('Run OPA unit tests for all Rego policy suites')
  .option('-p, --policies <dir>', 'Path to policies directory', 'policies/compliance')
  .action((opts) => {
    const engine = new PolicyEngine(opts.policies);
    const result = engine.testPolicies();
    console.log(result.output);
    if (!result.passed) {
      process.exit(1);
    }
  });

program.parse(process.argv);
