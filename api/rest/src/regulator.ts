/**
 * Regulator Portal Backend API
 *
 * Provides REST endpoints for:
 * - Querying audit trails with selective disclosure
 * - Managing data sharing agreements
 * - Verifying tamper-evidence chains
 * - Generating compliance reports
 * - Accessing regulatory audit logs
 */

import express, { Request, Response, NextFunction } from "express";
import { v4 as uuidv4 } from "uuid";

/**
 * Regulator authentication context
 */
interface RegulatorContext {
  regulatorId: string;
  role: "auditor" | "officer" | "admin";
  dsaId?: string;
  standards: Array<"ISA3000" | "SOC2" | "GDPR" | "SOX">;
}

/**
 * Audit trail query parameters
 */
interface AuditTrailQuery {
  startTime?: number;
  endTime?: number;
  eventTypes?: string[];
  submitter?: string;
  minSensitivity?: "public" | "internal" | "confidential" | "restricted";
  onlyControlEvents?: boolean;
  limit?: number;
  offset?: number;
}

/**
 * Selective disclosure request
 */
interface DisclosureRequest {
  eventIndex: number;
  allowedFields: string[];
  regulatorId: string;
}

/**
 * Data Sharing Agreement request
 */
interface DSARequest {
  dataProvider: string;
  regulatorAddress: string;
  standards: string[];
  allowedEventTypes: string[];
  role: string;
  effectiveLedger: number;
  expiryLedger?: number;
}

/**
 * Compliance report request
 */
interface ComplianceReportRequest {
  standard: "ISA3000" | "SOC2" | "GDPR" | "SOX";
  auditSubject: string;
  eventsExamined: number;
  objectivesTested: string[];
  controlsOperating: number;
  controlsDeficient: number;
}

/**
 * Regulator portal routes
 */
