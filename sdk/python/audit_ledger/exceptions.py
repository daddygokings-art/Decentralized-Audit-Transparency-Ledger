"""AuditLedger Python SDK — Custom exception classes with error context,
retry logic, and structured logging support.

Issue #249: Add comprehensive error handling.
"""

from __future__ import annotations

import logging
import time
from functools import wraps
from typing import Any, Callable, Optional, Tuple, Type, TypeVar

logger = logging.getLogger("audit_ledger")

F = TypeVar("F", bound=Callable[..., Any])

# ── Base exception hierarchy ──────────────────────────────────────────────────


class AuditLedgerError(Exception):
    """Base exception for all AuditLedger SDK errors.

    All SDK exceptions carry an optional ``context`` dict so callers can
    inspect additional metadata without parsing exception messages.

    Attributes:
        message: Human-readable error description.
        context: Structured dict of additional error context.
    """

    def __init__(self, message: str, context: Optional[dict] = None) -> None:
        super().__init__(message)
        self.message = message
        self.context: dict = context or {}

    def __repr__(self) -> str:  # pragma: no cover
        return f"{type(self).__name__}({self.message!r}, context={self.context!r})"

    def log(self, level: int = logging.ERROR) -> None:
        """Emit a structured log record for this error."""
        logger.log(
            level,
            "%s: %s",
            type(self).__name__,
            self.message,
            extra={"audit_ledger_context": self.context},
        )


# ── Contract / RPC errors ─────────────────────────────────────────────────────


class ContractError(AuditLedgerError):
    """Raised when the Soroban contract returns an on-chain error code.

    Attributes:
        code: Numeric contract error code.
        name: Human-readable error name resolved from ``ERROR_CODES``.
        context: Additional context (method called, params, etc.).
    """

    ERROR_CODES: dict[int, str] = {
        1: "CallerNotOwner",
        2: "GlobalMaxLogsReached",
        3: "EventTypeMaxLogsReached",
        4: "EventDoesNotExist",
        5: "EventTypeIndexOutOfBounds",
        6: "NewOwnerIsZero",
        7: "CapNotSet",
        8: "MetadataTooLarge",
        9: "InvalidSignature",
        10: "ContractPaused",
        11: "RateLimitExceeded",
        14: "NoEventsForType",
        15: "AlreadyInitialized",
    }

    def __init__(self, code: int, context: Optional[dict] = None) -> None:
        self.code = code
        self.name = self.ERROR_CODES.get(code, f"UnknownError({code})")
        ctx = context or {}
        ctx.setdefault("error_code", code)
        ctx.setdefault("error_name", self.name)
        super().__init__(f"ContractError #{code}: {self.name}", context=ctx)


class RPCError(AuditLedgerError):
    """Raised when a Soroban RPC call fails at the network or protocol level.

    Attributes:
        context: May include ``url``, ``method``, ``status_code``, etc.
    """

    def __init__(self, message: str, context: Optional[dict] = None) -> None:
        super().__init__(message, context=context)


class NetworkError(RPCError):
    """Raised on network-level failures (timeouts, connection errors).

    This is a subclass of :class:`RPCError` and is retryable by default.
    """


class RateLimitError(RPCError):
    """Raised when the RPC endpoint returns HTTP 429 or rate-limit contract error.

    Attributes:
        retry_after: Optional seconds to wait before retrying.
    """

    def __init__(
        self,
        message: str,
        retry_after: Optional[float] = None,
        context: Optional[dict] = None,
    ) -> None:
        ctx = context or {}
        if retry_after is not None:
            ctx["retry_after"] = retry_after
        super().__init__(message, context=ctx)
        self.retry_after = retry_after


# ── Validation errors ─────────────────────────────────────────────────────────


class ValidationError(AuditLedgerError):
    """Raised when input parameters fail client-side validation.

    Attributes:
        field: Name of the field that failed validation (if applicable).
    """

    def __init__(
        self,
        message: str,
        field: Optional[str] = None,
        context: Optional[dict] = None,
    ) -> None:
        ctx = context or {}
        if field:
            ctx["field"] = field
        super().__init__(message, context=ctx)
        self.field = field


