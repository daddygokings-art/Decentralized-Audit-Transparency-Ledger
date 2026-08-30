# Contract Event Feature Flags & Progressive Delivery Guide

This guide covers configuring feature flags, managing progressive canary deployments, running A/B experiments, and operating emergency kill switches across the `AuditLedger` platform.

---

## 🚀 Key Capabilities

- **Progressive Canary Rollouts**: Stepwise ramp-up (e.g. 10% → 25% → 50% → 100%) with automated health checks.
- **LaunchDarkly & OpenFeature Compatibility**: Standardized provider adapters for cloud flag management.
- **Emergency Kill Switches**: Instantaneous shutoff for problematic event types or features.
- **Multivariate Experimentation**: Deterministic user bucketing and variant evaluation.

---

## 🛠️ CLI Operations

Manage flags using the unified CLI tool:

```bash
# 1. Create a new flag with canary config
./scripts/feature-flags/manage-flags.sh create enable_zk_proof_events \
  --type percentage_rollout \
  --canary 25

# 2. Advance canary rollout stage (+25%)
./scripts/feature-flags/manage-flags.sh advance-canary enable_zk_proof_events

# 3. Monitor canary deployment health
./scripts/feature-flags/canary-monitor.sh enable_zk_proof_events 50

# 4. Trigger emergency kill switch
./scripts/feature-flags/manage-flags.sh kill enable_zk_proof_events \
  --reason "High memory consumption detected in event parser"

# 5. Reset kill switch after fix is deployed
./scripts/feature-flags/manage-flags.sh reset-kill enable_zk_proof_events
```

---

## 📊 Observability & Dashboards

- **Grafana Dashboard**: Import `monitoring/grafana/dashboards/feature-flags-canary.json` to monitor canary traffic splits, error rate differentials, and active kill switches.
- **Prometheus Alerts**: Configured in `infra/k8s/monitoring/feature-flags-alerts.yaml` to alert on canary SLA breaches and kill switch activations.
