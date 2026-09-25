export { DebugSession } from './engine/session';
export type { Breakpoint, StoppedState, StackFrame, Variable, ScopeDescriptor, StepDirection, StopReason } from './engine/session';
export { evaluate, stringify, typeName, ExpressionError, scopeMembers } from './engine/expr';
export { storageAt, storageDiff, storageRows, deepEqual } from './engine/storage';
export type { StorageDiff, StorageState } from './engine/storage';
export { gasBreakdown, gasOf, checkGasConsistency, remainingGas, accumulate } from './engine/gas';
export type { GasConsistency, GasSlice, GasUsage } from './engine/gas';
export { eventLog, eventsUpTo, eventBreakdown } from './engine/events';
export type { EventBreakdown, EventRecord } from './engine/events';
export { loadTrace, parseTrace, serializeTrace, consistencyWarnings } from './trace/load';
export { validateTrace, assertValidTrace } from './trace/validate';
export type { TraceIssue } from './trace/validate';
export { normalizeRpcResult } from './trace/normalize';
export type { NormalizeInput, RpcResultEnvelope } from './trace/normalize';
export { TRACE_FORMAT_VERSION, storageKeyId, emptyTotals } from './trace/schema';
export type {
  StorageEntry,
  StorageOp,
  StorageScope,
  Step,
  StepCall,
  StepError,
  StepGas,
  StepStorage,
  TraceContract,
  TraceDetail,
  TraceEvent,
  TraceFrame,
  TraceLocation,
  TraceSource,
  TraceTotals,
  TraceTransaction,
  TransactionTrace,
} from './trace/schema';
export { DapServer, sessionFromText } from './dap/server';
export { MessageDecoder, encodeMessage, makeEvent, makeErrorResponse, makeResponse, collectResponses, resetSeq, nextSeq } from './dap/protocol';
export type { DapHandler, DapMessage } from './dap/protocol';
export {
  TraceBuilder,
  buildBatchTrace,
  buildInconsistentTrace,
  buildLogEventTrace,
  buildPanicTrace,
  buildReadWithWritebackTrace,
  scenarioBuilders,
} from './fixtures/builder';

export const VERSION = '1.0.0';
