# Modern Slavery Act Transparency & Audit Trail

## Overview

The AuditLedger modern slavery transparency module provides immutable, on-chain logging and off-chain analytics for organizations to demonstrate compliance with:

- **UK Modern Slavery Act 2015** — requires organisations with a UK turnover of £36M+ to publish a modern slavery statement annually
- **Australian Modern Slavery Act 2018** — requires reporting entities to publish a modern slavery statement

This module tracks:

1. **Risk Assessments** — periodic evaluations of modern slavery risks across the organisation
2. **Supply Chain Mapping** — registry of suppliers and partners with risk classification
3. **Training & Awareness** — documentation of personnel trained on MSA obligations
4. **Due Diligence** — investigation findings and corrective action tracking
5. **Policies** — modern slavery prevention policies and governance
6. **Compliance Reports** — aggregated snapshots for annual MSA statements

All records are immutably stored on the Stellar blockchain via the AuditLedger Soroban contract, making them publicly verifiable and tamper-evident.

---

## Core Concepts

### Risk Assessment

A periodic evaluation of modern slavery risks within the organisation's operations and supply chain.

**Data Points:**
- `assessment_id` — unique period identifier (e.g. `"2026_q1_assessment"`)
- `scope` — geographic or operational scope (e.g. `"global"`, `"apac"`)
- `risk_level` — 0 (low), 1 (medium), 2 (high), 3 (critical)
- `high_risk_areas` — count of identified risk concentrations
- `key_risks` — brief summary of material risks
- `planned_remediations` — number of corrective actions
- `stakeholder_consultation_done` — whether consultation occurred

**Compliance Framework References:**
- UK MSA §54 — guidance emphasizes board-level understanding of risks
- Australian MSA §16 — requires due diligence processes and risk assessments

### Supply Chain Mapping

A registry of suppliers, manufacturers, and strategic partners, classified by risk.

**Data Points:**
- `supplier_id` — unique identifier
- `name` — organization name
- `country` — country code
- `risk_level` — 0–3 classification based on due diligence
- `audited` — whether audited by the organisation
- `last_audit_date` — Unix timestamp of most recent audit

**Context:**
- Organizations must know their supply chain and identify high-risk areas
- Risk factors include: geography, sector, production practices, labor practices
- Regular audits reduce risk and demonstrate due diligence

### Training & Awareness

Records of personnel training on modern slavery risks, due diligence procedures, and reporting obligations.

**Data Points:**
- `training_id` — unique session identifier
- `topic` — training type (e.g. `"msa_awareness"`, `"due_diligence"`, `"reporting"`)
- `attendees` — number of personnel trained
- `risk_assessment_covered` — true if risk assessment methodology was included
- `due_diligence_covered` — true if due diligence procedures were covered
- `reporting_covered` — true if reporting obligations were addressed

**Compliance Requirements:**
- UK MSA: Organizations must ensure relevant staff understand MSA obligations
- Australian MSA: Training demonstrates due diligence in identifying and addressing risks

### Due Diligence Investigation

Records of formal investigations into supplier practices, labor conditions, or specific allegations.

**Data Points:**
- `record_id` — unique investigation identifier
- `subject` — supplier or facility investigated
- `scope` — investigation focus (e.g. `"labour_practices"`, `"child_labor"`, `"forced_labour"`)
- `findings` — summary of findings
- `risk_level` — 0 (no issues), 1 (low), 2 (medium), 3 (high), 4 (critical)
- `corrective_actions_required` — number of actions identified
- `corrective_actions_completed_pct` — completion percentage (0–100)

**Tracking Remediation:**
- All corrective actions must be tracked from identification through completion
- Delays or incomplete remediation are documented, enabling accountability
- Re-audits verify effectiveness of corrective actions

### Modern Slavery Prevention Policy

Records of organisational policies addressing modern slavery risks.

**Data Points:**
- `policy_id` — unique policy identifier
- `version` — policy version number
- `scope` — scope of application (e.g. `"global"`, `"supply_chain_only"`)
- `content_summary` — brief summary of policy content
- `stakeholder_input_included` — whether stakeholders participated in development
- `adopted_at` — adoption date
- `last_updated_at` — last revision date

**MSA Compliance:**
- UK MSA §54(4) — organizations must publish a slavery and human trafficking statement describing policies
- Australian MSA §16(1)(d) — statement must describe consultation on modern slavery risks

