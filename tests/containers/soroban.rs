//! # Soroban RPC mock container
//!
//! This module provides a [`SorobanHandle`] that exposes a Soroban-compatible
//! JSON-RPC endpoint for integration testing.
//!
//! ## Strategy
//!
//! A real `stellar/quickstart` container would take 60-90 seconds to boot and
//! requires core network consensus before RPC is usable — too heavy for unit-
//! level integration tests.  Instead we run a lightweight **stub server** using
//! the `wiremock` crate (pure Rust, no extra Docker image needed) that responds
//! to the Soroban JSON-RPC methods used by the audit-ledger toolchain.
//!
//! Tests that need the *full* Stellar stack should use the `#[ignore]` tests in
//! `tests/integration_testnet.rs`.
//!
//! ## Supported stubs
//!
//! | RPC method              | Behaviour |
//! |-------------------------|-----------|
//! | `getHealth`             | Returns `{"status":"healthy"}` |
//! | `getLatestLedger`       | Returns configurable ledger sequence |
//! | `getEvents`             | Returns pre-loaded events from [`SorobanHandle::add_event`] |
//! | `simulateTransaction`   | Returns a successful simulation response |
//! | `sendTransaction`       | Returns a pending-then-success hash |
//! | `getTransaction`        | Returns SUCCESS for any hash |
//! | `getLedgerEntries`      | Returns pre-loaded storage entries |

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{wait_until_ready, CONTAINER_POLL_INTERVAL, CONTAINER_READY_TIMEOUT};

// ── JSON-RPC types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

// ── Stub state ─────────────────────────────────────────────────────────────────

/// Mutable state shared by the stub HTTP server.
#[derive(Default)]
struct StubState {
    /// Events indexed by contract_id.
    events: HashMap<String, Vec<Value>>,
    /// Ledger entries indexed by key.
    ledger_entries: HashMap<String, Value>,
    /// The current "latest ledger" sequence the stub reports.
    latest_ledger: u64,
    /// Transaction statuses by hash.
    tx_statuses: HashMap<String, String>,
}

// ── Public handle ─────────────────────────────────────────────────────────────

/// A lightweight in-process Soroban RPC stub server.
///
/// The server is bound to a random loopback port and responds to the subset of
/// Soroban JSON-RPC methods used in tests.
pub struct SorobanHandle {
    /// The TCP port the stub is listening on.
    pub port: u16,
    /// Base RPC URL.
    pub rpc_url: String,
    /// Shared state for pre-loading responses.
    state: Arc<RwLock<StubState>>,
    /// Shutdown signal sender.
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

impl SorobanHandle {
    // ── Lifecycle ──────────────────────────────────────────────────────────────

    /// Start the in-process stub server on a random loopback port.
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let state = Arc::new(RwLock::new(StubState {
            latest_ledger: 1_000_000,
            ..Default::default()
        }));

        // Bind to an OS-assigned port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let rpc_url = format!("http://127.0.0.1:{port}");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn the stub server task.
        let state_srv = Arc::clone(&state);
        tokio::spawn(async move {
            run_stub_server(listener, state_srv, shutdown_rx).await;
        });

        let handle = Self {
            port,
            rpc_url: rpc_url.clone(),
            state,
            _shutdown: shutdown_tx,
        };

