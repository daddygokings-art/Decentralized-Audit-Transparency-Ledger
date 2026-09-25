/**
 * GENERATED FILE — DO NOT EDIT.
 * Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
 * Source of truth: abi/audit-ledger.json
 *
 * Type-safe client for the AuditLedger Soroban contract.
 *
 * The client is transport-agnostic: supply any implementation of
 * `AuditLedgerTransport` (Stellar RPC, a local `Env` in tests, or a mock).
 * Argument and return types are derived from the contract IDL, so a contract
 * change that is not regenerated here fails to compile rather than failing at
 * runtime.
 */

import type * as generated from './types';
import type { ArchiveConfig, ArchiveStats, ArchivedEventRef, ContractStatistics, DedupPolicy, Event, EventHeader, EventVersion, FieldChange, MigrationFunction, NonceState, ProposalAction, Role, Schema, SchemaCompatibility, Snapshot, TtlCleanupStats, VersionComparison } from './types';

/** Transport contract: invokes a contract function and resolves its return value. */
export interface AuditLedgerTransport {
  /**
   * @param method Contract function name exactly as declared on-chain.
   * @param args Positional arguments, already converted to wire form.
   */
  invoke<T = unknown>(method: string, args: unknown[]): Promise<T>;
}

/** Options accepted by {@link GeneratedAuditLedgerClient}. */
export interface GeneratedClientOptions {
  /** Contract id (StrKey `C…`) or contract address to invoke. */
  contractId: string;
  transport: AuditLedgerTransport;
}

/** Generated, type-safe client for the AuditLedger contract. */
export class GeneratedAuditLedgerClient {
  readonly contractId: string;
  private readonly transport: AuditLedgerTransport;

  constructor(options: GeneratedClientOptions) {
    this.contractId = options.contractId;
    this.transport = options.transport;
  }

  /**
   * State-changing contract function `initialize`.
   * @param owners wire name `owners` (vec<address>)
   * @param globalMaxLogs wire name `global_max_logs` (u32)
   * @param maxMetadataBytes wire name `max_metadata_bytes` (u32)
   */
  async initialize(owners: Array<string>, globalMaxLogs: number, maxMetadataBytes: number): Promise<void> {
    await this.transport.invoke('initialize', [owners, globalMaxLogs, maxMetadataBytes]);
  }

  /**
   * State-changing contract function `log_events`.
   * @param events wire name `events` (vec<tuple<address,symbol,bytes>>)
   * @returns vec<u32>
   */
  async logEvents(events: Array<[string, string, string]>): Promise<Array<number>> {
    return this.transport.invoke<Array<number>>('log_events', [events]);
  }

  /**
   * State-changing contract function `log_event`.
   * @param submitter wire name `submitter` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param metadata wire name `metadata` (bytes)
   * @param category wire name `category` (option<symbol>)
   * @param subEventType wire name `sub_event_type` (option<symbol>)
   * @param force wire name `force` (bool)
   * @returns bytesn<32>
   */
  async logEvent(submitter: string, eventType: string, metadata: string, category: string | null, subEventType: string | null, force: boolean): Promise<string> {
    return this.transport.invoke<string>('log_event', [submitter, eventType, metadata, category, subEventType, force]);
  }

  /**
   * State-changing contract function `log_event_with_hierarchy`.
   * @param submitter wire name `submitter` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param metadata wire name `metadata` (bytes)
   * @param category wire name `category` (option<symbol>)
   * @param subEventType wire name `sub_event_type` (option<symbol>)
   * @param force wire name `force` (bool)
   * @returns bytesn<32>
   */
  async logEventWithHierarchy(submitter: string, eventType: string, metadata: string, category: string | null, subEventType: string | null, force: boolean): Promise<string> {
    return this.transport.invoke<string>('log_event_with_hierarchy', [submitter, eventType, metadata, category, subEventType, force]);
  }

  /**
   * State-changing contract function `log_event_with_nonce`.
   * @param submitter wire name `submitter` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param metadata wire name `metadata` (bytes)
   * @param nonce wire name `nonce` (u32)
   * @returns bytesn<32>
   */
  async logEventWithNonce(submitter: string, eventType: string, metadata: string, nonce: number): Promise<string> {
    return this.transport.invoke<string>('log_event_with_nonce', [submitter, eventType, metadata, nonce]);
  }

  /**
   * Read-only contract function `get_submitter_nonce`.
   * @param submitter wire name `submitter` (address)
   * @returns u32
   */
  async getSubmitterNonce(submitter: string): Promise<number> {
    return this.transport.invoke<number>('get_submitter_nonce', [submitter]);
  }

  /**
   * Read-only contract function `get_submitter_nonce_state`.
   * @param submitter wire name `submitter` (address)
   * @returns NonceState
   */
  async getSubmitterNonceState(submitter: string): Promise<NonceState> {
    return this.transport.invoke<NonceState>('get_submitter_nonce_state', [submitter]);
  }

  /**
   * State-changing contract function `set_submitter_nonce_config`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   * @param windowSize wire name `window_size` (u32)
   * @param maxNonce wire name `max_nonce` (u32)
   */
  async setSubmitterNonceConfig(caller: string, submitter: string, windowSize: number, maxNonce: number): Promise<void> {
    await this.transport.invoke('set_submitter_nonce_config', [caller, submitter, windowSize, maxNonce]);
  }

  /**
   * State-changing contract function `reset_submitter_nonce`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   */
  async resetSubmitterNonce(caller: string, submitter: string): Promise<void> {
    await this.transport.invoke('reset_submitter_nonce', [caller, submitter]);
  }

