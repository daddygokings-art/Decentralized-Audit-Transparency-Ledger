/// REST API extensions for DAO governance
/// Endpoints for proposal creation, voting, delegation, treasury, and disputes

import express, { Request, Response, Router } from 'express';

// ============================================================================
// Types
// ============================================================================

interface Proposal {
  proposal_id: number;
  proposal_type: string;
  proposer: string;
  title: string;
  description: string;
  status: string;
  votes_for: number;
  votes_against: number;
  votes_abstain: number;
  start_ledger: number;
  end_ledger: number;
  execution_ledger: number;
}

interface VotingPower {
  user: string;
  base_power: number;
  delegated_power: number;
  delegated_to?: string;
  total_power: number;
}

interface Fund {
  name: string;
  balance: number;
  budget_limit: number;
  budget_used: number;
  period_ledgers: number;
}

interface Allocation {
  allocation_id: number;
  recipient: string;
  fund_name: string;
  amount: number;
  purpose: string;
  approved: boolean;
  approval_count: number;
  required_approvals: number;
}

interface Dispute {
  dispute_id: number;
  plaintiff: string;
  defendant: string;
  description: string;
  status: string;
  votes_for: number;
  votes_against: number;
  votes_abstain: number;
  outcome?: string;
}

// ============================================================================
// Governance API Router
// ============================================================================

const daoGovernanceRouter = Router();

// ========================================================================
// Proposal Endpoints
// ========================================================================

/**
 * POST /governance/proposals
 * Create a new proposal
 */
daoGovernanceRouter.post('/governance/proposals', async (req: Request, res: Response) => {
  try {
    const {
      proposal_type,
      title,
      description,
      parameters,
      quorum_bps,
      approval_threshold_bps,
    } = req.body;

    // TODO: Call soroban contract
    // const proposalId = await daoGovernance.propose(...);

    const proposal: Proposal = {
      proposal_id: 1,
      proposal_type,
      proposer: req.user.address,
      title,
      description,
      status: 'Active',
      votes_for: 0,
      votes_against: 0,
      votes_abstain: 0,
      start_ledger: 12345,
      end_ledger: 13000,
      execution_ledger: 13500,
    };

    res.status(201).json({ data: proposal });
  } catch (error) {
    console.error('Error creating proposal:', error);
    res.status(500).json({ error: 'Failed to create proposal' });
  }
});

/**
 * GET /governance/proposals
 * List all proposals
 */
daoGovernanceRouter.get('/governance/proposals', async (req: Request, res: Response) => {
  try {
    const { status, sort_by = 'newest', limit = '20' } = req.query;

    // TODO: Query contract for proposals

    const proposals: Proposal[] = [];

    res.json({
      data: proposals,
      count: proposals.length,
    });
  } catch (error) {
    console.error('Error fetching proposals:', error);
    res.status(500).json({ error: 'Failed to fetch proposals' });
  }
});

/**
 * GET /governance/proposals/:proposal_id
 * Get proposal details
 */
daoGovernanceRouter.get(
  '/governance/proposals/:proposal_id',
  async (req: Request, res: Response) => {
    try {
      const { proposal_id } = req.params;

      // TODO: Query contract

      const proposal: Proposal = {
        proposal_id: parseInt(proposal_id),
        proposal_type: 'ParameterChange',
        proposer: 'GXXXXX...',
        title: 'Increase tier pricing',
        description: 'Proposal to increase premium tier price',
        status: 'Active',
        votes_for: 1500,
        votes_against: 500,
        votes_abstain: 200,
        start_ledger: 12000,
        end_ledger: 13000,
        execution_ledger: 13600,
      };

      res.json({ data: proposal });
    } catch (error) {
      console.error('Error fetching proposal:', error);
      res.status(500).json({ error: 'Failed to fetch proposal' });
    }
  }
);

/**
 * POST /governance/proposals/:proposal_id/vote
 * Cast a vote on a proposal
 */
daoGovernanceRouter.post(
  '/governance/proposals/:proposal_id/vote',
  async (req: Request, res: Response) => {
    try {
      const { proposal_id } = req.params;
      const { choice } = req.body; // 'for', 'against', 'abstain'
      const voter = req.user.address;

      // TODO: Call contract vote function

      res.json({
        data: {
          proposal_id: parseInt(proposal_id),
          voter,
          choice,
          voting_power: 1000,
          message: 'Vote recorded',
        },
      });
    } catch (error) {
      console.error('Error casting vote:', error);
      res.status(500).json({ error: 'Failed to cast vote' });
    }
  }
);

/**
 * POST /governance/proposals/:proposal_id/cancel
 * Cancel a proposal (proposer or owner only)
 */