        // Wait until the stub responds to getHealth.
        let attempts = (CONTAINER_READY_TIMEOUT.as_millis()
            / CONTAINER_POLL_INTERVAL.as_millis()) as u32;
        wait_until_ready(attempts, CONTAINER_POLL_INTERVAL, || {
            let url = rpc_url.clone();
            async move {
                let client = HttpClient::new();
                let body = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "getHealth",
                    "params": {}
                });
                client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("soroban stub not ready: {e}"))
            }
        })
        .await
        .map_err(|e| format!("soroban stub never became ready: {e}"))?;

        Ok(handle)
    }

    // ── Pre-loading helpers ────────────────────────────────────────────────────

    /// Pre-load an event so it is returned by `getEvents` calls for
    /// `contract_id`.
    pub fn add_event(&self, contract_id: &str, event: Value) {
        let mut s = self.state.write().unwrap();
        s.events
            .entry(contract_id.to_string())
            .or_default()
            .push(event);
    }

    /// Pre-load a ledger entry (storage key → XDR value).
    pub fn add_ledger_entry(&self, key: &str, value: Value) {
        self.state
            .write()
            .unwrap()
            .ledger_entries
            .insert(key.to_string(), value);
    }

    /// Override the ledger sequence reported by `getLatestLedger`.
    pub fn set_latest_ledger(&self, seq: u64) {
        self.state.write().unwrap().latest_ledger = seq;
    }

    /// Pre-configure a transaction outcome (useful for error-path tests).
    pub fn set_tx_status(&self, hash: &str, status: &str) {
        self.state
            .write()
            .unwrap()
            .tx_statuses
            .insert(hash.to_string(), status.to_string());
    }

    // ── RPC client helpers ─────────────────────────────────────────────────────

    /// Send a raw JSON-RPC request to the stub and return the parsed result.
    pub async fn rpc_call(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let client = HttpClient::new();
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await?
            .json::<JsonRpcResponse>()
            .await?;

        if let Some(err) = resp.error {
            return Err(format!("RPC error {}: {}", err.code, err.message).into());
        }
        Ok(resp.result.unwrap_or(Value::Null))
    }

    /// Call `getHealth` and return `true` if the response is healthy.
    pub async fn is_healthy(&self) -> bool {
        self.rpc_call("getHealth", json!({}))
            .await
            .map(|v| v["status"].as_str() == Some("healthy"))
            .unwrap_or(false)
    }

    /// Call `getLatestLedger` and return the sequence number.
    pub async fn get_latest_ledger(&self) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.rpc_call("getLatestLedger", json!({})).await?;
        result["sequence"]
            .as_u64()
            .ok_or_else(|| "missing sequence field".into())
    }

    /// Call `getEvents` for a contract and return the event array.
    pub async fn get_events(
        &self,
        contract_id: &str,
        start_ledger: u64,
    ) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
        let params = json!({
            "startLedger": start_ledger.to_string(),
            "filters": [{
                "type": "contract",
                "contractIds": [contract_id]
            }]
        });
        let result = self.rpc_call("getEvents", params).await?;
        Ok(result["events"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }
}

// ── Stub HTTP server ───────────────────────────────────────────────────────────

async fn run_stub_server(
    listener: tokio::net::TcpListener,
    state: Arc<RwLock<StubState>>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(handle_connection(stream, state));
                    }
                    Err(e) => {
                        eprintln!("[soroban stub] accept error: {e}");
                        break;
                    }
                }
            }
            _ = &mut shutdown => break,
        }
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    state: Arc<RwLock<StubState>>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 65536];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n == 0 => return,
        Ok(n) => n,
        Err(_) => return,
    };

    let raw = String::from_utf8_lossy(&buf[..n]);

    // Parse the HTTP request body (everything after the blank line).
    let body_start = raw.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let body = &raw[body_start..];

    let response_body = if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(body) {
        dispatch_rpc(req, &state)
    } else {
        json_err(0, -32700, "Parse error")
    };

    let json_str = serde_json::to_string(&response_body).unwrap_or_default();
    let http_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        json_str.len(),
        json_str
    );
    let _ = stream.write_all(http_response.as_bytes()).await;
}

