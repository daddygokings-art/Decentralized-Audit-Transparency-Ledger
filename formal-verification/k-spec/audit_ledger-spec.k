// ============================================================================
// K-Framework Semantic Specification — AuditLedger Smart Contract
// Issue #378: Formal Verification of AuditLedger
// ============================================================================
//
// This module defines the operational semantics of the AuditLedger contract
// using the K Framework (https://kframework.org/).
//
// It models the key state transitions, initialization invariants, and cap
// enforcement rewrite rules for the Soroban Rust contract.
//
// Run with:
//   kompile audit_ledger-spec.k --backend haskell
//   kprove audit_ledger-spec.k
// ============================================================================

requires "domains.md"

module AUDIT-LEDGER-SYNTAX
    imports DOMAINS-SYNTAX

    // ── Types ────────────────────────────────────────────────────────────────

    syntax Address    ::= r"[A-Z0-9]{56}"  [token]  // Stellar G-account
    syntax Symbol     ::= r"[a-zA-Z_][a-zA-Z0-9_]{0,31}" [token]
    syntax Bytes32    ::= r"0x[0-9a-f]{64}" [token]
    syntax EventIndex ::= Int  // u32 global event index
    syntax Nonce      ::= Int  // u64 per-submitter nonce

    // ── Contract Operations ──────────────────────────────────────────────────

    syntax Stmt ::=
        // Initialization
          "initialize" "(" Address "," Int ")"
        // Logging
        | "logEvent" "(" Address "," Symbol "," Bytes ")"
        | "logEvents" "(" List ")"
        // Governance
        | "setGlobalMaxLogs" "(" Address "," Int ")"
        | "setEventMaxLogs" "(" Address "," Symbol "," Int ")"
        | "removeEventCap" "(" Address "," Symbol ")"
        | "transferOwnership" "(" Address "," Address ")"
        | "setPaused" "(" Address "," Bool ")"
        // Queries (read-only)
        | "totalEvents" "(" ")"
        | "getEvent" "(" Bytes32 ")"
        | "eventCount" "(" Symbol ")"

    // ── Values ───────────────────────────────────────────────────────────────

    syntax KResult ::= Int | Bool | String | Bytes32 | EventRecord

    syntax EventRecord ::= eventRecord(
        index:     Int,
        timestamp: Int,
        eventType: Symbol,
        category:  Symbol,
        submitter: Address,
        metadata:  Bytes,
        version:   Int,
        eventHash: Bytes32,
        prevHash:  Bytes32
    )

    syntax Bytes ::= bytes(String)

endmodule

// ============================================================================

module AUDIT-LEDGER-CONFIG
    imports AUDIT-LEDGER-SYNTAX
    imports MAP
    imports SET
    imports LIST

    // ── Configuration ────────────────────────────────────────────────────────
    //
    // The <k> cell holds the current computation.
    // All contract storage is modelled as K cells mirroring the Soroban
    // DataKey variants that matter for these proofs.

    configuration
        <audit-ledger>
            <k>                $PGM:Stmt                  </k>
            <initialized>      false                      </initialized>
            <owner>            .Address                   </owner>
            <totalEvents>      0                          </totalEvents>
            <globalMaxLogs>    0                          </globalMaxLogs>
            <paused>           false                      </paused>
            // Map from Bytes32 event-ID → EventRecord
            <events>           .Map                       </events>
            // Map from Symbol event-type → Int count
            <eventTypeCounts>  .Map                       </eventTypeCounts>
            // Map from Symbol event-type → Int cap (absent = no cap)
            <eventTypeCaps>    .Map                       </eventTypeCaps>
            // Map from Symbol event-type → Bool (cap removed marker)
            <capRemoved>       .Map                       </capRemoved>
            // Map from Address submitter → Int (last nonce)
            <submitterNonces>  .Map                       </submitterNonces>
            // Sequential list of Bytes32 event-IDs (ordered by insertion)
            <eventOrder>       .List                      </eventOrder>
            // Current ledger timestamp (block time)
            <now>              0                          </now>
        </audit-ledger>

endmodule

// ============================================================================