---

## Risk Assessment Methodology

### Risk Scoring Algorithm

The on-chain contract computes a **weighted risk score** considering:

1. **Maximum risk level** — highest risk classification found (0–3 scale)
   - Scaled to 0–7.5 points
2. **High-risk area concentration** — number and distribution of identified risks
   - Contributes up to 2.5 points
3. **Stakeholder consultation discount** — organizations conducting stakeholder consultation receive a score reduction
   - Up to 30% reduction if 70%+ of assessments included consultation

**Final Score:** 0–10 scale where:
- **0–2** = Low risk (adequate controls in place)
- **2–5** = Medium risk (controls exist but gaps identified)
- **5–7.5** = High risk (significant gaps, urgent remediation needed)
- **7.5–10** = Critical risk (immediate intervention required)

### Risk Factors

Organizations should evaluate:

- **Geographic concentration** — heavy reliance on high-risk countries (e.g., parts of South/Southeast Asia)
- **Industry sector** — sectors with known modern slavery issues (textiles, agriculture, construction, hospitality)
- **Commodity types** — high-risk materials (e.g. cobalt, cocoa, timber)
- **Labor intensity** — high reliance on manual labor
- **Regulatory environment** — weak labor enforcement in supplier locations
- **Supplier relationships** — duration, oversight, and audit frequency
- **Subcontracting practices** — uncontrolled subcontracting increases risk

---

## Supply Chain Mapping

### Mapping Process

1. **Inventory** — identify all direct suppliers and significant subcontractors
2. **Categorize** — classify by:
   - Production function (raw material, component, assembly, logistics)
   - Geography and sector
   - Volume and criticality to operations
3. **Risk classify** — assign risk level based on due diligence findings:
   - **0 (Low)**: Low-risk country, sector, and audit history; strong controls
   - **1 (Medium)**: Mixed indicators; reasonable controls; recent audit
   - **2 (High)**: High-risk country/sector or weak audit findings; remediation in progress
   - **3 (Critical)**: Multiple risk factors; significant findings; urgent remediation needed
4. **Audit schedule** — plan frequency based on risk:
   - Critical (3): Quarterly re-audits or continuous monitoring
   - High (2): Annual audit + re-audit after remediation
   - Medium (1): Every 2–3 years or following complaints
   - Low (0): Every 3–5 years or as part of routine supplier review
5. **Monitor** — track remediation actions and audit results on-chain

### Data Integrity

All supply chain nodes are recorded on-chain with:
- Audit date and findings
- Risk classification
- Last update timestamp

This creates a tamper-evident record of the organization's knowledge and due diligence over time.

---

## Training Effectiveness

### Training Topics

Organizations should ensure personnel training covers:

1. **Modern Slavery Awareness**
   - What constitutes modern slavery (forced labor, debt bondage, human trafficking, child labor)
   - Red flags and warning signs
   - Reporting mechanisms

2. **Due Diligence Procedures**
   - How to assess supplier practices
   - On-site audit procedures
   - Documentation and evidence gathering
   - Corrective action negotiation

3. **Reporting Obligations**
   - MSA statement requirements
   - Confidential hotline and whistleblower protections
   - Escalation procedures
   - External stakeholder communication

### Effectiveness Metrics

The SDK calculates:

- **Training reach** — total personnel trained as % of organization size
- **Content coverage** — % of sessions addressing each topic
- **Attendance patterns** — average session size (indicates engagement)
- **Repeat training** — frequency of refresher sessions

Organizations should aim for:
- ≥ 80% of relevant staff trained annually
- 100% of procurement, supplier management, and audit staff trained
- Annual refresher training for all roles

---

## Corrective Action Tracking

### Lifecycle

1. **Identification** — investigation findings specify corrective actions
2. **Planning** — organization negotiates implementation timeline with supplier
3. **Execution** — supplier implements changes and provides evidence
4. **Verification** — organization or third party re-audits to verify
5. **Closure** — actions marked as complete when verified effective

### On-Chain Record

Each due diligence investigation includes:

- `corrective_actions_required` — initial count
- `corrective_actions_completed_pct` — progress (0–100%)
- Timestamps of findings and updates

Off-chain, the analyzer tracks:

- **Fully completed** — 100% done, remediation verified
- **Partially completed** — 1–99% progress
- **Not started** — 0% progress