class MetadataTooLargeError(ValidationError):
    """Raised when event metadata exceeds the configured size limit."""

    def __init__(
        self,
        actual_size: int,
        max_size: int,
        context: Optional[dict] = None,
    ) -> None:
        ctx = context or {}
        ctx.update({"actual_size": actual_size, "max_size": max_size})
        super().__init__(
            f"Metadata size {actual_size} bytes exceeds limit of {max_size} bytes",
            field="metadata",
            context=ctx,
        )
        self.actual_size = actual_size
        self.max_size = max_size


# ── Verification errors ───────────────────────────────────────────────────────


class VerificationError(AuditLedgerError):
    """Raised when client-side event verification fails.

    Attributes:
        event_index: Index of the event that failed verification (if known).
    """

    def __init__(
        self,
        message: str,
        event_index: Optional[int] = None,
        context: Optional[dict] = None,
    ) -> None:
        ctx = context or {}
        if event_index is not None:
            ctx["event_index"] = event_index
        super().__init__(message, context=ctx)
        self.event_index = event_index


class HashChainError(VerificationError):
    """Raised when the hash chain between events is broken."""


class SignatureVerificationError(VerificationError):
    """Raised when an Ed25519 event signature fails verification."""


class EventIDMismatchError(VerificationError):
    """Raised when a recomputed event ID does not match the stored ID."""


# ── Retry logic ───────────────────────────────────────────────────────────────

#: Exception types that are safe to retry automatically.
RETRYABLE_ERRORS: Tuple[Type[AuditLedgerError], ...] = (
    NetworkError,
    RateLimitError,
)


def with_retry(
    max_attempts: int = 3,
    backoff_base: float = 1.0,
    backoff_max: float = 30.0,
    retryable: Tuple[Type[Exception], ...] = RETRYABLE_ERRORS,
    logger: logging.Logger = logger,
) -> Callable[[F], F]:
    """Decorator that retries a function on transient errors with exponential backoff.

    Args:
        max_attempts: Maximum number of total attempts (including the first).
        backoff_base: Base sleep duration in seconds; doubles each attempt.
        backoff_max: Upper bound on the sleep duration in seconds.
        retryable: Tuple of exception classes that trigger a retry.
        logger: Logger instance used for retry warnings.

    Returns:
        A decorator that wraps the target function with retry logic.

    Example::

        @with_retry(max_attempts=3, backoff_base=0.5)
        def fetch_events(client):
            return client.total_events()
    """

    def decorator(func: F) -> F:
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            last_exc: Optional[Exception] = None
            for attempt in range(1, max_attempts + 1):
                try:
                    return func(*args, **kwargs)
                except retryable as exc:  # type: ignore[misc]
                    last_exc = exc
                    if attempt == max_attempts:
                        break
                    # Honour Retry-After if available
                    sleep_time = min(
                        getattr(exc, "retry_after", None) or (backoff_base * (2 ** (attempt - 1))),
                        backoff_max,
                    )
                    logger.warning(
                        "Retryable error on attempt %d/%d for %s: %s. "
                        "Sleeping %.1fs before retry.",
                        attempt,
                        max_attempts,
                        func.__qualname__,
                        exc,
                        sleep_time,
                        extra={"audit_ledger_context": getattr(exc, "context", {})},
                    )
                    time.sleep(sleep_time)
            raise last_exc  # type: ignore[misc]

        return wrapper  # type: ignore[return-value]

    return decorator


# ── Convenience helpers ───────────────────────────────────────────────────────


def log_and_raise(
    exc: AuditLedgerError,
    level: int = logging.ERROR,
) -> None:
    """Log *exc* at the given level then re-raise it.

    Args:
        exc: The :class:`AuditLedgerError` to log and raise.
        level: :mod:`logging` level constant (default: ``logging.ERROR``).

    Raises:
        AuditLedgerError: Always re-raises the provided exception.
    """
    exc.log(level)
    raise exc