module AUDIT-LEDGER-RULES
    imports AUDIT-LEDGER-CONFIG
    imports AUDIT-LEDGER-SYNTAX
    imports DOMAINS
    imports MAP
    imports SET
    imports LIST
    imports K-EQUAL

    // ── Helper function: compute a simple event hash ──────────────────────────
    // In production this is SHA-256; here we model it symbolically.
    syntax Bytes32 ::= computeHash(EventRecord) [function, total]
    rule computeHash(_) => "0xDEADBEEF00000000000000000000000000000000000000000000000000000000"
        [simplification]

    syntax Bytes32 ::= genesisHash() [function, total]
    rule genesisHash() => "0x0000000000000000000000000000000000000000000000000000000000000000"

    // ── INITIALIZATION ────────────────────────────────────────────────────────

    // INI-1: Successful initialization.
    rule
        <k> initialize(Owner, MaxLogs) => . </k>
        <initialized>   false => true     </initialized>
        <owner>         _ => Owner        </owner>
        <globalMaxLogs> _ => MaxLogs      </globalMaxLogs>
        <totalEvents>   _ => 0            </totalEvents>
        requires Owner =/=K .Address
        andBool  MaxLogs >Int 0

    // INI-2: Double-initialization reverts (AlreadyInitialized = error code 30).
    rule
        <k> initialize(_, _) => #error(30) </k>
        <initialized> true </initialized>

    // INI-3: Zero-owner rejected (NewOwnerIsZero = error code 6).
    rule
        <k> initialize(.Address, _) => #error(6) </k>
        <initialized> false </initialized>

    // ── LOGGING ───────────────────────────────────────────────────────────────

    // LOG-1: Successful logEvent.
    rule
        <k> logEvent(Submitter, EventType, Meta) => TotalEvents </k>
        <initialized>     true                                            </initialized>
        <paused>          false                                           </paused>
        <totalEvents>     TotalEvents => TotalEvents +Int 1               </totalEvents>
        <globalMaxLogs>   GlobalMax                                       </globalMaxLogs>
        <events>          Events => Events[EventHash <- newEvent]         </events>
        <eventOrder>      Order => Order ListItem(EventHash)              </eventOrder>
        <eventTypeCounts> Counts => Counts[EventType <- (
            {Counts[EventType] orDefault 0} +Int 1
        )]                                                                </eventTypeCounts>
        <now>             Now                                             </now>
        requires TotalEvents <Int GlobalMax
        andBool  notBool eventTypeCapExceeded(EventType, Counts, EventTypeCaps)
        // Build the new event record.
        where
            PrevHash := #if TotalEvents ==Int 0
                            #then genesisHash()
                            #else computeHash({Events[nth(Order, TotalEvents -Int 1)]}:>EventRecord)
                        #fi ,
            EventHash := computeHash(eventRecord(
                TotalEvents, Now, EventType, Symbol(""), Submitter, Meta,
                1, genesisHash(), PrevHash
            )),
            newEvent := eventRecord(
                TotalEvents, Now, EventType, Symbol(""), Submitter, Meta,
                1, EventHash, PrevHash
            )
        <eventTypeCaps> EventTypeCaps </eventTypeCaps>

    // LOG-2: logEvent reverts when contract not initialized.
    rule
        <k> logEvent(_, _, _) => #error(9) </k>
        <initialized> false </initialized>

    // LOG-3: logEvent reverts when contract is paused.
    rule
        <k> logEvent(_, _, _) => #error(13) </k>
        <initialized> true  </initialized>
        <paused>      true  </paused>

    // LOG-4: logEvent reverts when global cap is reached.
    rule
        <k> logEvent(_, _, _) => #error(2) </k>
        <initialized>   true             </initialized>
        <paused>        false            </paused>
        <totalEvents>   TotalEvents      </totalEvents>
        <globalMaxLogs> GlobalMax        </globalMaxLogs>
        requires TotalEvents >=Int GlobalMax

    // LOG-5: logEvent reverts when per-event-type cap is reached.
    rule
        <k> logEvent(_, EventType, _) => #error(3) </k>
        <initialized>     true             </initialized>
        <paused>          false            </paused>
        <totalEvents>     TotalEvents      </totalEvents>
        <globalMaxLogs>   GlobalMax        </globalMaxLogs>
        <eventTypeCounts> Counts           </eventTypeCounts>
        <eventTypeCaps>   EventTypeCaps    </eventTypeCaps>
        requires TotalEvents <Int GlobalMax
        andBool  eventTypeCapExceeded(EventType, Counts, EventTypeCaps)

    // ── CAP ENFORCEMENT ───────────────────────────────────────────────────────

    // Helper predicate: returns true when event-type cap is set and reached.
    syntax Bool ::= eventTypeCapExceeded(Symbol, Map, Map) [function, total]
    rule eventTypeCapExceeded(EventType, Counts, Caps) =>
        EventType in_keys(Caps)
        andBool {Caps[EventType]}:>Int <=Int ({Counts[EventType] orDefault 0}:>Int)

    // GOV-1: setGlobalMaxLogs — owner only.
    rule
        <k> setGlobalMaxLogs(Caller, NewMax) => . </k>
        <owner>         Caller          </owner>
        <totalEvents>   TotalEvents     </totalEvents>
        <globalMaxLogs> _ => NewMax     </globalMaxLogs>
        requires NewMax >=Int TotalEvents

    // GOV-2: setGlobalMaxLogs reverts for non-owner.
    rule
        <k> setGlobalMaxLogs(Caller, _) => #error(1) </k>
        <owner> Owner </owner>
        requires Caller =/=K Owner

    // GOV-3: setGlobalMaxLogs reverts when NewMax < totalEvents.
    rule
        <k> setGlobalMaxLogs(Caller, NewMax) => #error(16) </k>
        <owner>       Caller      </owner>
        <totalEvents> TotalEvents </totalEvents>
        requires NewMax <Int TotalEvents

    // GOV-4: setEventMaxLogs — owner only.
    rule
        <k> setEventMaxLogs(Caller, EventType, NewMax) => . </k>
        <owner>         Caller                                          </owner>
        <eventTypeCaps> Caps => Caps[EventType <- NewMax]              </eventTypeCaps>

    // GOV-5: setEventMaxLogs reverts for non-owner.
    rule
        <k> setEventMaxLogs(Caller, _, _) => #error(1) </k>
        <owner> Owner </owner>
        requires Caller =/=K Owner

    // GOV-6: removeEventCap — owner only.
    rule
        <k> removeEventCap(Caller, EventType) => . </k>
        <owner>       Caller                                                </owner>
        <eventTypeCaps> Caps => Caps[EventType undef]                       </eventTypeCaps>
        <capRemoved>  Removed => Removed[EventType <- true]                 </capRemoved>
        requires EventType in_keys(Caps)

    // GOV-7: removeEventCap reverts when cap not set.
    rule
        <k> removeEventCap(Caller, EventType) => #error(7) </k>
        <owner>         Caller       </owner>
        <eventTypeCaps> Caps         </eventTypeCaps>
        requires notBool EventType in_keys(Caps)

    // GOV-8: removeEventCap reverts for non-owner.
    rule
        <k> removeEventCap(Caller, _) => #error(1) </k>
        <owner> Owner </owner>
        requires Caller =/=K Owner

    // GOV-9: transferOwnership — owner only, non-zero new owner.
    rule
        <k> transferOwnership(Caller, NewOwner) => . </k>
        <owner> Caller => NewOwner </owner>
        requires NewOwner =/=K .Address
        andBool  NewOwner =/=K Caller

    // GOV-10: transferOwnership reverts for non-owner.
    rule
        <k> transferOwnership(Caller, _) => #error(1) </k>
        <owner> Owner </owner>
        requires Caller =/=K Owner

    // GOV-11: transferOwnership reverts for zero address.
    rule
        <k> transferOwnership(Caller, .Address) => #error(6) </k>
        <owner> Caller </owner>

    // GOV-12: transferOwnership reverts for same owner.
    rule
        <k> transferOwnership(Owner, Owner) => #error(15) </k>
        <owner> Owner </owner>

    // GOV-13: setPaused — owner only.
    rule
        <k> setPaused(Caller, Paused) => . </k>
        <owner>  Caller             </owner>
        <paused> _ => Paused        </paused>

    // GOV-14: setPaused reverts for non-owner.
    rule
        <k> setPaused(Caller, _) => #error(1) </k>
        <owner> Owner </owner>
        requires Caller =/=K Owner

    // ── QUERY RULES ───────────────────────────────────────────────────────────

    rule
        <k> totalEvents() => TotalEvents </k>
        <totalEvents> TotalEvents </totalEvents>

    rule
        <k> eventCount(EventType) => {Counts[EventType] orDefault 0} </k>
        <eventTypeCounts> Counts </eventTypeCounts>

    rule
        <k> getEvent(EventId) => {Events[EventId]}:>EventRecord </k>
        <events> Events </events>
        requires EventId in_keys(Events)

    rule
        <k> getEvent(EventId) => #error(4) </k>
        <events> Events </events>
        requires notBool EventId in_keys(Events)

    // ── ERROR HANDLING ────────────────────────────────────────────────────────

    syntax KItem ::= #error(Int)

