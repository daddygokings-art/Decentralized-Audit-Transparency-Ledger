import { createHash, randomBytes } from "crypto";

export type CodeChallengeMethod = "S256" | "plain";

function base64url(input: Buffer): string {
  return input.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** RFC 7636 §4.1 — 43 to 128 char unreserved-character random string. */
export function generateCodeVerifier(): string {
  return base64url(randomBytes(32));
}

/** RFC 7636 §4.2 */
export function generateCodeChallenge(verifier: string, method: CodeChallengeMethod = "S256"): string {
  if (method === "plain") return verifier;
  return base64url(createHash("sha256").update(verifier).digest());
}

/** RFC 7636 §4.6 — verify a presented code_verifier against the stored challenge. */
export function verifyCodeChallenge(
  verifier: string,
  challenge: string,
  method: CodeChallengeMethod = "S256"
): boolean {
  if (!verifier || !challenge) return false;
  const computed = generateCodeChallenge(verifier, method);
  return timingSafeEqualStr(computed, challenge);
}

function timingSafeEqualStr(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) {
    mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return mismatch === 0;
}