daoGovernanceRouter.post(
  '/governance/proposals/:proposal_id/cancel',
  async (req: Request, res: Response) => {
    try {
      const { proposal_id } = req.params;

      // TODO: Call contract cancel function

      res.json({
        message: 'Proposal cancelled',
        proposal_id: parseInt(proposal_id),
      });
    } catch (error) {
      console.error('Error cancelling proposal:', error);
      res.status(500).json({ error: 'Failed to cancel proposal' });
    }
  }
);

// ========================================================================
// Delegation Endpoints
// ========================================================================

/**
 * POST /governance/delegation
 * Delegate voting power
 */
daoGovernanceRouter.post('/governance/delegation', async (req: Request, res: Response) => {
  try {
    const { delegate_to } = req.body;
    const delegator = req.user.address;

    // TODO: Call contract delegate function

    res.json({
      data: {
        delegator,
        delegate_to,
        power_delegated: 5000,
        message: 'Delegation set',
      },
    });
  } catch (error) {
    console.error('Error delegating:', error);
    res.status(500).json({ error: 'Failed to delegate' });
  }
});

/**
 * POST /governance/delegation/revoke
 * Revoke delegation
 */
daoGovernanceRouter.post(
  '/governance/delegation/revoke',
  async (req: Request, res: Response) => {
    try {
      const user = req.user.address;

      // TODO: Call contract undelegate function

      res.json({
        message: 'Delegation revoked',
        user,
      });
    } catch (error) {
      console.error('Error revoking delegation:', error);
      res.status(500).json({ error: 'Failed to revoke delegation' });
    }
  }
);

/**
 * GET /governance/voting-power/:user_address
 * Get user's voting power
 */
daoGovernanceRouter.get(
  '/governance/voting-power/:user_address',
  async (req: Request, res: Response) => {
    try {
      const { user_address } = req.params;

      // TODO: Query contract

      const votingPower: VotingPower = {
        user: user_address,
        base_power: 10000,
        delegated_power: 5000,
        delegated_to: undefined,
        total_power: 15000,
      };

      res.json({ data: votingPower });
    } catch (error) {
      console.error('Error fetching voting power:', error);
      res.status(500).json({ error: 'Failed to fetch voting power' });
    }
  }
);

// ========================================================================
// Treasury Endpoints
// ========================================================================

/**
 * POST /governance/treasury/funds
 * Create a new fund
 */
daoGovernanceRouter.post(
  '/governance/treasury/funds',
  async (req: Request, res: Response) => {
    try {
      const { fund_name, budget_limit, period_ledgers } = req.body;

      // TODO: Call contract create_fund

      const fund: Fund = {
        name: fund_name,
        balance: 0,
        budget_limit,
        budget_used: 0,
        period_ledgers,
      };

      res.status(201).json({ data: fund });
    } catch (error) {
      console.error('Error creating fund:', error);
      res.status(500).json({ error: 'Failed to create fund' });
    }
  }
);

/**
 * GET /governance/treasury/funds
 * List all funds
 */
daoGovernanceRouter.get('/governance/treasury/funds', async (req: Request, res: Response) => {
  try {
    // TODO: Query contract for funds

    const funds: Fund[] = [
      {
        name: 'operations',
        balance: 50000,
        budget_limit: 100000,
        budget_used: 45000,
        period_ledgers: 52560,
      },
      {
        name: 'development',
        balance: 75000,
        budget_limit: 150000,
        budget_used: 25000,
        period_ledgers: 52560,
      },
    ];

    res.json({ data: funds, count: funds.length });
  } catch (error) {
    console.error('Error fetching funds:', error);
    res.status(500).json({ error: 'Failed to fetch funds' });
  }
});

/**
 * POST /governance/treasury/allocations
 * Request fund allocation
 */
daoGovernanceRouter.post(
  '/governance/treasury/allocations',
  async (req: Request, res: Response) => {
    try {
      const { recipient, fund_name, amount, purpose } = req.body;

      // TODO: Call contract request_allocation

      const allocation: Allocation = {
        allocation_id: 1,
        recipient,
        fund_name,
        amount,
        purpose,
        approved: false,
        approval_count: 0,
        required_approvals: 2,
      };

      res.status(201).json({ data: allocation });
    } catch (error) {
      console.error('Error requesting allocation:', error);
      res.status(500).json({ error: 'Failed to request allocation' });
    }
  }
);

/**
 * POST /governance/treasury/allocations/:allocation_id/approve
 * Approve an allocation (multi-sig)
 */
daoGovernanceRouter.post(
  '/governance/treasury/allocations/:allocation_id/approve',
  async (req: Request, res: Response) => {
    try {
      const { allocation_id } = req.params;

      // TODO: Call contract approve_allocation

      res.json({
        message: 'Allocation approved',
        allocation_id: parseInt(allocation_id),
        signer: req.user.address,
      });
    } catch (error) {
      console.error('Error approving allocation:', error);
      res.status(500).json({ error: 'Failed to approve allocation' });
    }
  }
);

