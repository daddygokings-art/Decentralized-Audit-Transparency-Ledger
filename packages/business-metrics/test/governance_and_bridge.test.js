const test = require('node:test');
const assert = require('node:assert');
const { GovernanceMetricsCalculator } = require('../dist/calculators/governance.js');
const { BridgeThroughputMetricsCalculator } = require('../dist/calculators/bridge.js');
const { ApiAdoptionMetricsCalculator } = require('../dist/calculators/apiAdoption.js');

test('Governance, Bridge, and API Adoption Calculators', async (t) => {
  const now = Date.now();

  await t.test('calculates governance participation and dispute metrics', () => {
    const actions = [
      { id: '1', type: 'proposal_created', proposalId: 'prop-1', timestamp: now - 10000 },
      { id: '2', type: 'vote_cast', proposalId: 'prop-1', voter: 'addr1', weight: 50, quorumRequired: 100, timestamp: now - 8000 },
      { id: '3', type: 'vote_cast', proposalId: 'prop-1', voter: 'addr2', weight: 50, quorumRequired: 100, timestamp: now - 7000 },
      { id: '4', type: 'proposal_executed', proposalId: 'prop-1', latencyHours: 24, timestamp: now - 5000 },
      { id: '5', type: 'dispute_raised', timestamp: now - 3000 },
      { id: '6', type: 'dispute_resolved', timestamp: now - 1000 },
    ];

    const kpi = GovernanceMetricsCalculator.calculate(actions);

    assert.strictEqual(kpi.totalProposals, 1);
    assert.strictEqual(kpi.activeProposals, 0);
    assert.strictEqual(kpi.quorumAttainmentPct, 100);
    assert.strictEqual(kpi.avgExecutionLatencyHours, 24);
    assert.strictEqual(kpi.disputeResolutionRatePct, 100);
  });

  await t.test('calculates bridge volume and verification success rate', () => {
    const transfers = [
      { txHash: '0x1', sourceChain: 'stellar', targetChain: 'ethereum', timestamp: now - 10000, verifiedAt: now - 8000, amountUsd: 50000, gasCostUsd: 12.5, status: 'verified', cachedProof: true },
      { txHash: '0x2', sourceChain: 'stellar', targetChain: 'polygon', timestamp: now - 5000, verifiedAt: now - 4000, amountUsd: 25000, gasCostUsd: 0.5, status: 'verified', cachedProof: false },
    ];

    const kpi = BridgeThroughputMetricsCalculator.calculate(transfers);

    assert.strictEqual(kpi.totalRelayedEvents, 2);
    assert.strictEqual(kpi.volumeUsdTotal, 75000);
    assert.strictEqual(kpi.verificationSuccessRatePct, 100);
    assert.strictEqual(kpi.cacheHitRatePct, 50);
    assert.strictEqual(kpi.avgRelayLatencySeconds, 1.5);
  });

  await t.test('calculates API adoption and SLA compliance', () => {
    const calls = [
      { timestamp: now - 1000, endpoint: '/api/v1/events', protocol: 'rest', clientToken: 'tok_1', tier: 'enterprise', durationMs: 45, statusCode: 200 },
      { timestamp: now - 2000, endpoint: '/graphql', protocol: 'graphql', clientToken: 'tok_2', tier: 'pro', durationMs: 120, statusCode: 200 },
      { timestamp: now - 3000, endpoint: '/api/v1/events', protocol: 'rest', clientToken: 'tok_3', tier: 'free', durationMs: 350, statusCode: 200 },
    ];

    const kpi = ApiAdoptionMetricsCalculator.calculate(calls, 200, now);

    assert.strictEqual(kpi.totalApiCalls24h, 3);
    assert.strictEqual(kpi.activeDeveloperTokens, 3);
    assert.strictEqual(kpi.protocolBreakdown.rest, 2);
    assert.strictEqual(kpi.protocolBreakdown.graphql, 1);
    assert.strictEqual(kpi.slaCompliancePct, 66.67); // 2 out of 3 under 200ms
  });
});