endmodule

// ============================================================================
// PROOF CLAIMS
// ============================================================================

module AUDIT-LEDGER-SPEC
    imports AUDIT-LEDGER-RULES

    // ── SPEC-1: Initialization correctness ───────────────────────────────────
    //
    // Claim: After initialize(Owner, Max), initialized=true, owner=Owner,
    //        globalMaxLogs=Max, totalEvents=0.

    claim
        <audit-ledger>
            <k> initialize(OWNER:Address, MAX:Int) => . </k>
            <initialized>   false => true    </initialized>
            <owner>         _ => OWNER       </owner>
            <globalMaxLogs> _ => MAX         </globalMaxLogs>
            <totalEvents>   _ => 0           </totalEvents>
            <paused>        false            </paused>
            ...
        </audit-ledger>
        requires OWNER =/=K .Address
        andBool  MAX >Int 0

    // ── SPEC-2: Single logEvent increments totalEvents ────────────────────────

    claim
        <audit-ledger>
            <k> logEvent(SUBMITTER:Address, ETYPE:Symbol, META:Bytes) => TOTAL </k>
            <initialized>   true             </initialized>
            <paused>        false            </paused>
            <totalEvents>   TOTAL => TOTAL +Int 1 </totalEvents>
            <globalMaxLogs> MAX              </globalMaxLogs>
            ...
        </audit-ledger>
        requires TOTAL <Int MAX

    // ── SPEC-3: Cap enforcement — logEvent reverts at cap ────────────────────

    claim
        <audit-ledger>
            <k> logEvent(SUBMITTER:Address, ETYPE:Symbol, META:Bytes) => #error(2) </k>
            <initialized>   true    </initialized>
            <paused>        false   </paused>
            <totalEvents>   TOTAL   </totalEvents>
            <globalMaxLogs> MAX     </globalMaxLogs>
            ...
        </audit-ledger>
        requires TOTAL >=Int MAX

    // ── SPEC-4: Paused contract rejects logEvent ──────────────────────────────

    claim
        <audit-ledger>
            <k> logEvent(_:Address, _:Symbol, _:Bytes) => #error(13) </k>
            <initialized> true  </initialized>
            <paused>      true  </paused>
            ...
        </audit-ledger>

    // ── SPEC-5: Non-owner setGlobalMaxLogs reverts ────────────────────────────

    claim
        <audit-ledger>
            <k> setGlobalMaxLogs(CALLER:Address, _:Int) => #error(1) </k>
            <owner> OWNER:Address </owner>
            ...
        </audit-ledger>
        requires CALLER =/=K OWNER

    // ── SPEC-6: Owner setGlobalMaxLogs succeeds when new max >= totalEvents ───

    claim
        <audit-ledger>
            <k> setGlobalMaxLogs(OWNER:Address, NEWMAX:Int) => . </k>
            <owner>         OWNER               </owner>
            <totalEvents>   TOTAL               </totalEvents>
            <globalMaxLogs> _ => NEWMAX         </globalMaxLogs>
            ...
        </audit-ledger>
        requires NEWMAX >=Int TOTAL

    // ── SPEC-7: transferOwnership changes owner ───────────────────────────────

    claim
        <audit-ledger>
            <k> transferOwnership(OWNER:Address, NEWOWNER:Address) => . </k>
            <owner> OWNER => NEWOWNER </owner>
            ...
        </audit-ledger>
        requires NEWOWNER =/=K .Address
        andBool  NEWOWNER =/=K OWNER

endmodule
