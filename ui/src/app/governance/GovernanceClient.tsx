"use client";
import { useState } from "react";
import {
  SorobanRpc,
  Contract,
  Networks,
  TransactionBuilder,
  Account,
  xdr,
  nativeToScVal,
  Address,
} from "@stellar/stellar-sdk";

const CONTRACT_ID = process.env.NEXT_PUBLIC_CONTRACT_ID ?? "";
const RPC_URL = process.env.NEXT_PUBLIC_RPC_URL ?? "https://soroban-testnet.stellar.org";
const NETWORK = process.env.NEXT_PUBLIC_NETWORK ?? "testnet";
const networkPassphrase = NETWORK === "mainnet" ? Networks.PUBLIC : Networks.TESTNET;

type ActionResult = { ok: true; txHash: string } | { ok: false; error: string };

async function signAndSubmit(
  publicKey: string,
  method: string,
  args: xdr.ScVal[],
  signWith: (tx: string) => Promise<string>
): Promise<ActionResult> {
  try {
    const server = new SorobanRpc.Server(RPC_URL, { allowHttp: RPC_URL.startsWith("http://") });
    const account = await server.getAccount(publicKey);
    const contract = new Contract(CONTRACT_ID);
    const op = contract.call(method, ...args);
    const tx = new TransactionBuilder(account, { fee: "100", networkPassphrase })
      .addOperation(op)
      .setTimeout(30)
      .build();

    const sim = await server.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(sim)) throw new Error(sim.error);

    const prepared = SorobanRpc.assembleTransaction(tx, sim).build();
    const signed = await signWith(prepared.toXDR());
    const result = await server.sendTransaction(
      TransactionBuilder.fromXDR(signed, networkPassphrase) as Parameters<typeof server.sendTransaction>[0]
    );
    return { ok: true, txHash: result.hash };
  } catch (e: unknown) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** Freighter wallet integration */
async function freighterSign(txXdr: string): Promise<string> {
  // @ts-expect-error – Freighter injects window.freighter at runtime
  const { signTransaction } = window.freighter;
  if (!signTransaction) throw new Error("Freighter not installed");
  return signTransaction(txXdr, { networkPassphrase });
}

export default function GovernanceClient() {
  const [walletKey, setWalletKey] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [isError, setIsError] = useState(false);

  function showStatus(msg: string, err = false) {
    setStatus(msg);
    setIsError(err);
  }

  async function connectFreighter() {
    try {
      // @ts-expect-error – Freighter injects window.freighter
      const { getPublicKey, isConnected } = window.freighter;
      if (!isConnected) throw new Error("Freighter extension not found");
      const key = await getPublicKey();
      setWalletKey(key);
      showStatus(`Connected: ${key}`);
    } catch (e: unknown) {
      showStatus(e instanceof Error ? e.message : String(e), true);
    }
  }

  async function submit(method: string, args: xdr.ScVal[]) {
    if (!walletKey) { showStatus("Connect your wallet first", true); return; }
    showStatus("Signing and submitting…");
    const result = await signAndSubmit(walletKey, method, args, freighterSign);
    if (result.ok) showStatus(`✓ Submitted: ${result.txHash}`);
    else showStatus(`✗ Error: ${result.error}`, true);
  }

  // Form state
  const [newGlobalMax, setNewGlobalMax] = useState("");
  const [evtType, setEvtType] = useState("");
  const [evtMax, setEvtMax] = useState("");
  const [removeType, setRemoveType] = useState("");
  const [newOwner, setNewOwner] = useState("");

  const callerVal = walletKey
    ? Address.fromString(walletKey).toScVal()
    : xdr.ScVal.scvVoid();

  return (
    <div>
      {/* Wallet connect */}
      <div className="card mb-6">
        <div className="flex-between">
          <div>
            <p style={{ fontWeight: 600, marginBottom: 4 }}>Wallet</p>
            <p className="text-muted text-sm">
              {walletKey ?? "Not connected. Use Freighter browser extension."}
            </p>
          </div>
          <button onClick={connectFreighter}>
            {walletKey ? "Reconnect" : "Connect Freighter"}
          </button>
        </div>
        {status && (
          <p
            style={{
              marginTop: 12,
              color: isError ? "var(--error)" : "var(--success)",
              fontSize: 13,
              wordBreak: "break-all",
            }}
          >
            {status}
          </p>
        )}
      </div>

      <div className="grid-2 gap-4">
        {/* Set global max */}
        <div className="card">
          <p style={{ fontWeight: 600, marginBottom: 12 }}>Set Global Max Logs</p>
          <input
            placeholder="New max (u32)"
            value={newGlobalMax}
            onChange={(e) => setNewGlobalMax(e.target.value)}
            style={{ marginBottom: 12 }}
          />
          <button
            onClick={() =>
              submit("set_global_max_logs", [
                callerVal,
                nativeToScVal(parseInt(newGlobalMax, 10), { type: "u32" }),
              ])
            }
          >
            Set
          </button>
        </div>

        {/* Set event max */}
        <div className="card">
          <p style={{ fontWeight: 600, marginBottom: 12 }}>Set Event Type Max Logs</p>
          <input
            placeholder="Event type symbol"
            value={evtType}
            onChange={(e) => setEvtType(e.target.value)}
            style={{ marginBottom: 8 }}
          />
          <input
            placeholder="Max (u32)"
            value={evtMax}
            onChange={(e) => setEvtMax(e.target.value)}
            style={{ marginBottom: 12 }}
          />
          <button
            onClick={() =>
              submit("set_event_max_logs", [
                callerVal,
                xdr.ScVal.scvSymbol(evtType),
                nativeToScVal(parseInt(evtMax, 10), { type: "u32" }),
              ])
            }
          >
            Set
          </button>
        </div>

        {/* Remove event cap */}
        <div className="card">
          <p style={{ fontWeight: 600, marginBottom: 12 }}>Remove Event Cap</p>
          <input
            placeholder="Event type symbol"
            value={removeType}
            onChange={(e) => setRemoveType(e.target.value)}
            style={{ marginBottom: 12 }}
          />
          <button
            onClick={() =>
              submit("remove_event_cap", [
                callerVal,
                xdr.ScVal.scvSymbol(removeType),
              ])
            }
          >
            Remove Cap
          </button>
        </div>

        {/* Transfer ownership */}
        <div className="card">
          <p style={{ fontWeight: 600, marginBottom: 12 }}>Transfer Ownership</p>
          <input
            placeholder="New owner address (G…)"
            value={newOwner}
            onChange={(e) => setNewOwner(e.target.value)}
            style={{ marginBottom: 12 }}
          />
          <button
            style={{ background: "var(--warn)", color: "#000" }}
            onClick={() =>
              submit("transfer_ownership", [
                callerVal,
                Address.fromString(newOwner).toScVal(),
              ])
            }
          >
            Transfer Ownership
          </button>
        </div>
      </div>
    </div>
  );
}