  /**
   * State-changing contract function `set_default_nonce_config`.
   * @param caller wire name `caller` (address)
   * @param windowSize wire name `window_size` (u32)
   * @param maxNonce wire name `max_nonce` (u32)
   */
  async setDefaultNonceConfig(caller: string, windowSize: number, maxNonce: number): Promise<void> {
    await this.transport.invoke('set_default_nonce_config', [caller, windowSize, maxNonce]);
  }

  /**
   * Read-only contract function `get_default_nonce_window_size`.
   * @returns u32
   */
  async getDefaultNonceWindowSize(): Promise<number> {
    return this.transport.invoke<number>('get_default_nonce_window_size', []);
  }

  /**
   * Read-only contract function `get_default_nonce_max_value`.
   * @returns u32
   */
  async getDefaultNonceMaxValue(): Promise<number> {
    return this.transport.invoke<number>('get_default_nonce_max_value', []);
  }

  /**
   * Read-only contract function `total_events`.
   * @returns u32
   */
  async totalEvents(): Promise<number> {
    return this.transport.invoke<number>('total_events', []);
  }

  /**
   * Read-only contract function `get_event_type_count`.
   * @param eventType wire name `event_type` (symbol)
   * @returns u32
   */
  async getEventTypeCount(eventType: string): Promise<number> {
    return this.transport.invoke<number>('get_event_type_count', [eventType]);
  }

  /**
   * Read-only contract function `get_event`.
   * @param id wire name `id` (bytesn<32>)
   * @returns Event
   */
  async getEvent(id: string): Promise<Event> {
    return this.transport.invoke<Event>('get_event', [id]);
  }

  /**
   * Read-only contract function `get_event_metadata`.
   * @param id wire name `id` (bytesn<32>)
   * @returns bytes
   */
  async getEventMetadata(id: string): Promise<string> {
    return this.transport.invoke<string>('get_event_metadata', [id]);
  }

  /**
   * Read-only contract function `get_event_header`.
   * @param id wire name `id` (bytesn<32>)
   * @returns EventHeader
   */
  async getEventHeader(id: string): Promise<EventHeader> {
    return this.transport.invoke<EventHeader>('get_event_header', [id]);
  }

  /**
   * Read-only contract function `get_event_by_order`.
   * @param order wire name `order` (u32)
   * @returns Event
   */
  async getEventByOrder(order: number): Promise<Event> {
    return this.transport.invoke<Event>('get_event_by_order', [order]);
  }

  /**
   * Read-only contract function `event_count`.
   * @param eventType wire name `event_type` (symbol)
   * @returns u32
   */
  async eventCount(eventType: string): Promise<number> {
    return this.transport.invoke<number>('event_count', [eventType]);
  }

  /**
   * Read-only contract function `event_count_by_category`.
   * @param category wire name `category` (symbol)
   * @returns u32
   */
  async eventCountByCategory(category: string): Promise<number> {
    return this.transport.invoke<number>('event_count_by_category', [category]);
  }

  /**
   * Read-only contract function `list_events_by_category`.
   * @param category wire name `category` (symbol)
   * @param start wire name `start` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<EventHeader>
   */
  async listEventsByCategory(category: string, start: number, limit: number): Promise<Array<EventHeader>> {
    return this.transport.invoke<Array<EventHeader>>('list_events_by_category', [category, start, limit]);
  }

  /**
   * State-changing contract function `archive_events`.
   * @param caller wire name `caller` (address)
   * @param cutoffTimestamp wire name `cutoff_timestamp` (u64)
   * @returns u32
   */
  async archiveEvents(caller: string, cutoffTimestamp: number): Promise<number> {
    return this.transport.invoke<number>('archive_events', [caller, cutoffTimestamp]);
  }

  /**
   * Read-only contract function `get_archived_event`.
   * @param id wire name `id` (bytesn<32>)
   * @returns Event
   */
  async getArchivedEvent(id: string): Promise<Event> {
    return this.transport.invoke<Event>('get_archived_event', [id]);
  }

  /**
   * Read-only contract function `get_archived_event_ref`.
   * @param id wire name `id` (bytesn<32>)
   * @returns option<ArchivedEventRef>
   */
  async getArchivedEventRef(id: string): Promise<ArchivedEventRef | null> {
    return this.transport.invoke<ArchivedEventRef | null>('get_archived_event_ref', [id]);
  }

  /**
   * Read-only contract function `verify_archived_event_checksum`.
   * @param id wire name `id` (bytesn<32>)
   * @param candidate wire name `candidate` (Event)
   * @returns bool
   */
  async verifyArchivedEventChecksum(id: string, candidate: Event): Promise<boolean> {
    return this.transport.invoke<boolean>('verify_archived_event_checksum', [id, candidate]);
  }

  /**
   * Read-only contract function `get_archive_stats`.
   * @returns ArchiveStats
   */
  async getArchiveStats(): Promise<ArchiveStats> {
    return this.transport.invoke<ArchiveStats>('get_archive_stats', []);
  }

  /**
   * Read-only contract function `get_archived_event_count`.
   * @returns u32
   */
  async getArchivedEventCount(): Promise<number> {
    return this.transport.invoke<number>('get_archived_event_count', []);
  }

  /**
   * Read-only contract function `list_archived_events`.
   * @param start wire name `start` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<EventHeader>
   */
  async listArchivedEvents(start: number, limit: number): Promise<Array<EventHeader>> {
    return this.transport.invoke<Array<EventHeader>>('list_archived_events', [start, limit]);
  }

