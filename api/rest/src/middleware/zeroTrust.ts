import { Request, Response, NextFunction } from 'express';

export interface ZeroTrustAuthRequest extends Request {
  user?: {
    id: string;
    role: string;
    trustTier: number;
    spiffeId?: string;
  };
}

/**
 * Zero-Trust continuous verification and least privilege enforcer for REST endpoints
 */
export function zeroTrustEnforcer(options?: { minTrustTier?: number; requiredCapability?: string }) {
  const minTier = options?.minTrustTier ?? 1; // 1 = Low, 2 = Medium, 3 = High, 4 = Verified

  return (req: ZeroTrustAuthRequest, res: Response, next: NextFunction) => {
    const spiffeId = req.header('x-spiffe-id');
    const deviceScore = parseInt(req.header('x-device-posture-score') || '50', 10);
    const clientTier = spiffeId ? (deviceScore >= 80 ? 3 : 2) : 1;

    if (clientTier < minTier) {
      return res.status(403).json({
        error: 'Forbidden',
        message: 'Endpoint requires elevated Zero-Trust verification level',
        requiredTier: minTier,
        currentTier: clientTier,
      });
    }

    if (options?.requiredCapability) {
      const caps = (req.header('x-capabilities') || '').split(',').map((s) => s.trim());
      if (!caps.includes(options.requiredCapability) && !caps.includes('*')) {
        return res.status(403).json({
          error: 'Forbidden',
          message: `Missing required capability: ${options.requiredCapability}`,
        });
      }
    }

    req.user = {
      id: req.header('x-user-id') || 'workload-principal',
      role: req.header('x-user-role') || 'client',
      trustTier: clientTier,
      spiffeId,
    };

    next();
  };
}