Organizations can track:

```
remediation_summary = analyzer.remediation_summary()
# {
#   "total_actions_identified": 47,
#   "fully_completed": 38,
#   "partially_completed": 7,
#   "not_started": 2,
#   "avg_completion_pct": 91.5,
# }
```

---

## Compliance Reporting

### Annual Statement Structure

The MSAReport aggregates all on-chain data to support the organization's annual statement:

```json
{
  "reporting_period": "FY 2026",
  "generated_at": 1_700_000_000,
  "governance": {
    "board_responsibility": true,
    "policies_in_place": 3,
    "stakeholder_consultation_included": true
  },
  "risk": {
    "assessments_completed": 4,
    "max_risk_level": 2,
    "total_high_risk_areas": 12
  },
  "supply_chain": {
    "total_suppliers": 847,
    "suppliers_audited": 623,
    "audit_rate_pct": 73.5,
    "high_risk_suppliers": 45
  },
  "training": {
    "personnel_trained": 2150,
    "training_sessions": 18,
    "coverage": {
      "risk_assessment": 100.0,
      "due_diligence": 95.0,
      "reporting": 100.0
    }
  },
  "due_diligence": {
    "investigations_completed": 28,
    "total_corrective_actions": 87,
    "remediation_completion_pct": 89
  }
}
```

### Statement Publication

1. Generate the on-chain MSAReport via `build_msa_report()`
2. Export data to organizational compliance system
3. Draft narrative statement describing:
   - Governance structure
   - Risk identification and assessment approach
   - Due diligence and monitoring processes
   - Remediation actions taken
   - Training and awareness initiatives
   - Stakeholder engagement
4. Publish statement on organizational website
5. Certify statement via board or designated officer

---

## API Reference

### Soroban Contract Functions

#### Risk Assessment

```rust
// Record a risk assessment
pub fn record_risk_assessment(env: Env, caller: Address, assessment: RiskAssessment) -> u32;

// Retrieve a specific assessment
pub fn get_risk_assessment(env: Env, assessment_id: Symbol) -> RiskAssessment;

// Get total count
pub fn msa_risk_assessment_count(env: Env) -> u32;
```

#### Supply Chain

```rust
// Record a supplier / partner
pub fn record_supply_chain_node(env: Env, caller: Address, node: SupplyChainNode) -> u32;

// Retrieve supplier details
pub fn get_supply_chain_node(env: Env, supplier_id: Symbol) -> SupplyChainNode;

// Get total node count
pub fn msa_supply_chain_node_count(env: Env) -> u32;
```

#### Training

```rust
// Record a training session
pub fn record_training(env: Env, caller: Address, training: TrainingRecord) -> u32;

// Retrieve training record
pub fn get_training_record(env: Env, training_id: Symbol) -> TrainingRecord;

// Get total training sessions
pub fn msa_training_record_count(env: Env) -> u32;
```

#### Due Diligence

```rust
// Submit due diligence investigation findings
pub fn submit_due_diligence(env: Env, caller: Address, record: DueDiligenceRecord) -> u32;

// Retrieve investigation record
pub fn get_due_diligence_record(env: Env, record_id: Symbol) -> DueDiligenceRecord;

// Get total investigations
pub fn msa_due_diligence_count(env: Env) -> u32;
```

#### Policy

```rust
// Record an MSA prevention policy
pub fn record_msa_policy(env: Env, caller: Address, policy: MSAPolicy) -> u32;

// Retrieve policy details
pub fn get_msa_policy(env: Env, policy_id: Symbol) -> MSAPolicy;

// Get total policies
pub fn msa_policy_count(env: Env) -> u32;
```

#### Reporting

```rust
// Generate aggregated compliance report
pub fn build_msa_report(env: Env, caller: Address) -> MSAReport;

// Retrieve the latest report
pub fn get_msa_report(env: Env) -> Option<MSAReport>;
```

### Python SDK

#### Off-Chain Analytics