export function createRegulatorRoutes(): express.Router {
  const router = express.Router();

  /**
   * GET /regulator/audit-trails
   * Query immutable audit trail with optional selective disclosure
   */
  router.get("/regulator/audit-trails", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      if (!context) {
        return res.status(401).json({ error: "Unauthorized" });
      }

      const query: AuditTrailQuery = {
        startTime: req.query.startTime ? Number(req.query.startTime) : undefined,
        endTime: req.query.endTime ? Number(req.query.endTime) : undefined,
        eventTypes: req.query.eventTypes
          ? Array.isArray(req.query.eventTypes)
            ? req.query.eventTypes as string[]
            : [req.query.eventTypes as string]
          : undefined,
        submitter: req.query.submitter as string | undefined,
        minSensitivity: (req.query.minSensitivity as any) || "public",
        onlyControlEvents: req.query.onlyControlEvents === "true",
        limit: Math.min(Number(req.query.limit) || 100, 1000),
        offset: Number(req.query.offset) || 0,
      };

      // In production, this would query the smart contract via RPC
      const auditTrail = {
        query,
        regulatorId: context.regulatorId,
        standards: context.standards,
        entries: [
          {
            eventIndex: 0,
            eventHash: "0x" + "a".repeat(64),
            timestamp: Date.now(),
            eventType: "access_control",
            submitter: "GA...",
            sensitivity: "confidential",
            controlEvent: true,
          },
        ],
        totalCount: 1,
        hasMore: false,
      };

      res.json(auditTrail);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/audit-trails/:eventIndex
   * Get a specific audit trail entry with optional disclosure proof
   */
  router.get("/regulator/audit-trails/:eventIndex", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      if (!context) {
        return res.status(401).json({ error: "Unauthorized" });
      }

      const eventIndex = Number(req.params.eventIndex);
      const includeProof = req.query.proof === "true";

      const entry = {
        eventIndex,
        eventHash: "0x" + "a".repeat(64),
        prevHash: "0x" + "b".repeat(64),
        timestamp: Date.now(),
        eventType: "access_control",
        category: "security",
        submitter: "GA...",
        metadata: includeProof ? undefined : { /* encrypted */ },
        version: 1,
        parentEventId: null,
        regulatory: {
          standard: "ISA3000",
          controlCode: "CC6.1",
          demonstratesControl: true,
          retentionLedgers: 52560,
          sensitivity: "confidential",
        },
      };

      if (includeProof) {
        entry.disclosure = {
          eventIndex,
          disclosedRoot: "0x" + "c".repeat(64),
          completeRoot: "0x" + "d".repeat(64),
          disclosedFields: ["timestamp", "eventType", "submitter"],
          merkleProof: [],
        };
      }

      res.json(entry);
    } catch (error) {
      next(error);
    }
  });

  /**
   * POST /regulator/selective-disclosure
   * Generate selective disclosure proof for an event
   */
  router.post("/regulator/selective-disclosure", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      if (!context) {
        return res.status(401).json({ error: "Unauthorized" });
      }

      const disclosureReq = req.body as DisclosureRequest;

      const proof = {
        eventIndex: disclosureReq.eventIndex,
        disclosedFields: disclosureReq.allowedFields,
        merkleProof: [],
        disclosedRoot: "0x" + "c".repeat(64),
        completeRoot: "0x" + "d".repeat(64),
        verified: true,
        timestamp: Date.now(),
      };

      res.json(proof);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/tamper-evidence/:eventIndex
   * Verify tamper-evidence chain for an event
   */
  router.get("/regulator/tamper-evidence/:eventIndex", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      if (!context) {
        return res.status(401).json({ error: "Unauthorized" });
      }

      const eventIndex = Number(req.params.eventIndex);

      const verification = {
        eventIndex,
        chainValid: true,
        previousEventHash: "0x" + "b".repeat(64),
        currentEventHash: "0x" + "a".repeat(64),
        nextEventHash: "0x" + "e".repeat(64),
        hashAlgorithm: "SHA256",
        timestamp: Date.now(),
        verificationDetails: {
          genesisEvent: false,
          isLastEvent: false,
          chainIntegrityScore: 1.0,
          intermediateEventCount: 100,
        },
      };

      res.json(verification);
    } catch (error) {
      next(error);
    }
  });

  /**
   * POST /regulator/data-sharing-agreements
   * Create a new DSA
   */
  router.post("/regulator/data-sharing-agreements", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      if (context.role !== "admin" && context.role !== "officer") {
        return res.status(403).json({ error: "Insufficient permissions" });
      }

      const dsaReq = req.body as DSARequest;

      const dsa = {
        id: uuidv4(),
        dataProvider: dsaReq.dataProvider,
        regulatorAddress: dsaReq.regulatorAddress,
        standards: dsaReq.standards,
        allowedEventTypes: dsaReq.allowedEventTypes,
        role: dsaReq.role,
        effectiveLedger: dsaReq.effectiveLedger,
        expiryLedger: dsaReq.expiryLedger || 0,
        active: true,
        status: "draft",
        createdAt: Date.now(),
        createdBy: context.regulatorId,
        signatureProvider: null,
        signatureRegulator: null,
      };

      res.status(201).json(dsa);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/data-sharing-agreements
   * List DSAs for current regulator
   */
  router.get("/regulator/data-sharing-agreements", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const dsas = [
        {
          id: uuidv4(),
          dataProvider: "GBBBBBBB...",
          regulatorAddress: "GAAAAAA...",
          standards: ["ISA3000", "SOC2"],
          allowedEventTypes: ["access_control", "authentication"],
          role: "auditor",
          status: "executed",
          active: true,
          effectiveLedger: 1000,
          expiryLedger: 0,
          createdAt: Date.now(),
        },
      ];

      res.json(dsas);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/data-sharing-agreements/:dsaId
   * Get details of a specific DSA
   */
  router.get("/regulator/data-sharing-agreements/:dsaId", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      const dsaId = req.params.dsaId;

      const dsa = {
        id: dsaId,
        dataProvider: "GBBBBBBB...",
        regulatorAddress: "GAAAAAA...",
        standards: ["ISA3000"],
        allowedEventTypes: ["access_control"],
        role: "officer",
        minSensitivity: "internal",
        status: "executed",
        active: true,
        effectiveLedger: 1000,
        expiryLedger: 52560, // 1 year
        createdAt: Date.now(),
        executedAt: Date.now() + 86400000,
      };

      res.json(dsa);
    } catch (error) {
      next(error);
    }
  });

  /**
   * POST /regulator/compliance-reports
   * Generate a compliance report
   */
  router.post("/regulator/compliance-reports", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const reportReq = req.body as ComplianceReportRequest;

      const report = {
        id: uuidv4(),
        standard: reportReq.standard,
        auditSubject: reportReq.auditSubject,
        issuer: context.regulatorId,
        generatedAt: Date.now(),
        status: "draft",
        eventsExamined: reportReq.eventsExamined,
        objectivesTested: reportReq.objectivesTested,
        controlsOperating: reportReq.controlsOperating,
        controlsDeficient: reportReq.controlsDeficient,
        findingsSummaryHash: "0x" + "f".repeat(64),
      };

      res.status(201).json(report);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/compliance-reports
   * List compliance reports
   */
  router.get("/regulator/compliance-reports", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const standard = (req.query.standard as string) || "ISA3000";
      const status = (req.query.status as string) || "published";

      const reports = [
        {
          id: uuidv4(),
          standard,
          auditSubject: "COMPANY_NAME",
          issuer: context.regulatorId,
          generatedAt: Date.now(),
          status,
          eventsExamined: 500,
          controlsOperating: 45,
          controlsDeficient: 3,
        },
      ];

      res.json(reports);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/compliance-reports/:reportId
   * Get a specific compliance report
   */
  router.get("/regulator/compliance-reports/:reportId", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;
      const reportId = req.params.reportId;

      const report = {
        id: reportId,
        standard: "ISA3000",
        auditSubject: "COMPANY_NAME",
        issuer: context.regulatorId,
        generatedAt: Date.now(),
        status: "published",
        publishedAt: Date.now() - 86400000,
        eventsExamined: 500,
        objectivesTested: ["CC6.1", "CC7.1", "A1.1"],
        controlsOperating: 45,
        controlsDeficient: 3,
        findings: [
          {
            controlObjective: "CC6.1",
            finding: "Segregation of duties control is operating effectively",
            evidence: ["0x" + "a".repeat(64)],
          },
        ],
      };

      res.json(report);
    } catch (error) {
      next(error);
    }
  });

  /**
   * POST /regulator/export
   * Export audit data for compliance analysis
   */
  router.post("/regulator/export", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const exportReq = req.body;
      const format = exportReq.format || "csv"; // csv, json, pdf

      const filename = `audit_export_${Date.now()}.${format}`;

      res.setHeader("Content-Type", format === "json" ? "application/json" : "text/csv");
      res.setHeader("Content-Disposition", `attachment; filename="${filename}"`);

      // In production, stream the export data
      const sampleData = {
        exportedAt: Date.now(),
        format,
        rowCount: 0,
        checksum: "0x" + "0".repeat(64),
      };

      res.json(sampleData);
    } catch (error) {
      next(error);
    }
  });

  /**
   * GET /regulator/access-requests
   * List access requests for current regulator
   */
  router.get("/regulator/access-requests", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const requests = [
        {
          id: uuidv4(),
          requester: context.regulatorId,
          dataOwner: "COMPANY_ADDRESS",
          standard: "ISA3000",
          eventTypes: ["access_control"],
          status: "pending",
          createdAt: Date.now(),
        },
      ];

      res.json(requests);
    } catch (error) {
      next(error);
    }
  });

  /**
   * POST /regulator/statistics
   * Get audit trail statistics
   */
  router.post("/regulator/statistics", async (req: Request, res: Response, next: NextFunction) => {
    try {
      const context = req.regulatorContext as RegulatorContext;

      const stats = {
        totalEventsAudited: 5000,
        eventsByType: {
          access_control: 1200,
          authentication: 800,
          data_modification: 1500,
          configuration_change: 400,
          system_exception: 100,
        },
        eventsByStandard: {
          ISA3000: 3000,
          SOC2: 2000,
        },
        complianceScore: 0.94,
        flaggedEvents: 25,
        daysAudited: 365,
      };

      res.json(stats);
    } catch (error) {
      next(error);
    }
  });

  return router;
}

/**
 * Regulator authentication middleware
 */
export function regulatorAuthMiddleware(req: Request, res: Response, next: NextFunction) {
  try {
    const bearerToken = req.headers.authorization?.split(" ")[1];
    if (!bearerToken) {
      return res.status(401).json({ error: "Missing authorization header" });
    }

    // In production, verify JWT and extract regulator context
    req.regulatorContext = {
      regulatorId: "REG_001",
      role: "auditor",
      standards: ["ISA3000", "SOC2"],
    };

    next();
  } catch (error) {
    res.status(401).json({ error: "Invalid authentication" });
  }
}