  /**
   * State-changing contract function `purge_archived_events`.
   * @param caller wire name `caller` (address)
   * @param cutoffTimestamp wire name `cutoff_timestamp` (u64)
   * @param confirm wire name `confirm` (bool)
   * @returns u32
   */
  async purgeArchivedEvents(caller: string, cutoffTimestamp: number, confirm: boolean): Promise<number> {
    return this.transport.invoke<number>('purge_archived_events', [caller, cutoffTimestamp, confirm]);
  }

  /**
   * State-changing contract function `upgrade_contract`.
   * @param caller wire name `caller` (address)
   * @param newWasmHash wire name `new_wasm_hash` (bytesn<32>)
   */
  async upgradeContract(caller: string, newWasmHash: string): Promise<void> {
    await this.transport.invoke('upgrade_contract', [caller, newWasmHash]);
  }

  /**
   * Read-only contract function `get_event_by_type`.
   * @param eventType wire name `event_type` (symbol)
   * @param typeIndex wire name `type_index` (u32)
   * @returns Event
   */
  async getEventByType(eventType: string, typeIndex: number): Promise<Event> {
    return this.transport.invoke<Event>('get_event_by_type', [eventType, typeIndex]);
  }

  /**
   * Read-only contract function `list_events`.
   * @param offset wire name `offset` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async listEvents(offset: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('list_events', [offset, limit]);
  }

  /**
   * Read-only contract function `list_events_by_type`.
   * @param eventType wire name `event_type` (symbol)
   * @param offset wire name `offset` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async listEventsByType(eventType: string, offset: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('list_events_by_type', [eventType, offset, limit]);
  }

  /**
   * Read-only contract function `get_events_by_type`.
   * @param eventType wire name `event_type` (symbol)
   * @param start wire name `start` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async getEventsByType(eventType: string, start: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('get_events_by_type', [eventType, start, limit]);
  }

  /**
   * Read-only contract function `submitter_event_count`.
   * @param submitter wire name `submitter` (address)
   * @returns u32
   */
  async submitterEventCount(submitter: string): Promise<number> {
    return this.transport.invoke<number>('submitter_event_count', [submitter]);
  }

  /**
   * Read-only contract function `get_event_by_submitter`.
   * @param submitter wire name `submitter` (address)
   * @param submitterIndex wire name `submitter_index` (u32)
   * @returns Event
   */
  async getEventBySubmitter(submitter: string, submitterIndex: number): Promise<Event> {
    return this.transport.invoke<Event>('get_event_by_submitter', [submitter, submitterIndex]);
  }

  /**
   * Read-only contract function `get_events_by_submitter`.
   * @param submitter wire name `submitter` (address)
   * @param start wire name `start` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async getEventsBySubmitter(submitter: string, start: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('get_events_by_submitter', [submitter, start, limit]);
  }

  /**
   * Read-only contract function `get_events_by_time_range`.
   * @param startTime wire name `start_time` (u64)
   * @param endTime wire name `end_time` (u64)
   * @param offset wire name `offset` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async getEventsByTimeRange(startTime: number, endTime: number, offset: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('get_events_by_time_range', [startTime, endTime, offset, limit]);
  }

  /**
   * Read-only contract function `search_events`.
   * @param query wire name `query` (bytes)
   * @param offset wire name `offset` (u32)
   * @param limit wire name `limit` (u32)
   * @returns vec<Event>
   */
  async searchEvents(query: string, offset: number, limit: number): Promise<Array<Event>> {
    return this.transport.invoke<Array<Event>>('search_events', [query, offset, limit]);
  }

  /**
   * State-changing contract function `update_event`.
   * @param caller wire name `caller` (address)
   * @param index wire name `index` (u32)
   * @param newMetadata wire name `new_metadata` (bytes)
   * @returns bytesn<32>
   */
  async updateEvent(caller: string, index: number, newMetadata: string): Promise<string> {
    return this.transport.invoke<string>('update_event', [caller, index, newMetadata]);
  }

  /**
   * Read-only contract function `get_event_history`.
   * @param index wire name `index` (u32)
   * @returns vec<EventVersion>
   */
  async getEventHistory(index: number): Promise<Array<EventVersion>> {
    return this.transport.invoke<Array<EventVersion>>('get_event_history', [index]);
  }

  /**
   * State-changing contract function `rollback_event`.
   * @param caller wire name `caller` (address)
   * @param index wire name `index` (u32)
   * @param targetVersion wire name `target_version` (u32)
   * @returns bytesn<32>
   */
  async rollbackEvent(caller: string, index: number, targetVersion: number): Promise<string> {
    return this.transport.invoke<string>('rollback_event', [caller, index, targetVersion]);
  }

  /**
   * Read-only contract function `get_event_version_count`.
   * @param index wire name `index` (u32)
   * @returns u32
   */
  async getEventVersionCount(index: number): Promise<number> {
    return this.transport.invoke<number>('get_event_version_count', [index]);
  }

  /**
   * State-changing contract function `compare_event_versions`.
   * @param index wire name `index` (u32)
   * @param versionA wire name `version_a` (u32)
   * @param versionB wire name `version_b` (u32)
   * @returns i32
   */
  async compareEventVersions(index: number, versionA: number, versionB: number): Promise<number> {
    return this.transport.invoke<number>('compare_event_versions', [index, versionA, versionB]);
  }

  /**
   * Read-only contract function `verify_integrity`.
   * @returns bool
   */
  async verifyIntegrity(): Promise<boolean> {
    return this.transport.invoke<boolean>('verify_integrity', []);
  }

