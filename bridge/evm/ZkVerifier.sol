// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/**
 * @title ZkVerifier — Groth16 ZK Proof Verification for AuditLedger Events
 * @notice Issue #374: EVM bridge ZK proof verification.
 *
 * Verifies zero-knowledge inclusion proofs for AuditLedger events bridged from
 * the Stellar/Soroban network to EVM chains.
 *
 * Proof system: Groth16 over BN254 (alt_bn128).
 *   - Verification key (vk) is set at deployment and cannot be changed
 *     (trusted setup is immutable once deployed).
 *   - Proof format: (A, B, C) — 3 elliptic-curve points encoded as 256 bytes
 *     (32 bytes per coordinate, A=G1, B=G2, C=G1).
 *   - Public inputs: arbitrary-length array of field elements (bytes32[]).
 *
 * Gas optimisation:
 *   Uses the EVM precompiled contracts:
 *     0x06 — ecAdd     (G1 point addition)
 *     0x07 — ecMul     (G1 scalar multiplication)
 *     0x08 — ecPairing (BN254 pairing check)
 *
 * Deployment:
 *   1. Generate the verification key via snarkjs:
 *      `snarkjs groth16 setup circuit.r1cs powersOfTau.ptau vk.zkey`
 *      `snarkjs zkey export verificationkey vk.zkey vk.json`
 *   2. Encode vk.json as the constructor parameters below.
 *   3. Deploy ZkVerifier with the encoded verification key.
 *
 * @dev Adapted from the EIP-197 reference implementation and snarkjs
 *      solidity verifier template (MIT).
 */
