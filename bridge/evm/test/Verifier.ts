import { ethers } from "hardhat";
import { expect } from "chai";

describe("AuditLedgerVerifier Multi-Sig", () => {
  let verifier: any;
  let owner: any;
  let signer1: any;
  let signer2: any;
  let signer3: any;
  let nonSigner: any;

  const ledgerSeq = BigInt(1000);
  const txHash = ethers.id("test-tx");
  const eventIndex = 0;
  const eventHash = ethers.id("test-event");

  beforeEach(async () => {
    const [ownerSigner, s1, s2, s3, nonSignerSigner] = await ethers.getSigners();
    owner = ownerSigner;
    signer1 = s1;
    signer2 = s2;
    signer3 = s3;
    nonSigner = nonSignerSigner;

    const Verifier = await ethers.getContractFactory("AuditLedgerVerifier");
    const signers = [signer1.address, signer2.address, signer3.address];
    const threshold = 2;
    verifier = await Verifier.deploy(signers, threshold);
    await verifier.waitForDeployment();
  });

  it("should verify event with exact threshold signatures", async () => {
    const digest = ethers.solidityPacked(
      ["uint64", "bytes32", "bytes32"],
      [ledgerSeq, txHash, eventHash]
    );
    const hash = ethers.keccak256(digest);
    const ethSignedDigest = ethers.keccak256(
      ethers.solidityPacked(
        ["string", "bytes32"],
        ["\x19Ethereum Signed Message:\n32", hash]
      )
    );

    const sig1 = await signer1.signMessage(ethers.getBytes(ethSignedDigest));
    const sig2 = await signer2.signMessage(ethers.getBytes(ethSignedDigest));

    const result = await verifier.verifyEvent(
      ledgerSeq,
      txHash,
      eventIndex,
      eventHash,
      [sig1, sig2]
    );

    expect(result).to.be.true;
    expect(await verifier.isVerified(eventHash)).to.be.true;
  });

  it("should reject below threshold signatures", async () => {
    const digest = ethers.solidityPacked(
      ["uint64", "bytes32", "bytes32"],
      [ledgerSeq, txHash, eventHash]
    );
    const hash = ethers.keccak256(digest);
    const ethSignedDigest = ethers.keccak256(
      ethers.solidityPacked(
        ["string", "bytes32"],
        ["\x19Ethereum Signed Message:\n32", hash]
      )
    );

    const sig1 = await signer1.signMessage(ethers.getBytes(ethSignedDigest));

    await expect(
      verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, [sig1])
    ).to.be.revertedWithCustomError(verifier, "InvalidSignature");
  });

  it("should reject duplicate signers", async () => {
    const digest = ethers.solidityPacked(
      ["uint64", "bytes32", "bytes32"],
      [ledgerSeq, txHash, eventHash]
    );
    const hash = ethers.keccak256(digest);
    const ethSignedDigest = ethers.keccak256(
      ethers.solidityPacked(
        ["string", "bytes32"],
        ["\x19Ethereum Signed Message:\n32", hash]
      )
    );

    const sig1 = await signer1.signMessage(ethers.getBytes(ethSignedDigest));

    await expect(
      verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, [sig1, sig1])
    ).to.be.revertedWithCustomError(verifier, "DuplicateSigner");
  });

  it("should reject signatures from non-registered signers", async () => {
    const digest = ethers.solidityPacked(
      ["uint64", "bytes32", "bytes32"],
      [ledgerSeq, txHash, eventHash]
    );
    const hash = ethers.keccak256(digest);
    const ethSignedDigest = ethers.keccak256(
      ethers.solidityPacked(
        ["string", "bytes32"],
        ["\x19Ethereum Signed Message:\n32", hash]
      )
    );

    const sig1 = await signer1.signMessage(ethers.getBytes(ethSignedDigest));
    const sigNonSigner = await nonSigner.signMessage(ethers.getBytes(ethSignedDigest));

    await expect(
      verifier.verifyEvent(ledgerSeq, txHash, eventIndex, eventHash, [sig1, sigNonSigner])
    ).to.be.revertedWithCustomError(verifier, "InvalidProof");
  });

  it("should allow owner to update signers", async () => {
    const newSigners = [signer1.address, signer3.address];
    const newThreshold = 2;
    await verifier.updateSigners(newSigners, newThreshold);

    expect(await verifier.threshold()).to.equal(newThreshold);
  });

  it("should prevent invalid threshold (0)", async () => {
    const Verifier = await ethers.getContractFactory("AuditLedgerVerifier");
    await expect(
      Verifier.deploy([signer1.address, signer2.address], 0)
    ).to.be.revertedWithCustomError(Verifier, "InvalidThreshold");
  });

  it("should prevent invalid threshold (greater than signer count)", async () => {
    const Verifier = await ethers.getContractFactory("AuditLedgerVerifier");
    await expect(
      Verifier.deploy([signer1.address, signer2.address], 3)
    ).to.be.revertedWithCustomError(Verifier, "InvalidThreshold");
  });
});
