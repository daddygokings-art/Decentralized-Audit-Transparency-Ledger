/**
 * End-to-end tests for AuditLedgerVerifier (issue #110).
 *
 * Covers: valid proof, replay rejection, stale proof rejection,
 * invalid signature rejection.
 */

import { ethers } from "hardhat";
import { expect } from "chai";
import { Wallet, keccak256, solidityPacked, getBytes, randomBytes, hexlify } from "ethers";

describe("AuditLedgerVerifier", function () {
  let verifier: Awaited<ReturnType<typeof deployVerifier>>;
  let trustedSigner: Wallet;

  async function deployVerifier() {
    trustedSigner = Wallet.createRandom();
    const Verifier = await ethers.getContractFactory("AuditLedgerVerifier");
    const contract = await Verifier.deploy(trustedSigner.address);
    await contract.waitForDeployment();
    return contract;
  }

  function proofDigest(ledgerSeq: bigint, txHash: string, eventHash: string): Uint8Array {
    const digest = keccak256(
      solidityPacked(["uint64", "bytes32", "bytes32"], [ledgerSeq, txHash, eventHash])
    );
    return getBytes(digest);
  }

  async function sign(ledgerSeq: bigint, txHash: string, eventHash: string): Promise<string> {
    return trustedSigner.signMessage(proofDigest(ledgerSeq, txHash, eventHash));
  }

  beforeEach(async function () {
    verifier = await deployVerifier();
  });

  it("accepts a valid proof", async function () {
    const ledgerSeq = 100n;
    const txHash = hexlify(randomBytes(32));
    const eventHash = hexlify(randomBytes(32));
    const eventIndex = 0;

    const sig = await sign(ledgerSeq, txHash, eventHash);

    expect(await verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, sig)).to.be.true;
    expect(await verifier.isVerified(eventHash)).to.be.true;
  });

  it("rejects a replayed proof", async function () {
    const ledgerSeq = 200n;
    const txHash = hexlify(randomBytes(32));
    const eventHash = hexlify(randomBytes(32));
    const eventIndex = 1;

    const sig = await sign(ledgerSeq, txHash, eventHash);

    // First submission succeeds
    await verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, sig);

    // Second submission with same eventHash must revert with AlreadyVerified
    await expect(
      verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, sig)
    ).to.be.revertedWithCustomError(verifier, "AlreadyVerified");
  });

  it("rejects a stale proof", async function () {
    // Accept a proof at ledger 2000 to advance latestAcceptedLedger
    const recentLedger = 2000n;
    const txHash1 = hexlify(randomBytes(32));
    const eventHash1 = hexlify(randomBytes(32));
    const sig1 = await sign(recentLedger, txHash1, eventHash1);
    await verifier.verifyEvent(recentLedger, txHash1, 0, eventHash1, sig1);

    // Attempt a proof more than maxLedgerAge (1000) ledgers behind
    const staleLedger = 999n; // 2000 - 999 = 1001 > 1000
    const txHash2 = hexlify(randomBytes(32));
    const eventHash2 = hexlify(randomBytes(32));
    const sig2 = await sign(staleLedger, txHash2, eventHash2);

    await expect(
      verifier.verifyEvent(staleLedger, txHash2, 1, eventHash2, sig2)
    ).to.be.revertedWithCustomError(verifier, "ProofTooOld");
  });

  it("rejects a proof with an invalid signature", async function () {
    const ledgerSeq = 300n;
    const txHash = hexlify(randomBytes(32));
    const eventHash = hexlify(randomBytes(32));

    // Sign with a different (untrusted) key
    const rogue = Wallet.createRandom();
    const badSig = await rogue.signMessage(proofDigest(ledgerSeq, txHash, eventHash));

    await expect(
      verifier.verifyEvent(ledgerSeq, txHash, 2, eventHash, badSig)
    ).to.be.revertedWithCustomError(verifier, "InvalidProof");
  });
});