  /**
   * Read-only contract function `verify_integrity_range`.
   * @param from wire name `from` (u32)
   * @param to wire name `to` (u32)
   * @returns bool
   */
  async verifyIntegrityRange(from: number, to: number): Promise<boolean> {
    return this.transport.invoke<boolean>('verify_integrity_range', [from, to]);
  }

  /**
   * State-changing contract function `create_snapshot`.
   * @param caller wire name `caller` (address)
   * @param description wire name `description` (bytes)
   * @returns u32
   */
  async createSnapshot(caller: string, description: string): Promise<number> {
    return this.transport.invoke<number>('create_snapshot', [caller, description]);
  }

  /**
   * Read-only contract function `get_snapshot`.
   * @param snapshotId wire name `snapshot_id` (u32)
   * @returns Snapshot
   */
  async getSnapshot(snapshotId: number): Promise<Snapshot> {
    return this.transport.invoke<Snapshot>('get_snapshot', [snapshotId]);
  }

  /**
   * Read-only contract function `snapshot_count`.
   * @returns u32
   */
  async snapshotCount(): Promise<number> {
    return this.transport.invoke<number>('snapshot_count', []);
  }

  /**
   * Read-only contract function `verify_snapshot`.
   * @param snapshotId wire name `snapshot_id` (u32)
   * @returns bool
   */
  async verifySnapshot(snapshotId: number): Promise<boolean> {
    return this.transport.invoke<boolean>('verify_snapshot', [snapshotId]);
  }

  /**
   * State-changing contract function `cleanup_stale_hashes`.
   * @param caller wire name `caller` (address)
   * @param startIndex wire name `start_index` (u32)
   * @param batchSize wire name `batch_size` (u32)
   * @returns u32
   */
  async cleanupStaleHashes(caller: string, startIndex: number, batchSize: number): Promise<number> {
    return this.transport.invoke<number>('cleanup_stale_hashes', [caller, startIndex, batchSize]);
  }

  /**
   * State-changing contract function `set_global_max_logs`.
   * @param caller wire name `caller` (address)
   * @param newMax wire name `new_max` (u32)
   */
  async setGlobalMaxLogs(caller: string, newMax: number): Promise<void> {
    await this.transport.invoke('set_global_max_logs', [caller, newMax]);
  }

  /**
   * State-changing contract function `set_event_max_logs`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param newMax wire name `new_max` (u32)
   */
  async setEventMaxLogs(caller: string, eventType: string, newMax: number): Promise<void> {
    await this.transport.invoke('set_event_max_logs', [caller, eventType, newMax]);
  }

  /**
   * State-changing contract function `remove_event_cap`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   */
  async removeEventCap(caller: string, eventType: string): Promise<void> {
    await this.transport.invoke('remove_event_cap', [caller, eventType]);
  }

  /**
   * Read-only contract function `has_cap`.
   * @param eventType wire name `event_type` (symbol)
   * @returns bool
   */
  async hasCap(eventType: string): Promise<boolean> {
    return this.transport.invoke<boolean>('has_cap', [eventType]);
  }

  /**
   * State-changing contract function `transfer_ownership`.
   * @param caller wire name `caller` (address)
   * @param newOwner wire name `new_owner` (address)
   */
  async transferOwnership(caller: string, newOwner: string): Promise<void> {
    await this.transport.invoke('transfer_ownership', [caller, newOwner]);
  }

  /**
   * State-changing contract function `set_metadata_max_size`.
   * @param caller wire name `caller` (address)
   * @param maxSize wire name `max_size` (u32)
   */
  async setMetadataMaxSize(caller: string, maxSize: number): Promise<void> {
    await this.transport.invoke('set_metadata_max_size', [caller, maxSize]);
  }

  /**
   * State-changing contract function `set_event_metadata_max_size`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param maxSize wire name `max_size` (u32)
   */
  async setEventMetadataMaxSize(caller: string, eventType: string, maxSize: number): Promise<void> {
    await this.transport.invoke('set_event_metadata_max_size', [caller, eventType, maxSize]);
  }

  /**
   * State-changing contract function `set_metadata_schema`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param schema wire name `schema` (bytes)
   */
  async setMetadataSchema(caller: string, eventType: string, schema: string): Promise<void> {
    await this.transport.invoke('set_metadata_schema', [caller, eventType, schema]);
  }

  /**
   * Read-only contract function `get_metadata_schema`.
   * @param eventType wire name `event_type` (symbol)
   * @returns bytes
   */
  async getMetadataSchema(eventType: string): Promise<string> {
    return this.transport.invoke<string>('get_metadata_schema', [eventType]);
  }

  /**
   * State-changing contract function `register_schema`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param schema wire name `schema` (Schema)
   * @param version wire name `version` (u32)
   */
  async registerSchema(caller: string, eventType: string, schema: Schema, version: number): Promise<void> {
    await this.transport.invoke('register_schema', [caller, eventType, schema, version]);
  }

  /**
   * Read-only contract function `get_schema`.
   * @param eventType wire name `event_type` (symbol)
   * @param version wire name `version` (u32)
   * @returns option<Schema>
   */
  async getSchema(eventType: string, version: number): Promise<Schema | null> {
    return this.transport.invoke<Schema | null>('get_schema', [eventType, version]);
  }

  /**
   * Read-only contract function `list_schemas`.
   * @param eventType wire name `event_type` (symbol)
   * @returns vec<u32>
   */
  async listSchemas(eventType: string): Promise<Array<number>> {
    return this.transport.invoke<Array<number>>('list_schemas', [eventType]);
  }