fn dispatch_rpc(req: JsonRpcRequest, state: &Arc<RwLock<StubState>>) -> Value {
    let s = state.read().unwrap();
    match req.method.as_str() {
        "getHealth" => json_ok(req.id, json!({"status": "healthy"})),

        "getLatestLedger" => json_ok(
            req.id,
            json!({
                "id": "0000000000000000",
                "protocolVersion": 22,
                "sequence": s.latest_ledger
            }),
        ),

        "getEvents" => {
            let contract_ids: Vec<String> = req.params["filters"]
                .as_array()
                .and_then(|filters| filters.first())
                .and_then(|f| f["contractIds"].as_array())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let mut events = Vec::new();
            for cid in &contract_ids {
                if let Some(evts) = s.events.get(cid) {
                    events.extend(evts.clone());
                }
            }
            json_ok(
                req.id,
                json!({
                    "events": events,
                    "latestLedger": s.latest_ledger,
                    "cursor": "0000000000000000-0"
                }),
            )
        }

        "getLedgerEntries" => {
            let keys: Vec<String> = req.params["keys"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let entries: Vec<Value> = keys
                .iter()
                .filter_map(|k| {
                    s.ledger_entries.get(k).map(|v| {
                        json!({
                            "key": k,
                            "xdr": v,
                            "lastModifiedLedgerSeq": s.latest_ledger,
                        })
                    })
                })
                .collect();

            json_ok(
                req.id,
                json!({
                    "entries": entries,
                    "latestLedger": s.latest_ledger,
                }),
            )
        }

        "simulateTransaction" => json_ok(
            req.id,
            json!({
                "transactionData": "AAAAAAAAAAAAAAAA",
                "events": [],
                "minResourceFee": "1000000",
                "results": [{
                    "auth": [],
                    "xdr": "AAAAAAAAAAEAAAAB"
                }],
                "latestLedger": s.latest_ledger,
                "cost": { "cpuInsns": "1000", "memBytes": "1000" }
            }),
        ),

        "sendTransaction" => {
            let fake_hash = format!(
                "{:064x}",
                u64::from_be_bytes([1, 2, 3, 4, 5, 6, 7, 8])
            );
            json_ok(
                req.id,
                json!({
                    "hash": fake_hash,
                    "status": "PENDING",
                    "latestLedger": s.latest_ledger,
                    "latestLedgerCloseTime": 0
                }),
            )
        }

        "getTransaction" => {
            let hash = req.params["hash"].as_str().unwrap_or("").to_string();
            let status = s
                .tx_statuses
                .get(&hash)
                .cloned()
                .unwrap_or_else(|| "SUCCESS".to_string());
            json_ok(
                req.id,
                json!({
                    "status": status,
                    "latestLedger": s.latest_ledger,
                    "latestLedgerCloseTime": 0,
                    "createdAt": 0,
                    "applicationOrder": 1,
                    "feeBump": false,
                    "envelopeXdr": "",
                    "resultXdr": "",
                    "resultMetaXdr": "",
                    "ledger": s.latest_ledger,
                }),
            )
        }

        "getFeeStats" => json_ok(
            req.id,
            json!({
                "sorobanInclusionFee": {
                    "max": "200",
                    "min": "100",
                    "mode": "150",
                    "p10": "110",
                    "p20": "120",
                    "p30": "130",
                    "p40": "140",
                    "p50": "150",
                    "p60": "160",
                    "p70": "170",
                    "p80": "180",
                    "p90": "190",
                    "p95": "195",
                    "p99": "199",
                    "transactionCount": "1000",
                    "ledgerCount": 100
                },
                "inclusionFee": {
                    "max": "200",
                    "min": "100",
                    "mode": "150",
                    "p10": "110",
                    "p20": "120",
                    "p30": "130",
                    "p40": "140",
                    "p50": "150",
                    "p60": "160",
                    "p70": "170",
                    "p80": "180",
                    "p90": "190",
                    "p95": "195",
                    "p99": "199",
                    "transactionCount": "1000",
                    "ledgerCount": 100
                },
                "latestLedger": s.latest_ledger
            }),
        ),

        _ => json_err(req.id, -32601, "Method not found"),
    }
}

// ── RPC response helpers ──────────────────────────────────────────────────────

fn json_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn json_err(id: impl Into<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.into(),
        "error": { "code": code, "message": message }
    })
}
