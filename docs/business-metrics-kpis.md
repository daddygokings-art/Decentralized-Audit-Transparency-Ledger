# Contract Event Business Metrics & KPIs Guide

This document defines the key performance indicators (KPIs), statistical algorithms, and executive reporting engine for the Decentralized Audit Transparency Ledger.

## Overview

The `@audit-ledger/business-metrics` engine aggregates and evaluates five core business dimensions:
1. **Active Submitters & Decentralization**
2. **Event Volume Growth & Anomaly Detection**
3. **Governance & DAO Actions**
4. **Cross-Chain Bridge Throughput**
5. **Developer & API Adoption**

---

## 1. Submitter Metrics & Centralization Index

- **DAU / WAU / MAU**: Distinct active submitting accounts within rolling 24-hour, 7-day, and 30-day windows.
- **DAU / MAU Stickiness Ratio**: Measures platform engagement frequency ($DAU / MAU$).
- **7-Day & 30-Day Submitter Retention**: Measures repeat activity across consecutive time cohorts.
- **Gini Coefficient of Submitter Concentration**:
  $$G = \frac{2 \sum_{i=1}^n i \cdot y_i}{n \sum_{i=1}^n y_i} - \frac{n+1}{n}$$
  Where $y_1 \le y_2 \le \dots \le y_n$ are ordered submission counts per submitter.
  - $G \approx 0$: Perfectly distributed submission workload.
  - $G > 0.75$: High centralization risk (alerting triggered).

---

## 2. Event Volume Growth & Anomaly Detection

- **DoD / WoW / MoM Growth Rates**: Day-over-Day, Week-over-Week, and Month-over-Month growth percentages.
- **Throughput Profiling**: Average and peak events per second (eps).
- **Z-Score Anomaly Detection**:
  $$Z = \frac{V_{\text{current}} - \mu}{\sigma}$$
  Where $\mu$ is the rolling mean and $\sigma$ is the standard deviation across historical days. $|Z| \ge 2.5$ flags a volume anomaly (either an unexpected traffic surge or outage drop).

---

## 3. Cross-Chain Bridge Throughput

- **Bridged Volume (USD)**: Aggregate nominal value of audit logs verified on destination chains.
- **Verification Success Rate**: Percentage of proofs accepted by EVM Verifier contract without reverts.
- **Proof Cache Hit Rate**: Percentage of proofs served from LRU cache without re-computing cryptographic preimages.

---

## 4. API Adoption & SLAs

- **SLA Compliance**: Percentage of requests completed within $\le 200\text{ms}$.
- **Active Developer Tokens**: Unique API keys active in 24h.
- **Error Rates**: 4xx client errors vs 5xx server errors.

---

## Executive Endpoints

- `GET /api/v1/kpis/overview` - Complete executive snapshot JSON.
- `GET /api/v1/kpis/report` - Formatted Markdown executive briefing.
- `GET /api/v1/kpis/metrics` - Prometheus metrics exposition.