  /**
   * State-changing contract function `migrate_event_metadata`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param fromVersion wire name `from_version` (u32)
   * @param toVersion wire name `to_version` (u32)
   * @param migrationFn wire name `migration_fn` (MigrationFunction)
   */
  async migrateEventMetadata(caller: string, eventType: string, fromVersion: number, toVersion: number, migrationFn: MigrationFunction): Promise<void> {
    await this.transport.invoke('migrate_event_metadata', [caller, eventType, fromVersion, toVersion, migrationFn]);
  }

  /**
   * Read-only contract function `get_migration_function`.
   * @param eventType wire name `event_type` (symbol)
   * @param fromVersion wire name `from_version` (u32)
   * @param toVersion wire name `to_version` (u32)
   * @returns option<MigrationFunction>
   */
  async getMigrationFunction(eventType: string, fromVersion: number, toVersion: number): Promise<MigrationFunction | null> {
    return this.transport.invoke<MigrationFunction | null>('get_migration_function', [eventType, fromVersion, toVersion]);
  }

  /**
   * State-changing contract function `check_schema_compatibility`.
   * @param left wire name `left` (Schema)
   * @param right wire name `right` (Schema)
   * @returns SchemaCompatibility
   */
  async checkSchemaCompatibility(left: Schema, right: Schema): Promise<SchemaCompatibility> {
    return this.transport.invoke<SchemaCompatibility>('check_schema_compatibility', [left, right]);
  }

  /**
   * Read-only contract function `is_backward_compatible`.
   * @param left wire name `left` (Schema)
   * @param right wire name `right` (Schema)
   * @returns bool
   */
  async isBackwardCompatible(left: Schema, right: Schema): Promise<boolean> {
    return this.transport.invoke<boolean>('is_backward_compatible', [left, right]);
  }

  /**
   * Read-only contract function `is_forward_compatible`.
   * @param left wire name `left` (Schema)
   * @param right wire name `right` (Schema)
   * @returns bool
   */
  async isForwardCompatible(left: Schema, right: Schema): Promise<boolean> {
    return this.transport.invoke<boolean>('is_forward_compatible', [left, right]);
  }

  /**
   * State-changing contract function `set_event_ttl`.
   * @param caller wire name `caller` (address)
   * @param ttlLedgers wire name `ttl_ledgers` (u32)
   */
  async setEventTtl(caller: string, ttlLedgers: number): Promise<void> {
    await this.transport.invoke('set_event_ttl', [caller, ttlLedgers]);
  }

  /**
   * Read-only contract function `get_event_ttl`.
   * @returns u32
   */
  async getEventTtl(): Promise<number> {
    return this.transport.invoke<number>('get_event_ttl', []);
  }

  /**
   * State-changing contract function `cleanup_expired_events`.
   * @param caller wire name `caller` (address)
   * @param startIndex wire name `start_index` (u32)
   * @param batchSize wire name `batch_size` (u32)
   * @returns u32
   */
  async cleanupExpiredEvents(caller: string, startIndex: number, batchSize: number): Promise<number> {
    return this.transport.invoke<number>('cleanup_expired_events', [caller, startIndex, batchSize]);
  }

  /**
   * Read-only contract function `get_cleanup_stats`.
   * @returns TtlCleanupStats
   */
  async getCleanupStats(): Promise<TtlCleanupStats> {
    return this.transport.invoke<TtlCleanupStats>('get_cleanup_stats', []);
  }

  /**
   * State-changing contract function `register_webhook`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param url wire name `url` (bytes)
   * @param secret wire name `secret` (bytes)
   */
  async registerWebhook(caller: string, eventType: string, url: string, secret: string): Promise<void> {
    await this.transport.invoke('register_webhook', [caller, eventType, url, secret]);
  }

  /**
   * State-changing contract function `unregister_webhook`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param url wire name `url` (bytes)
   */
  async unregisterWebhook(caller: string, eventType: string, url: string): Promise<void> {
    await this.transport.invoke('unregister_webhook', [caller, eventType, url]);
  }

  /**
   * Read-only contract function `get_webhooks`.
   * @param eventType wire name `event_type` (symbol)
   * @returns vec<bytes>
   */
  async getWebhooks(eventType: string): Promise<Array<string>> {
    return this.transport.invoke<Array<string>>('get_webhooks', [eventType]);
  }

  /**
   * State-changing contract function `pause`.
   * @param caller wire name `caller` (address)
   */
  async pause(caller: string): Promise<void> {
    await this.transport.invoke('pause', [caller]);
  }

  /**
   * State-changing contract function `unpause`.
   * @param caller wire name `caller` (address)
   */
  async unpause(caller: string): Promise<void> {
    await this.transport.invoke('unpause', [caller]);
  }

  /**
   * Read-only contract function `is_paused`.
   * @returns bool
   */
  async isPaused(): Promise<boolean> {
    return this.transport.invoke<boolean>('is_paused', []);
  }

  /**
   * State-changing contract function `paused_since`.
   * @returns u64
   */
  async pausedSince(): Promise<number> {
    return this.transport.invoke<number>('paused_since', []);
  }

  /**
   * State-changing contract function `set_category_max_len`.
   * @param caller wire name `caller` (address)
   * @param maxLen wire name `max_len` (u32)
   */
  async setCategoryMaxLen(caller: string, maxLen: number): Promise<void> {
    await this.transport.invoke('set_category_max_len', [caller, maxLen]);
  }

  /**
   * State-changing contract function `block_submitter`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   */
  async blockSubmitter(caller: string, submitter: string): Promise<void> {
    await this.transport.invoke('block_submitter', [caller, submitter]);
  }

  /**
   * State-changing contract function `unblock_submitter`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   */
  async unblockSubmitter(caller: string, submitter: string): Promise<void> {
    await this.transport.invoke('unblock_submitter', [caller, submitter]);
  }

