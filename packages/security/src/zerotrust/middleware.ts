import { Request, Response, NextFunction } from 'express';
import { SpiffeIdentityValidator } from './identity';
import { ContinuousVerificationManager } from './continuousVerification';
import { DeviceTrustEvaluator } from './deviceTrust';
import { NetworkSegment, TrustTier, ZeroTrustContext } from './types';

export interface ZeroTrustRequest extends Request {
  zeroTrust?: ZeroTrustContext;
}

export function createZeroTrustMiddleware(options: {
  currentSegment: NetworkSegment;
  spiffeValidator?: SpiffeIdentityValidator;
  sessionManager?: ContinuousVerificationManager;
  requiredTrustTier?: TrustTier;
}) {
  const spiffeValidator = options.spiffeValidator || new SpiffeIdentityValidator();
  const sessionManager = options.sessionManager || new ContinuousVerificationManager();
  const requiredTrustTier = options.requiredTrustTier ?? TrustTier.Low;

  return (req: ZeroTrustRequest, res: Response, next: NextFunction) => {
    const spiffeHeader = req.header('x-spiffe-id');
    const sessionId = req.header('x-zt-session-id');
    const deviceId = req.header('x-device-id') || 'unknown-device';

    let identity = undefined;
    if (spiffeHeader) {
      identity = spiffeValidator.parseSpiffeId(spiffeHeader) || undefined;
    }

    const currentSessionId = sessionId || `anon-${Date.now()}`;
    let evalResult = sessionManager.evaluateSession(currentSessionId);

    let session = undefined;
    if (!evalResult.valid) {
      session = sessionManager.createSession(
        currentSessionId,
        identity?.principal || 'anonymous-workload',
        deviceId,
        identity ? TrustTier.Medium : TrustTier.Low,
        identity?.spiffeId
      );
    }

    const currentTier = session?.trustTier ?? TrustTier.Low;
    if (currentTier < requiredTrustTier) {
      return res.status(403).json({
        error: 'Forbidden',
        message: 'Zero-Trust tier requirement not satisfied',
        requiredTier: requiredTrustTier,
        actualTier: currentTier,
      });
    }

    req.zeroTrust = {
      identity,
      session: session!,
      activeCapabilities: (req.header('x-zt-capabilities') || '').split(',').filter(Boolean),
      currentSegment: options.currentSegment,
    };

    next();
  };
}
