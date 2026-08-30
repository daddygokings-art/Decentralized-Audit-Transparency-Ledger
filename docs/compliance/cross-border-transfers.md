# Cross-Border Transfer Compliance

Every transfer of ledger data outside its originating jurisdiction requires a recorded transfer impact assessment (TIA) before release. The REST API exposes this workflow at `POST /v1/compliance/transfers/assess`.

## Required assessment

An assessment records the destination, data categories, reviewer, risk rating, and one approved transfer mechanism:

- `adequacy`: an applicable adequacy decision is identified and remains current.
- `scc`: the current standard contractual clauses are executed by the parties.
- `bcr`: approved binding corporate rules cover the recipient and processing.
- `certification`: a valid, scoped certification covers the transfer.

High-risk assessments must document supplementary measures such as encryption with customer-held keys, pseudonymisation, access minimisation, and transparency controls. Assessments without supplementary measures remain `review_required` until a privacy reviewer approves them.

The OPA rule set in `policies/opa/compliance.rego` is the merge gate for these requirements. Assessment records should be retained with the compliance evidence artifact and linked to the corresponding contract event or export hash.