contract ZkVerifier {

    // ── Errors ────────────────────────────────────────────────────────────────

    error InvalidProofLength();
    error InvalidPublicInputsLength();
    error PairingCheckFailed();
    error EcOperationFailed();
    error NotOwner();
    error AlreadyInitialised();
    error BatchLengthMismatch();

    // ── Events ────────────────────────────────────────────────────────────────

    /// @notice Emitted when a single proof is successfully verified.
    event ProofVerified(
        bytes32 indexed eventHash,
        address indexed verifier,
        uint256 timestamp,
        bool    success
    );

    /// @notice Emitted when a batch verification completes.
    event BatchVerificationComplete(
        uint256 totalProofs,
        uint256 validProofs,
        uint256 timestamp
    );

    // ── Verification Key storage ──────────────────────────────────────────────

    /**
     * @dev BN254 G1 point.
     */
    struct G1Point {
        uint256 x;
        uint256 y;
    }

    /**
     * @dev BN254 G2 point (two-component coordinates in the extension field).
     */
    struct G2Point {
        uint256[2] x;   // [real, imag]
        uint256[2] y;
    }

    /**
     * @dev Groth16 verification key.
     *
     * alpha1: G1 point from trusted setup.
     * beta2:  G2 point from trusted setup.
     * gamma2: G2 point from trusted setup.
     * delta2: G2 point from trusted setup.
     * ic[]:   G1 points, one per public input + 1 for the constant term.
     */
    struct VerificationKey {
        G1Point alpha1;
        G2Point beta2;
        G2Point gamma2;
        G2Point delta2;
        G1Point[] ic;   // length == nPublicInputs + 1
    }

    /**
     * @dev Groth16 proof.
     *
     * Encoded as 256 bytes:
     *   bytes  0-63  : A  (G1 point, x || y, 32 bytes each)
     *   bytes 64-191 : B  (G2 point, [x_real, x_imag] || [y_real, y_imag])
     *   bytes 192-255: C  (G1 point, x || y)
     */
    struct Proof {
        G1Point a;
        G2Point b;
        G1Point c;
    }

    // ── State ─────────────────────────────────────────────────────────────────

    VerificationKey private _vk;
    address public immutable owner;
    uint256 public totalVerifications;
    uint256 public successfulVerifications;

    // ── Constructor ───────────────────────────────────────────────────────────

    /**
     * @param alpha1X  alpha1.x from trusted setup
     * @param alpha1Y  alpha1.y
     * @param beta2X   beta2.x  [real, imag]
     * @param beta2Y   beta2.y  [real, imag]
     * @param gamma2X  gamma2.x [real, imag]
     * @param gamma2Y  gamma2.y [real, imag]
     * @param delta2X  delta2.x [real, imag]
     * @param delta2Y  delta2.y [real, imag]
     * @param icX      ic[i].x values (length = nPublicInputs + 1)
     * @param icY      ic[i].y values
     */
    constructor(
        uint256    alpha1X, uint256   alpha1Y,
        uint256[2] memory beta2X,   uint256[2] memory beta2Y,
        uint256[2] memory gamma2X,  uint256[2] memory gamma2Y,
        uint256[2] memory delta2X,  uint256[2] memory delta2Y,
        uint256[]  memory icX,      uint256[]  memory icY
    ) {
        require(icX.length == icY.length, "ZkVerifier: ic length mismatch");

        owner = msg.sender;

        _vk.alpha1 = G1Point(alpha1X, alpha1Y);
        _vk.beta2  = G2Point(beta2X,  beta2Y);
        _vk.gamma2 = G2Point(gamma2X, gamma2Y);
        _vk.delta2 = G2Point(delta2X, delta2Y);

        _vk.ic = new G1Point[](icX.length);
        for (uint256 i = 0; i < icX.length; ++i) {
            _vk.ic[i] = G1Point(icX[i], icY[i]);
        }
    }

    // ── Public verification API ───────────────────────────────────────────────

    /**
     * @notice Verify a single Groth16 proof with arbitrary public inputs.
     *
     * @param proof        256-byte proof: [A.x, A.y, B.x0, B.x1, B.y0, B.y1, C.x, C.y]
     * @param publicInputs Array of field elements (BN254 scalar field, < p).
     * @return valid       true if the pairing equation holds.
     */
    function verifyEventZkProof(
        bytes calldata    proof,
        bytes32[] calldata publicInputs
    ) external view returns (bool valid) {
        if (proof.length != 256) revert InvalidProofLength();
        if (publicInputs.length + 1 != _vk.ic.length) revert InvalidPublicInputsLength();

        Proof memory p = _decodeProof(proof);
        valid = _verifyGroth16(p, _castPublicInputs(publicInputs));
    }

    /**
     * @notice Batch verify multiple proofs in a single call.
     *
     * Each proof is verified independently. The function never reverts for an
     * individual invalid proof — it records `false` in the result array.
     *
     * @param proofs            Array of 256-byte proofs.
     * @param publicInputsArray Parallel array of public-input arrays.
     * @return results          Parallel boolean results.
     */
    function verifyBatchProof(
        bytes[] calldata    proofs,
        bytes32[][] calldata publicInputsArray
    ) external returns (bool[] memory results) {
        if (proofs.length != publicInputsArray.length) revert BatchLengthMismatch();

        uint256 n = proofs.length;
        results = new bool[](n);
        uint256 validCount;

        for (uint256 i = 0; i < n; ++i) {
            // Individual proof failures are tolerated — record false.
            if (proofs[i].length != 256) {
                results[i] = false;
                continue;
            }
            if (publicInputsArray[i].length + 1 != _vk.ic.length) {
                results[i] = false;
                continue;
            }
            Proof memory p = _decodeProof(proofs[i]);
            results[i] = _verifyGroth16(p, _castPublicInputs(publicInputsArray[i]));
            if (results[i]) {
                ++validCount;
                emit ProofVerified(bytes32(0), msg.sender, block.timestamp, true);
            }
        }

        emit BatchVerificationComplete(n, validCount, block.timestamp);
    }

    /**
     * @notice High-level helper: verify that `eventHash` is included in the
     *         AuditLedger state committed to in the proof.
     *
     * The first public input is interpreted as the event hash (keccak256 of
     * the serialised event struct); the remaining inputs are the Merkle root
     * and any additional context values.
     *
     * @param eventHash 32-byte hash of the Soroban Event struct.
     * @param proof     256-byte Groth16 proof.
     * @return included true if the proof verifies the event's inclusion.
     */
    function verifyEventInclusion(
        bytes32        eventHash,
        bytes calldata proof
    ) external returns (bool included) {
        if (proof.length != 256) revert InvalidProofLength();
        // The first IC element is for the constant; IC[1] corresponds to
        // the first public input (eventHash cast to uint256).
        if (_vk.ic.length < 2) revert InvalidPublicInputsLength();

        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(eventHash);

        Proof memory p = _decodeProof(proof);
        included = _verifyGroth16(p, inputs);

        ++totalVerifications;
        if (included) {
            ++successfulVerifications;
        }

        emit ProofVerified(eventHash, msg.sender, block.timestamp, included);
    }

    // ── Verification key read ─────────────────────────────────────────────────

    /**
     * @notice Return the number of public inputs this verifier expects.
     */
    function nPublicInputs() external view returns (uint256) {
        return _vk.ic.length > 0 ? _vk.ic.length - 1 : 0;
    }

    /**
     * @notice Return the alpha1 point of the verification key.
     */
    function getAlpha1() external view returns (uint256 x, uint256 y) {
        return (_vk.alpha1.x, _vk.alpha1.y);
    }

    // ── Internal: Groth16 verifier ────────────────────────────────────────────

    /**
     * @dev Core Groth16 verification.
     *
     * Checks: e(A, B) == e(alpha1, beta2) * e(vk_x, gamma2) * e(C, delta2)
     *
     * Where vk_x = IC[0] + sum(IC[i+1] * input[i])
     *
     * This is validated via a single ecPairing call with 4 pairs (negated A).
     */
    function _verifyGroth16(
        Proof memory p,
        uint256[] memory inputs
    ) internal view returns (bool) {
        // Compute linear combination: vk_x = IC[0] + Σ IC[i+1]·inputs[i]
        G1Point memory vk_x = _g1Copy(_vk.ic[0]);
        for (uint256 i = 0; i < inputs.length; ++i) {
            G1Point memory term = _g1ScalarMul(_vk.ic[i + 1], inputs[i]);
            vk_x = _g1Add(vk_x, term);
        }

        // Pairing check: e(−A, B) · e(alpha1, beta2) · e(vk_x, gamma2) · e(C, delta2) == 1
        G1Point memory negA = _g1Negate(p.a);

        return _pairing4(
            negA,     p.b,
            _vk.alpha1, _vk.beta2,
            vk_x,     _vk.gamma2,
            p.c,      _vk.delta2
        );
    }

    // ── Internal: elliptic curve helpers (use EVM precompiles) ───────────────

    uint256 private constant FIELD_PRIME =
        21888242871839275222246405745257275088696311157297823662689037894645226208583;

    /**
     * @dev G1 point negation: (x, y) → (x, p - y).
     */
    function _g1Negate(G1Point memory p_) internal pure returns (G1Point memory) {
        if (p_.x == 0 && p_.y == 0) return G1Point(0, 0);
        return G1Point(p_.x, FIELD_PRIME - (p_.y % FIELD_PRIME));
    }

    /**
     * @dev Copy a G1 point to avoid aliasing.
     */
    function _g1Copy(G1Point memory p_) internal pure returns (G1Point memory) {
        return G1Point(p_.x, p_.y);
    }

    /**
     * @dev G1 point addition via precompile 0x06.
     */
    function _g1Add(G1Point memory a, G1Point memory b) internal view returns (G1Point memory r) {
        uint256[4] memory input = [a.x, a.y, b.x, b.y];
        bool success;
        assembly ("memory-safe") {
            success := staticcall(gas(), 0x06, input, 0x80, r, 0x40)
        }
        if (!success) revert EcOperationFailed();
    }

    /**
     * @dev G1 scalar multiplication via precompile 0x07.
     */
    function _g1ScalarMul(G1Point memory p_, uint256 s) internal view returns (G1Point memory r) {
        uint256[3] memory input = [p_.x, p_.y, s];
        bool success;
        assembly ("memory-safe") {
            success := staticcall(gas(), 0x07, input, 0x60, r, 0x40)
        }
        if (!success) revert EcOperationFailed();
    }

    /**
     * @dev BN254 pairing check for 4 pairs via precompile 0x08.
     *
     * Returns true iff e(a1,a2)·e(b1,b2)·e(c1,c2)·e(d1,d2) == 1.
     * Encoding: [a1.x, a1.y, a2.x[0], a2.x[1], a2.y[0], a2.y[1], b1.x, ...]
     */
    function _pairing4(
        G1Point memory a1, G2Point memory a2,
        G1Point memory b1, G2Point memory b2,
        G1Point memory c1, G2Point memory c2,
        G1Point memory d1, G2Point memory d2
    ) internal view returns (bool) {
        uint256[24] memory input;

        // Pair 1: (a1, a2)
        input[0]  = a1.x;
        input[1]  = a1.y;
        input[2]  = a2.x[0];
        input[3]  = a2.x[1];
        input[4]  = a2.y[0];
        input[5]  = a2.y[1];

        // Pair 2: (b1, b2)
        input[6]  = b1.x;
        input[7]  = b1.y;
        input[8]  = b2.x[0];
        input[9]  = b2.x[1];
        input[10] = b2.y[0];
        input[11] = b2.y[1];

        // Pair 3: (c1, c2)
        input[12] = c1.x;
        input[13] = c1.y;
        input[14] = c2.x[0];
        input[15] = c2.x[1];
        input[16] = c2.y[0];
        input[17] = c2.y[1];

        // Pair 4: (d1, d2)
        input[18] = d1.x;
        input[19] = d1.y;
        input[20] = d2.x[0];
        input[21] = d2.x[1];
        input[22] = d2.y[0];
        input[23] = d2.y[1];

        uint256[1] memory out;
        bool success;
        assembly ("memory-safe") {
            success := staticcall(gas(), 0x08, input, 0x300, out, 0x20)
        }
        if (!success) revert EcOperationFailed();
        return out[0] == 1;
    }

    // ── Internal: proof decoding ──────────────────────────────────────────────

    /**
     * @dev Decode a 256-byte Groth16 proof into a Proof struct.
     *
     * Layout:
     *   [0..31]   A.x
     *   [32..63]  A.y
     *   [64..95]  B.x[0]  (real)
     *   [96..127] B.x[1]  (imag)
     *   [128..159] B.y[0] (real)
     *   [160..191] B.y[1] (imag)
     *   [192..223] C.x
     *   [224..255] C.y
     */
    function _decodeProof(bytes calldata raw) internal pure returns (Proof memory p) {
        p.a.x     = uint256(bytes32(raw[0:32]));
        p.a.y     = uint256(bytes32(raw[32:64]));
        p.b.x[0]  = uint256(bytes32(raw[64:96]));
        p.b.x[1]  = uint256(bytes32(raw[96:128]));
        p.b.y[0]  = uint256(bytes32(raw[128:160]));
        p.b.y[1]  = uint256(bytes32(raw[160:192]));
        p.c.x     = uint256(bytes32(raw[192:224]));
        p.c.y     = uint256(bytes32(raw[224:256]));
    }

    /**
     * @dev Cast bytes32[] public inputs to uint256[] for the pairing check.
     */
    function _castPublicInputs(bytes32[] calldata raw) internal pure returns (uint256[] memory out) {
        out = new uint256[](raw.length);
        for (uint256 i = 0; i < raw.length; ++i) {
            out[i] = uint256(raw[i]);
        }
    }
}