```python
from audit_ledger.modern_slavery import (
    RiskAssessment,
    SupplyChainNode,
    TrainingRecord,
    DueDiligenceRecord,
    MSAPolicy,
    ModernSlaveryAnalyzer,
    calculate_risk_score,
    supply_chain_risk_summary,
    training_effectiveness,
    remediation_progress,
    build_compliance_report,
)

# Create analyzer from contract data
analyzer = ModernSlaveryAnalyzer(
    assessments=[...],
    nodes=[...],
    trainings=[...],
    due_diligence=[...],
    policies=[...]
)

# Calculate metrics
risk_score = analyzer.risk_score()  # 0–10
sc_summary = analyzer.supply_chain_summary()
training = analyzer.training_summary()
remediation = analyzer.remediation_summary()

# Generate full report
report = analyzer.compliance_report(generated_at=1_700_000_000)
print(report.to_json())  # export for statement publication
```

---

## Best Practices

1. **Automate data collection** — integrate contract event streams into compliance workflows
2. **Regular audits** — schedule supply chain audits based on risk classification
3. **Stakeholder engagement** — involve workers, unions, community organizations in policy development
4. **Transparent remediation** — publish corrective action plans and progress updates
5. **Board oversight** — ensure governance structure includes board-level responsibility
6. **Third-party verification** — engage external auditors to validate assessment methodology
7. **Continuous improvement** — annually review MSA program effectiveness and update policies
8. **Multi-year trend analysis** — use on-chain historical data to identify patterns and gaps
9. **Incident response** — establish protocols for investigating and remediating allegations
10. **Whistleblower protection** — implement confidential reporting mechanisms and retaliation safeguards

---

## Regulatory References

- **UK Modern Slavery Act 2015** — https://www.legislation.gov.uk/ukpga/2015/30/contents
- **UK Statutory Guidance** — https://www.gov.uk/government/publications/slavery-and-human-trafficking-transparent-supply-chains-etc-draft-guidance
- **Australian Modern Slavery Act 2018** — https://www.legislation.gov.au/C2018A00153/latest/text
- **UNGP Guiding Principles on Business and Human Rights** — https://www.ohchr.org/sites/default/files/Documents/Publications/GuidingPrinciples_EN.pdf
- **SROI Network** — https://www.sroinetwork.org

---

## Examples

### Recording a Quarterly Risk Assessment

```rust
let assessment = RiskAssessment {
    assessment_id: symbol_short!("2026_q1"),
    recorded_at: env.ledger().timestamp(),
    submitter: organization_address,
    scope: symbol_short!("global"),
    risk_level: 1,
    high_risk_areas: 3,
    key_risks: Bytes::from_slice(&env, b"supply chain concentration in Asia"),
    planned_remediations: 2,
    stakeholder_consultation_done: true,
};

client.record_risk_assessment(&owner, &assessment);
```

### Adding a Supplier to the Registry

```rust
let node = SupplyChainNode {
    supplier_id: symbol_short!("supp_vn_001"),
    name: Bytes::from_slice(&env, b"Vietnam Textiles Ltd"),
    country: symbol_short!("VN"),
    risk_level: 2,
    audited: true,
    last_audit_date: env.ledger().timestamp(),
    registered_at: env.ledger().timestamp(),
};

client.record_supply_chain_node(&owner, &node);
```

### Tracking Training Effectiveness

```python
trainings = [
    TrainingRecord(
        training_id="train_2026_q1_001",
        attendees=200,
        risk_assessment_covered=True,
        due_diligence_covered=True,
        reporting_covered=True,
    ),
    TrainingRecord(
        training_id="train_2026_q1_002",
        attendees=150,
        risk_assessment_covered=False,
        due_diligence_covered=True,
        reporting_covered=True,
    ),
]

analyzer = ModernSlaveryAnalyzer(trainings=trainings)
summary = analyzer.training_summary()
# {
#   "total_personnel_trained": 350,
#   "total_sessions": 2,
#   "risk_assessment_covered_pct": 50.0,
#   "due_diligence_covered_pct": 100.0,
#   "reporting_covered_pct": 100.0,
# }
```

### Publishing an Annual Statement

```python
# Fetch all on-chain data (via RPC calls)
assessments = [...]
nodes = [...]
trainings = [...]
due_diligence = [...]
policies = [...]

# Generate report
analyzer = ModernSlaveryAnalyzer(
    assessments, nodes, trainings, due_diligence, policies
)
report = analyzer.compliance_report(generated_at=1_700_000_000)

# Export to JSON for publication
statement_json = report.to_json()
# Manually craft narrative and publish on organizational website
```

---

## License

[MIT](../LICENSE)