  /**
   * State-changing contract function `enable_allowlist_mode`.
   * @param caller wire name `caller` (address)
   */
  async enableAllowlistMode(caller: string): Promise<void> {
    await this.transport.invoke('enable_allowlist_mode', [caller]);
  }

  /**
   * State-changing contract function `disable_allowlist_mode`.
   * @param caller wire name `caller` (address)
   */
  async disableAllowlistMode(caller: string): Promise<void> {
    await this.transport.invoke('disable_allowlist_mode', [caller]);
  }

  /**
   * State-changing contract function `allow_submitter`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   */
  async allowSubmitter(caller: string, submitter: string): Promise<void> {
    await this.transport.invoke('allow_submitter', [caller, submitter]);
  }

  /**
   * State-changing contract function `remove_submitter_from_allowlist`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   */
  async removeSubmitterFromAllowlist(caller: string, submitter: string): Promise<void> {
    await this.transport.invoke('remove_submitter_from_allowlist', [caller, submitter]);
  }

  /**
   * Read-only contract function `get_metadata_max_size`.
   * @param eventType wire name `event_type` (symbol)
   * @returns u32
   */
  async getMetadataMaxSize(eventType: string): Promise<number> {
    return this.transport.invoke<number>('get_metadata_max_size', [eventType]);
  }

  /**
   * Read-only contract function `get_statistics`.
   * @param caller wire name `caller` (address)
   * @returns ContractStatistics
   */
  async getStatistics(caller: string): Promise<ContractStatistics> {
    return this.transport.invoke<ContractStatistics>('get_statistics', [caller]);
  }

  /**
   * State-changing contract function `set_event_emission_mode`.
   * @param caller wire name `caller` (address)
   * @param mode wire name `mode` (u32)
   */
  async setEventEmissionMode(caller: string, mode: number): Promise<void> {
    await this.transport.invoke('set_event_emission_mode', [caller, mode]);
  }

  /**
   * Read-only contract function `get_event_emission_mode`.
   * @returns u32
   */
  async getEventEmissionMode(): Promise<number> {
    return this.transport.invoke<number>('get_event_emission_mode', []);
  }

  /**
   * State-changing contract function `set_low_cost_mode`.
   * @param caller wire name `caller` (address)
   * @param enabled wire name `enabled` (bool)
   */
  async setLowCostMode(caller: string, enabled: boolean): Promise<void> {
    await this.transport.invoke('set_low_cost_mode', [caller, enabled]);
  }

  /**
   * Read-only contract function `is_low_cost_mode`.
   * @returns bool
   */
  async isLowCostMode(): Promise<boolean> {
    return this.transport.invoke<boolean>('is_low_cost_mode', []);
  }

  /**
   * State-changing contract function `set_submitter_rate_limit`.
   * @param caller wire name `caller` (address)
   * @param submitter wire name `submitter` (address)
   * @param maxPerTimestamp wire name `max_per_timestamp` (u32)
   */
  async setSubmitterRateLimit(caller: string, submitter: string, maxPerTimestamp: number): Promise<void> {
    await this.transport.invoke('set_submitter_rate_limit', [caller, submitter, maxPerTimestamp]);
  }

  /**
   * State-changing contract function `compact_storage`.
   * @param caller wire name `caller` (address)
   * @param staleTypes wire name `stale_types` (vec<symbol>)
   * @returns u32
   */
  async compactStorage(caller: string, staleTypes: Array<string>): Promise<number> {
    return this.transport.invoke<number>('compact_storage', [caller, staleTypes]);
  }

  /**
   * State-changing contract function `log_event_signed`.
   * @param submitter wire name `submitter` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param metadata wire name `metadata` (bytes)
   * @param signaturePayload wire name `signature_payload` (bytes)
   * @returns bytesn<32>
   */
  async logEventSigned(submitter: string, eventType: string, metadata: string, signaturePayload: string): Promise<string> {
    return this.transport.invoke<string>('log_event_signed', [submitter, eventType, metadata, signaturePayload]);
  }

  /**
   * Read-only contract function `get_event_signature`.
   * @param eventId wire name `event_id` (bytesn<32>)
   * @returns option<bytes>
   */
  async getEventSignature(eventId: string): Promise<string | null> {
    return this.transport.invoke<string | null>('get_event_signature', [eventId]);
  }

  /**
   * Read-only contract function `find_event_by_content`.
   * @param eventType wire name `event_type` (symbol)
   * @param submitter wire name `submitter` (address)
   * @param metadata wire name `metadata` (bytes)
   * @returns option<Event>
   */
  async findEventByContent(eventType: string, submitter: string, metadata: string): Promise<Event | null> {
    return this.transport.invoke<Event | null>('find_event_by_content', [eventType, submitter, metadata]);
  }

  /**
   * State-changing contract function `add_owner`.
   * @param caller wire name `caller` (address)
   * @param newOwner wire name `new_owner` (address)
   */
  async addOwner(caller: string, newOwner: string): Promise<void> {
    await this.transport.invoke('add_owner', [caller, newOwner]);
  }

  /**
   * State-changing contract function `remove_owner`.
   * @param caller wire name `caller` (address)
   * @param ownerToRemove wire name `owner_to_remove` (address)
   */
  async removeOwner(caller: string, ownerToRemove: string): Promise<void> {
    await this.transport.invoke('remove_owner', [caller, ownerToRemove]);
  }