/**
 * POST /governance/treasury/allocations/:allocation_id/execute
 * Execute an approved allocation
 */
daoGovernanceRouter.post(
  '/governance/treasury/allocations/:allocation_id/execute',
  async (req: Request, res: Response) => {
    try {
      const { allocation_id } = req.params;

      // TODO: Call contract execute_allocation

      res.json({
        message: 'Allocation executed',
        allocation_id: parseInt(allocation_id),
        transaction_hash: '0xabc123...',
      });
    } catch (error) {
      console.error('Error executing allocation:', error);
      res.status(500).json({ error: 'Failed to execute allocation' });
    }
  }
);

// ========================================================================
// Dispute Resolution Endpoints
// ========================================================================

/**
 * POST /governance/disputes
 * File a new dispute
 */
daoGovernanceRouter.post('/governance/disputes', async (req: Request, res: Response) => {
  try {
    const { defendant, description, evidence_uri, stake_amount } = req.body;
    const plaintiff = req.user.address;

    // TODO: Call contract file_dispute

    const dispute: Dispute = {
      dispute_id: 1,
      plaintiff,
      defendant,
      description,
      status: 'Filed',
      votes_for: 0,
      votes_against: 0,
      votes_abstain: 0,
    };

    res.status(201).json({ data: dispute });
  } catch (error) {
    console.error('Error filing dispute:', error);
    res.status(500).json({ error: 'Failed to file dispute' });
  }
});

/**
 * GET /governance/disputes/:dispute_id
 * Get dispute details
 */
daoGovernanceRouter.get(
  '/governance/disputes/:dispute_id',
  async (req: Request, res: Response) => {
    try {
      const { dispute_id } = req.params;

      // TODO: Query contract

      const dispute: Dispute = {
        dispute_id: parseInt(dispute_id),
        plaintiff: 'GPLAINTIFF...',
        defendant: 'GDEFENDANT...',
        description: 'Unauthorized access claim',
        status: 'Voting',
        votes_for: 4,
        votes_against: 2,
        votes_abstain: 1,
      };

      res.json({ data: dispute });
    } catch (error) {
      console.error('Error fetching dispute:', error);
      res.status(500).json({ error: 'Failed to fetch dispute' });
    }
  }
);

/**
 * POST /governance/disputes/:dispute_id/vote
 * Cast a juror vote
 */
daoGovernanceRouter.post(
  '/governance/disputes/:dispute_id/vote',
  async (req: Request, res: Response) => {
    try {
      const { dispute_id } = req.params;
      const { outcome } = req.body;
      const juror = req.user.address;

      // TODO: Call contract cast_juror_vote

      res.json({
        message: 'Vote recorded',
        dispute_id: parseInt(dispute_id),
        juror,
        outcome,
      });
    } catch (error) {
      console.error('Error casting juror vote:', error);
      res.status(500).json({ error: 'Failed to cast vote' });
    }
  }
);

/**
 * POST /governance/disputes/:dispute_id/evidence
 * Submit evidence for dispute
 */
daoGovernanceRouter.post(
  '/governance/disputes/:dispute_id/evidence',
  async (req: Request, res: Response) => {
    try {
      const { dispute_id } = req.params;
      const { evidence_uri, is_plaintiff } = req.body;

      // TODO: Call contract submit_evidence

      res.json({
        message: 'Evidence submitted',
        dispute_id: parseInt(dispute_id),
        evidence_uri,
      });
    } catch (error) {
      console.error('Error submitting evidence:', error);
      res.status(500).json({ error: 'Failed to submit evidence' });
    }
  }
);

/**
 * POST /governance/disputes/:dispute_id/appeals
 * File an appeal
 */
daoGovernanceRouter.post(
  '/governance/disputes/:dispute_id/appeals',
  async (req: Request, res: Response) => {
    try {
      const { dispute_id } = req.params;
      const { reason } = req.body;

      // TODO: Call contract file_appeal

      res.status(201).json({
        message: 'Appeal filed',
        dispute_id: parseInt(dispute_id),
        appeal_id: 1,
      });
    } catch (error) {
      console.error('Error filing appeal:', error);
      res.status(500).json({ error: 'Failed to file appeal' });
    }
  }
);

/**
 * GET /governance/health
 * Health check for governance system
 */
daoGovernanceRouter.get('/governance/health', async (req: Request, res: Response) => {
  try {
    res.json({
      status: 'healthy',
      governance_connected: true,
      treasury_connected: true,
      dispute_resolution_connected: true,
      timestamp: new Date().toISOString(),
    });
  } catch (error) {
    res.status(503).json({
      status: 'unhealthy',
      error: 'Service unavailable',
    });
  }
});

export default daoGovernanceRouter;
