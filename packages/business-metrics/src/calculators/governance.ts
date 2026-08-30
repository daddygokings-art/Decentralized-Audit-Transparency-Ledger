import { GovernanceActionRecord, GovernanceKPIs } from '../types';

export class GovernanceMetricsCalculator {
  public static calculate(records: GovernanceActionRecord[]): GovernanceKPIs {
    const proposals = new Set<string>();
    const executedProposals = new Set<string>();
    let totalVotes = 0;
    let totalQuorumRequired = 0;
    let totalVotesCast = 0;
    let totalLatencyHours = 0;
    let latencyCount = 0;

    let disputesInitiated = 0;
    let disputesResolved = 0;

    for (const r of records) {
      if (r.proposalId) {
        proposals.add(r.proposalId);
      }

      if (r.type === 'proposal_executed' && r.proposalId) {
        executedProposals.add(r.proposalId);
        if (r.latencyHours !== undefined) {
          totalLatencyHours += r.latencyHours;
          latencyCount++;
        }
      }

      if (r.type === 'vote_cast') {
        totalVotes++;
        if (r.weight) totalVotesCast += r.weight;
        if (r.quorumRequired) totalQuorumRequired = Math.max(totalQuorumRequired, r.quorumRequired);
      }

      if (r.type === 'dispute_raised') {
        disputesInitiated++;
      }

      if (r.type === 'dispute_resolved') {
        disputesResolved++;
      }
    }

    const totalProposals = proposals.size;
    const activeProposals = Math.max(0, totalProposals - executedProposals.size);
    const turnoutRatePct = totalProposals > 0 ? Number(((totalVotes / (totalProposals * 10)) * 100).toFixed(2)) : 0;
    const quorumAttainmentPct =
      totalQuorumRequired > 0
        ? Number((Math.min(100, (totalVotesCast / totalQuorumRequired) * 100)).toFixed(2))
        : 100.0;
    const avgExecutionLatencyHours = latencyCount > 0 ? Number((totalLatencyHours / latencyCount).toFixed(2)) : 0;
    const disputeResolutionRatePct =
      disputesInitiated > 0 ? Number(((disputesResolved / disputesInitiated) * 100).toFixed(2)) : 100.0;

    return {
      totalProposals,
      activeProposals,
      turnoutRatePct,
      quorumAttainmentPct,
      avgExecutionLatencyHours,
      disputesInitiated,
      disputesResolved,
      disputeResolutionRatePct,
    };
  }
}
