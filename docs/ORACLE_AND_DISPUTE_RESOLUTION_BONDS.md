# Oracle & Dispute Resolution: Bond Deposits, Challenge Windows, and Slashing Rules

## 1. Executive Summary

This document specifies the public economic security model governing Oracle submissions, prediction market resolutions, and decentralized dispute resolutions on the Decentralized Audit Transparency Ledger.

To ensure game-theoretic security against malicious reports and bribery attacks, the protocol enforces:
1. **Bond Deposit Requirements**: Mandatory collateral staked by submitters and challengers.
2. **Deterministic Challenge Windows**: Bounded time horizons during which proposed data can be audited and disputed.
3. **Automated Slashing Rules**: Deterministic penalties that strip dishonest actors of capital while rewarding honest verifiers and jurors.

---

## 2. Bond Deposit Requirements

All participants proposing oracle values or outcomes must lock economic collateral (bonds) prior to state acceptance.

### 2.1 Submitter Bond Scaling
Submitter bonds ensure that submitting false or erroneous data carries an immediate financial penalty:
- **Baseline Bond Requirement**: `1,000 AUDIT` (or equivalent in XLM/native tokens) for standard audit event resolutions.
- **Dynamic Liquidity Scaling**: For prediction markets or high-value contracts, the required bond scales linearly with the Total Value Locked (TVL) at risk:
  $$\text{Required Bond} = \max\left(\text{Base Bond}, \frac{\text{TVL} \times \text{Risk Factor}}{10,000}\right)$$
- **Escrow Mechanism**: Bonds are locked directly in the Soroban escrow vault at the moment of submission (`src/dao_dispute_resolution.rs`).
- **Release Conditions**: If no valid challenge is registered before the challenge window lapses, the bond is unlocked in full alongside any earned submission fee rewards.

### 2.2 Challenger Bond Matching
To deter frivolous griefing and denial-of-service disputes:
- **Equal Stake Rule**: A challenger must deposit a bond equal to `100%` of the submitter's initial bond.
- **Escalation / Multi-party Disputes**: Secondary challengers must match or exceed the cumulative bonded amount.
- **Escrow Custody**: Challenger bonds remain locked until juror vote finalization or appeal resolution.

---

## 3. Challenge Windows

The challenge window is the strictly enforced ledger time interval between data submission and protocol finalization.

### 3.1 Window Durations

| Market / Submission Type | Challenge Window (Ledgers) | Approximate Time | Rationale |
|--------------------------|---------------------------|------------------|-----------|
| Standard Audit Log       | 43,200 ledgers            | ~72 hours        | Adequate window for off-chain auditors |
| Prediction Market Event  | 100,800 ledgers           | ~7 days          | Multi-jurisdictional verification |
| High-Stakes Financial    | 201,600 ledgers           | ~14 days         | Multi-tier dispute and juror assignment |
| Emergency Expedited      | 14,400 ledgers            | ~24 hours        | Requires 3-of-5 governance multisig |

### 3.2 State Lifecycle Transitions

```
[ Submitted ] 
      |
      +---> (No challenge within Window) ------> [ Finalized / Settled ]
      |                                              (Bond Returned)
      |
      +---> (Challenge Filed + Bond Locked) ---> [ Under Dispute ]
                                                        |
                                                        v
                                                 [ Juror Voting ]
                                                        |
                                                        v
                                                 [ Final Outcome ]
                                              /                     \
                                   [ Submitter Upheld ]     [ Challenger Upheld ]
                                   (Challenger Slashed)      (Submitter Slashed)
```

1. **Submission Phase**: Oracle submits value along with `submitter_bond`. The countdown ledger timer begins.
2. **Challenge Window**: Any network participant may inspect evidence and submit a counter-claim with matching bond.
3. **Dispute Phase**: If contested, status transitions to `DisputeStatus::Voting`. Jurors are drawn and voting periods commence.
4. **Resolution & Settlement**: Once voting concludes or appeal deadlines expire, stakes are settled according to slashing rules.

---

## 4. Slashing Rules & Token Redistribution

When a dispute is finalized, the losing party's bond is slashed according to deterministic formulas.

### 4.1 Slashing Parameters

Configured via `DisputeConfig` (`src/dao_dispute_resolution.rs`):
- `slashing_bps`: 5,000 basis points (`50.00%`) baseline penalty; up to 10,000 basis points (`100%`) for proven fraudulent intent or sybil collusion.
- `juror_reward_bps`: 1,000 basis points (`10.00%`) of the slashed stake distributed pro-rata among jurors voting with the winning outcome.
- `challenger_reward_bps`: 4,000 basis points (`40.00%`) of the slashed stake awarded to the successful challenger.
- `burn_or_treasury_bps`: 5,000 basis points (`50.00%`) routed to the DAO Community Treasury for ecosystem security grants.

### 4.2 Slashing Scenarios

#### Scenario A: Malicious Submitter (Plaintiff / Challenger Wins)
- **Submitter Bond**: `10,000 AUDIT`
- **Result**: Submitter is slashed for 100% of bond.
  - Challenger receives refund (`10,000 AUDIT`) + Challenger Reward (`4,000 AUDIT`).
  - Active honest jurors receive pro-rata share of `1,000 AUDIT`.
  - DAO Treasury receives `5,000 AUDIT`.
- **Submitter Sanction**: Submitter reputation score decremented in `reputation_system.rs`.

#### Scenario B: Frivolous Challenge (Submitter Wins)
- **Challenger Bond**: `10,000 AUDIT`
- **Result**: Challenger is slashed for 100% of bond.
  - Submitter receives refund (`10,000 AUDIT`) + Inconvenience Bounty (`4,000 AUDIT`).
  - Jurors receive `1,000 AUDIT`.
  - Treasury receives `5,000 AUDIT`.

---

## 5. Governance Appeals

1. **Appeals Window**: Following initial juror determination, either party may file an appeal within `17,280 ledgers` (~28 hours).
2. **Appeal Bond Multiplier**: Filing an appeal requires depositing `2x` the original dispute stake amount.
3. **Escalated Juror Pool**: The appeal is reviewed by an expanded jury pool (minimum 15 jurors) or direct DAO token vote.
4. **Finality**: Appeal outcomes are final and non-reversible.
