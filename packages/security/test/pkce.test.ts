import { describe, expect, it } from "vitest";
import { generateCodeChallenge, generateCodeVerifier, verifyCodeChallenge } from "../src/oauth/pkce";

describe("PKCE", () => {
  it("round-trips an S256 challenge/verifier pair", () => {
    const verifier = generateCodeVerifier();
    const challenge = generateCodeChallenge(verifier, "S256");
    expect(verifyCodeChallenge(verifier, challenge, "S256")).toBe(true);
  });

  it("rejects an incorrect verifier", () => {
    const verifier = generateCodeVerifier();
    const challenge = generateCodeChallenge(verifier, "S256");
    expect(verifyCodeChallenge("wrong-verifier-value", challenge, "S256")).toBe(false);
  });

  it("supports the plain method", () => {
    const verifier = generateCodeVerifier();
    const challenge = generateCodeChallenge(verifier, "plain");
    expect(challenge).toBe(verifier);
    expect(verifyCodeChallenge(verifier, challenge, "plain")).toBe(true);
  });

  it("generates verifiers of sufficient length and uniqueness", () => {
    const a = generateCodeVerifier();
    const b = generateCodeVerifier();
    expect(a.length).toBeGreaterThanOrEqual(43);
    expect(a).not.toEqual(b);
  });
});