  /**
   * State-changing contract function `set_required_signatures`.
   * @param caller wire name `caller` (address)
   * @param required wire name `required` (u32)
   */
  async setRequiredSignatures(caller: string, required: number): Promise<void> {
    await this.transport.invoke('set_required_signatures', [caller, required]);
  }

  /**
   * State-changing contract function `submit_proposal`.
   * @param proposer wire name `proposer` (address)
   * @param action wire name `action` (ProposalAction)
   * @param ttlSeconds wire name `ttl_seconds` (u64)
   * @returns u32
   */
  async submitProposal(proposer: string, action: ProposalAction, ttlSeconds: number): Promise<number> {
    return this.transport.invoke<number>('submit_proposal', [proposer, action, ttlSeconds]);
  }

  /**
   * State-changing contract function `approve_proposal`.
   * @param approver wire name `approver` (address)
   * @param proposalId wire name `proposal_id` (u32)
   */
  async approveProposal(approver: string, proposalId: number): Promise<void> {
    await this.transport.invoke('approve_proposal', [approver, proposalId]);
  }

  /**
   * State-changing contract function `execute_proposal`.
   * @param executor wire name `executor` (address)
   * @param proposalId wire name `proposal_id` (u32)
   */
  async executeProposal(executor: string, proposalId: number): Promise<void> {
    await this.transport.invoke('execute_proposal', [executor, proposalId]);
  }

  /**
   * State-changing contract function `set_role`.
   * @param caller wire name `caller` (address)
   * @param target wire name `target` (address)
   * @param role wire name `role` (option<Role>)
   */
  async setRole(caller: string, target: string, role: Role | null): Promise<void> {
    await this.transport.invoke('set_role', [caller, target, role]);
  }

  /**
   * Read-only contract function `get_role`.
   * @param target wire name `target` (address)
   * @returns option<Role>
   */
  async getRole(target: string): Promise<Role | null> {
    return this.transport.invoke<Role | null>('get_role', [target]);
  }

  /**
   * State-changing contract function `enable_rbac`.
   * @param caller wire name `caller` (address)
   * @param enabled wire name `enabled` (bool)
   */
  async enableRbac(caller: string, enabled: boolean): Promise<void> {
    await this.transport.invoke('enable_rbac', [caller, enabled]);
  }

  /**
   * Read-only contract function `is_rbac_enabled`.
   * @returns bool
   */
  async isRbacEnabled(): Promise<boolean> {
    return this.transport.invoke<boolean>('is_rbac_enabled', []);
  }

  /**
   * State-changing contract function `set_dedup_policy`.
   * @param caller wire name `caller` (address)
   * @param policy wire name `policy` (DedupPolicy)
   */
  async setDedupPolicy(caller: string, policy: DedupPolicy): Promise<void> {
    await this.transport.invoke('set_dedup_policy', [caller, policy]);
  }

  /**
   * Read-only contract function `get_dedup_policy`.
   * @returns DedupPolicy
   */
  async getDedupPolicy(): Promise<DedupPolicy> {
    return this.transport.invoke<DedupPolicy>('get_dedup_policy', []);
  }

  /**
   * State-changing contract function `set_dedup_policy_for_type`.
   * @param caller wire name `caller` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param policy wire name `policy` (option<DedupPolicy>)
   */
  async setDedupPolicyForType(caller: string, eventType: string, policy: DedupPolicy | null): Promise<void> {
    await this.transport.invoke('set_dedup_policy_for_type', [caller, eventType, policy]);
  }

  /**
   * Read-only contract function `get_dedup_policy_for_type`.
   * @param eventType wire name `event_type` (symbol)
   * @returns DedupPolicy
   */
  async getDedupPolicyForType(eventType: string): Promise<DedupPolicy> {
    return this.transport.invoke<DedupPolicy>('get_dedup_policy_for_type', [eventType]);
  }

  /**
   * State-changing contract function `log_event_with_custom_key`.
   * @param submitter wire name `submitter` (address)
   * @param eventType wire name `event_type` (symbol)
   * @param metadata wire name `metadata` (bytes)
   * @param category wire name `category` (option<symbol>)
   * @param subEventType wire name `sub_event_type` (option<symbol>)
   * @param force wire name `force` (bool)
   * @param customKey wire name `custom_key` (option<bytesn<32>>)
   * @returns bytesn<32>
   */
  async logEventWithCustomKey(submitter: string, eventType: string, metadata: string, category: string | null, subEventType: string | null, force: boolean, customKey: string | null): Promise<string> {
    return this.transport.invoke<string>('log_event_with_custom_key', [submitter, eventType, metadata, category, subEventType, force, customKey]);
  }

  /**
   * State-changing contract function `cleanup_stale_dedup_entries`.
   * @param caller wire name `caller` (address)
   * @param startIndex wire name `start_index` (u32)
   * @param batchSize wire name `batch_size` (u32)
   * @returns u32
   */
  async cleanupStaleDedupEntries(caller: string, startIndex: number, batchSize: number): Promise<number> {
    return this.transport.invoke<number>('cleanup_stale_dedup_entries', [caller, startIndex, batchSize]);
  }

  /**
   * State-changing contract function `set_archive_config`.
   * @param caller wire name `caller` (address)
   * @param config wire name `config` (ArchiveConfig)
   */
  async setArchiveConfig(caller: string, config: ArchiveConfig): Promise<void> {
    await this.transport.invoke('set_archive_config', [caller, config]);
  }

  /**
   * Read-only contract function `get_archive_config`.
   * @returns option<ArchiveConfig>
   */
  async getArchiveConfig(): Promise<ArchiveConfig | null> {
    return this.transport.invoke<ArchiveConfig | null>('get_archive_config', []);
  }

