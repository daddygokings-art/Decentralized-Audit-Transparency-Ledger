"""AuditLedger Python SDK."""

from .client import AuditLedgerClient
from .models import Event, ContractError, RPCError, AuditLedgerError, Page
from .exceptions import (
    AuditLedgerError,  # re-exported from exceptions as canonical base
    ContractError,     # re-exported for convenience
    RPCError,          # re-exported for convenience
    NetworkError,
    RateLimitError,
    ValidationError,
    MetadataTooLargeError,
    VerificationError,
    HashChainError,
    SignatureVerificationError,
    EventIDMismatchError,
    with_retry,
    log_and_raise,
    RETRYABLE_ERRORS,
)
from .pagination import (
    encode_cursor,
    decode_cursor,
    PaginationState,
    TotalCountCache,
    PageIterator,
    iter_all_items,
    fetch_page_by_cursor,
)
from .verification import (
    compute_event_id,
    verify_event_id,
    compute_chain_hash,
    verify_hash_chain,
    verify_event_signature,
    IntegrityProof,
    generate_integrity_proof,
)

__all__ = [
    # Client
    "AuditLedgerClient",
    # Models
    "Event",
    "Page",
    # Exceptions (issue #249)
    "AuditLedgerError",
    "ContractError",
    "RPCError",
    "NetworkError",
    "RateLimitError",
    "ValidationError",
    "MetadataTooLargeError",
    "VerificationError",
    "HashChainError",
    "SignatureVerificationError",
    "EventIDMismatchError",
    "with_retry",
    "log_and_raise",
    "RETRYABLE_ERRORS",
    # Pagination (issue #248)
    "encode_cursor",
    "decode_cursor",
    "PaginationState",
    "TotalCountCache",
    "PageIterator",
    "iter_all_items",
    "fetch_page_by_cursor",
    # Verification (issue #247)
    "compute_event_id",
    "verify_event_id",
    "compute_chain_hash",
    "verify_hash_chain",
    "verify_event_signature",
    "IntegrityProof",
    "generate_integrity_proof",
]
