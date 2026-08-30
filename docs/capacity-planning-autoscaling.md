# Predictive Capacity Planning & Auto-Scaling System

This document describes the predictive capacity planning architecture, Holt-Winters ML time-series forecasting model, Kubernetes HPA/VPA integration, resource quotas, and cloud cost optimization for the Decentralized Audit Transparency Ledger.

---

## 1. Architecture Overview

```
┌──────────────────────────────┐
│  Stellar Soroban Blockchain  │
│  & Bridge Relayers (Pods)    │
└──────────────┬───────────────┘
               │ Prometheus Telemetry (TPS, CPU, Mem, Queue)
               ▼
┌──────────────────────────────┐
│  Capacity Planning Daemon    │
│  • Holt-Winters Forecaster   │
│  • Auto-scaling Evaluator    │
│  • Cost Rightsizing Analyzer │
└──────────────┬───────────────┘
               │ Exposes Custom Metrics
               ▼
┌──────────────────────────────┐
│  K8s Custom Metrics Adapter  ├────────────────────────┐
└──────────────┬───────────────┘                        │
               │                                        │
               ▼                                        ▼
┌──────────────────────────────┐        ┌──────────────────────────────┐
│  Horizontal Pod Autoscaler   │        │   Cluster Autoscaler (CA)    │
│  (HPA v2 - Predictive TPS)   │        │   (Node Group Expansion)     │
└──────────────┬───────────────┘        └──────────────────────────────┘
               │
               ▼
┌──────────────────────────────┐
│   Scaled Relayer Deployments │
└──────────────────────────────┘
```

---

## 2. ML-Based Predictive Forecasting

The `MLForecaster` engine uses Holt-Winters Triple Exponential Smoothing to model:
- **Level ($\alpha = 0.35$)**: Base transaction arrival rate.
- **Trend ($\beta = 0.15$)**: Growth acceleration or decline rate.
- **Seasonality ($\gamma = 0.25$)**: 24-hour diurnal usage patterns.

### Forecast Formula:
$$\hat{y}_{t+h} = (\ell_t + h b_t) \cdot s_{t+h-m}$$
with $95\%$ confidence bounds:
$$\text{Upper Bound} = \hat{y}_{t+h} + 1.96 \cdot \sigma \sqrt{h}$$

By looking ahead $15$ to $30$ minutes, Kubernetes scales up pods before traffic peaks hit, eliminating cold-start throttling.

---

## 3. Kubernetes Autoscaling Architecture

1. **Horizontal Pod Autoscaling (HPA v2)**:
   - Configured in `infra/k8s/scaling/hpa-relayer-predictive.yaml`.
   - Scale-out target: $40$ TPS per replica or $70\%$ CPU utilization.
   - Behavior: Immediate scale-up ($0\text{s}$ stabilization), graceful scale-down ($300\text{s}$ cooldown).

2. **Vertical Pod Autoscaling (VPA)**:
   - Configured in `infra/k8s/scaling/vpa-api-relayer.yaml`.
   - Recommends optimal CPU ($100\text{m} - 2000\text{m}$) and Memory ($128\text{Mi} - 2048\text{Mi}$) requests based on empirical utilization.

3. **Cluster Autoscaler & Spot Optimization**:
   - Priority Expander gives preference to EC2 Spot / GCP Preemptible node pools for relayer workers.
   - PodDisruptionBudgets (`infra/k8s/scaling/pod-disruption-budgets.yaml`) ensure high availability during node drainage.

---

## 4. On-Chain Quotas & Telemetry (`src/capacity_planning.rs`)

- **Telemetry Ledger**: On-chain circular buffer storing resource utilization snapshots.
- **Multi-Tenant Quota Tiers**: Defines maximum daily events, burst TPS, and storage allowances per submitter.
- **Billing Accounting**: Cost tracking per million events billed to submitters.