  /**
   * Read-only contract function `get_event_audit_trail`.
   * @param index wire name `index` (u32)
   * @returns vec<EventVersion>
   */
  async getEventAuditTrail(index: number): Promise<Array<EventVersion>> {
    return this.transport.invoke<Array<EventVersion>>('get_event_audit_trail', [index]);
  }

  /**
   * State-changing contract function `tag_event_version`.
   * @param caller wire name `caller` (address)
   * @param index wire name `index` (u32)
   * @param version wire name `version` (u32)
   * @param tag wire name `tag` (symbol)
   */
  async tagEventVersion(caller: string, index: number, version: number, tag: string): Promise<void> {
    await this.transport.invoke('tag_event_version', [caller, index, version, tag]);
  }

  /**
   * Read-only contract function `get_event_version_tag`.
   * @param index wire name `index` (u32)
   * @param version wire name `version` (u32)
   * @returns option<symbol>
   */
  async getEventVersionTag(index: number, version: number): Promise<string | null> {
    return this.transport.invoke<string | null>('get_event_version_tag', [index, version]);
  }

  /**
   * Read-only contract function `get_event_diff`.
   * @param index wire name `index` (u32)
   * @param fromVersion wire name `from_version` (u32)
   * @param toVersion wire name `to_version` (u32)
   * @returns vec<FieldChange>
   */
  async getEventDiff(index: number, fromVersion: number, toVersion: number): Promise<Array<FieldChange>> {
    return this.transport.invoke<Array<FieldChange>>('get_event_diff', [index, fromVersion, toVersion]);
  }

  /**
   * State-changing contract function `compare_event_versions_detailed`.
   * @param index wire name `index` (u32)
   * @param fromVersion wire name `from_version` (u32)
   * @param toVersion wire name `to_version` (u32)
   * @returns VersionComparison
   */
  async compareEventVersionsDetailed(index: number, fromVersion: number, toVersion: number): Promise<VersionComparison> {
    return this.transport.invoke<VersionComparison>('compare_event_versions_detailed', [index, fromVersion, toVersion]);
  }

  /** All function names this client exposes, for tooling and validation. */
  static readonly functionNames: readonly string[] = [
    'initialize',
    'log_events',
    'log_event',
    'log_event_with_hierarchy',
    'log_event_with_nonce',
    'get_submitter_nonce',
    'get_submitter_nonce_state',
    'set_submitter_nonce_config',
    'reset_submitter_nonce',
    'set_default_nonce_config',
    'get_default_nonce_window_size',
    'get_default_nonce_max_value',
    'total_events',
    'get_event_type_count',
    'get_event',
    'get_event_metadata',
    'get_event_header',
    'get_event_by_order',
    'event_count',
    'event_count_by_category',
    'list_events_by_category',
    'archive_events',
    'get_archived_event',
    'get_archived_event_ref',
    'verify_archived_event_checksum',
    'get_archive_stats',
    'get_archived_event_count',
    'list_archived_events',
    'purge_archived_events',
    'upgrade_contract',
    'get_event_by_type',
    'list_events',
    'list_events_by_type',
    'get_events_by_type',
    'submitter_event_count',
    'get_event_by_submitter',
    'get_events_by_submitter',
    'get_events_by_time_range',
    'search_events',
    'update_event',
    'get_event_history',
    'rollback_event',
    'get_event_version_count',
    'compare_event_versions',
    'verify_integrity',
    'verify_integrity_range',
    'create_snapshot',
    'get_snapshot',
    'snapshot_count',
    'verify_snapshot',
    'cleanup_stale_hashes',
    'set_global_max_logs',
    'set_event_max_logs',
    'remove_event_cap',
    'has_cap',
    'transfer_ownership',
    'set_metadata_max_size',
    'set_event_metadata_max_size',
    'set_metadata_schema',
    'get_metadata_schema',
    'register_schema',
    'get_schema',
    'list_schemas',
    'migrate_event_metadata',
    'get_migration_function',
    'check_schema_compatibility',
    'is_backward_compatible',
    'is_forward_compatible',
    'set_event_ttl',
    'get_event_ttl',
    'cleanup_expired_events',
    'get_cleanup_stats',
    'register_webhook',
    'unregister_webhook',
    'get_webhooks',
    'pause',
    'unpause',
    'is_paused',
    'paused_since',
    'set_category_max_len',
    'block_submitter',
    'unblock_submitter',
    'enable_allowlist_mode',
    'disable_allowlist_mode',
    'allow_submitter',
    'remove_submitter_from_allowlist',
    'get_metadata_max_size',
    'get_statistics',
    'set_event_emission_mode',
    'get_event_emission_mode',
    'set_low_cost_mode',
    'is_low_cost_mode',
    'set_submitter_rate_limit',
    'compact_storage',
    'log_event_signed',
    'get_event_signature',
    'find_event_by_content',
    'add_owner',
    'remove_owner',
    'set_required_signatures',
    'submit_proposal',
    'approve_proposal',
    'execute_proposal',
    'set_role',
    'get_role',
    'enable_rbac',
    'is_rbac_enabled',
    'set_dedup_policy',
    'get_dedup_policy',
    'set_dedup_policy_for_type',
    'get_dedup_policy_for_type',
    'log_event_with_custom_key',
    'cleanup_stale_dedup_entries',
    'set_archive_config',
    'get_archive_config',
    'get_event_audit_trail',
    'tag_event_version',
    'get_event_version_tag',
    'get_event_diff',
    'compare_event_versions_detailed',
  ];
}

export type { generated };
/**
 * contract: AuditLedger v0.1.0
 * idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
 */