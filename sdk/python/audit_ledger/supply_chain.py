"""
Supply Chain Module SDK Bindings (issue #872)

Provides Python bindings for cursor-based paginated queries of supply chain
records (brands, products, provenance events, certifications, labor reports,
environmental reports).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, TypeVar, AsyncIterator

from .models import (
    MAX_PAGE_LIMIT,
    DEFAULT_PAGE_LIMIT,
    PageCursor,
    PageLimit,
    Address,
)

# ────────────────────────────────────────────────────────────────────────────
# Supply Chain Types
# ────────────────────────────────────────────────────────────────────────────

T = TypeVar("T")


@dataclass(frozen=True)
class Location:
    """Geographical location or facility."""

    name: str
    country: str
    coordinates: str
    facility_id: str

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "Location":
        return cls(
            name=str(d["name"]),
            country=str(d["country"]),
            coordinates=str(d["coordinates"]),
            facility_id=str(d["facility_id"]),
        )


@dataclass(frozen=True)
class CustodyTransfer:
    """Custody transfer record in supply chain."""

    from_address: Address
    to_address: Address
    timestamp: int
    location: Location
    transfer_notes: str

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "CustodyTransfer":
        return cls(
            from_address=str(d["from_address"]),
            to_address=str(d["to_address"]),
            timestamp=int(d["timestamp"]),
            location=Location.from_dict(d["location"]),
            transfer_notes=str(d["transfer_notes"]),
        )


@dataclass(frozen=True)
class Provenance:
    """Provenance data for product origin tracking."""

    origin_location: Location
    timestamp: int
    raw_material_source: str
    producer_address: Address
    batch_id: str
    chain_of_custody: List[CustodyTransfer]
    is_verified: bool

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "Provenance":
        return cls(
            origin_location=Location.from_dict(d["origin_location"]),
            timestamp=int(d["timestamp"]),
            raw_material_source=str(d["raw_material_source"]),
            producer_address=str(d["producer_address"]),
            batch_id=str(d["batch_id"]),
            chain_of_custody=[
                CustodyTransfer.from_dict(ct) for ct in d["chain_of_custody"]
            ],
            is_verified=bool(d["is_verified"]),
        )


@dataclass(frozen=True)
class AuditEntry:
    """Audit entry for certifications."""

    timestamp: int
    auditor: Address
    status: str
    notes: str

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "AuditEntry":
        return cls(
            timestamp=int(d["timestamp"]),
            auditor=str(d["auditor"]),
            status=str(d["status"]),
            notes=str(d["notes"]),
        )


@dataclass(frozen=True)
class Certification:
    """Certification record (ISO, organic, fair trade, etc.)."""

    cert_id: str
    cert_type: str
    issuer: Address
    issued_date: int
    expiry_date: int
    scope: str
    is_active: bool
    verification_hash: str
    audit_trail: List[AuditEntry]

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "Certification":
        return cls(
            cert_id=str(d["cert_id"]),
            cert_type=str(d["cert_type"]),
            issuer=str(d["issuer"]),
            issued_date=int(d["issued_date"]),
            expiry_date=int(d["expiry_date"]),
            scope=str(d["scope"]),
            is_active=bool(d["is_active"]),
            verification_hash=str(d["verification_hash"]),
            audit_trail=[AuditEntry.from_dict(ae) for ae in d["audit_trail"]],
        )


@dataclass(frozen=True)
class LaborConditions:
    """Labor conditions report."""

    facility_id: str
    report_date: int
    reporter: Address
    worker_count: int
    wage_compliance: bool
    working_hours_compliance: bool
    child_labor_free: bool
    safety_standards_met: bool
    freedom_of_association: bool
    report_hash: str
    certifications: List[str]

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "LaborConditions":
        return cls(
            facility_id=str(d["facility_id"]),
            report_date=int(d["report_date"]),
            reporter=str(d["reporter"]),
            worker_count=int(d["worker_count"]),
            wage_compliance=bool(d["wage_compliance"]),
            working_hours_compliance=bool(d["working_hours_compliance"]),
            child_labor_free=bool(d["child_labor_free"]),
            safety_standards_met=bool(d["safety_standards_met"]),
            freedom_of_association=bool(d["freedom_of_association"]),
            report_hash=str(d["report_hash"]),
            certifications=[str(c) for c in d["certifications"]],
        )


@dataclass(frozen=True)
class EnvironmentalImpact:
    """Environmental impact report."""

    facility_id: str
    report_period: tuple[int, int]
    carbon_footprint: int
    water_usage: int
    waste_generated: int
    renewable_energy_percent: int
    emissions_reduction_percent: int
    certifications: List[str]
    report_hash: str

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "EnvironmentalImpact":
        period = d["report_period"]
        return cls(
            facility_id=str(d["facility_id"]),
            report_period=(int(period[0]), int(period[1])),
            carbon_footprint=int(d["carbon_footprint"]),
            water_usage=int(d["water_usage"]),
            waste_generated=int(d["waste_generated"]),
            renewable_energy_percent=int(d["renewable_energy_percent"]),
            emissions_reduction_percent=int(d["emissions_reduction_percent"]),
            certifications=[str(c) for c in d["certifications"]],
            report_hash=str(d["report_hash"]),
        )


@dataclass(frozen=True)
class Brand:
    """Brand registration."""

    brand_id: str
    name: str
    owner: Address
    verified: bool
    description: str
    website: str
    support_contact: str

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "Brand":
        return cls(
            brand_id=str(d["brand_id"]),
            name=str(d["name"]),
            owner=str(d["owner"]),
            verified=bool(d["verified"]),
            description=str(d["description"]),
            website=str(d["website"]),
            support_contact=str(d["support_contact"]),
        )


@dataclass(frozen=True)
class ProductSKU:
    """Product SKU tracked for supply chain."""

    sku: str
    brand_id: str
    product_name: str
    description: str
    provenance_id: str
    certifications: List[str]
    labor_reports: List[str]
    environmental_reports: List[str]
    created_date: int
    last_updated: int

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "ProductSKU":
        return cls(
            sku=str(d["sku"]),
            brand_id=str(d["brand_id"]),
            product_name=str(d["product_name"]),
            description=str(d["description"]),
            provenance_id=str(d["provenance_id"]),
            certifications=[str(c) for c in d["certifications"]],
            labor_reports=[str(lr) for lr in d["labor_reports"]],
            environmental_reports=[str(er) for er in d["environmental_reports"]],
            created_date=int(d["created_date"]),
            last_updated=int(d["last_updated"]),
        )


# ────────────────────────────────────────────────────────────────────────────
# Pagination Types
# ────────────────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class SupplyChainQueryFilters:
    """Multi-predicate filter for supply chain queries.

    All predicates are optional; omitted predicates match every record.

    Attributes:
        timestamp_min: Inclusive lower bound on timestamp.
        timestamp_max: Inclusive upper bound on timestamp.
        status: Filter by status (e.g., 'active', 'expired', 'verified').
        submitter: Filter by submitter/creator/reporter address.
        brand_id: Filter by brand ID (for products).
        facility_id: Filter by facility ID (for labor/environmental reports).
    """

    timestamp_min: Optional[int] = None
    timestamp_max: Optional[int] = None
    status: Optional[str] = None
    submitter: Optional[Address] = None
    brand_id: Optional[str] = None
    facility_id: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        """Serialize filters for RPC payload."""
        result: Dict[str, Any] = {}
        if self.timestamp_min is not None:
            result["timestamp_min"] = self.timestamp_min
        if self.timestamp_max is not None:
            result["timestamp_max"] = self.timestamp_max
        if self.status is not None:
            result["status"] = self.status
        if self.submitter is not None:
            result["submitter"] = self.submitter
        if self.brand_id is not None:
            result["brand_id"] = self.brand_id
        if self.facility_id is not None:
            result["facility_id"] = self.facility_id
        return result


@dataclass(frozen=True)
class PaginatedResult:
    """Paginated result wrapper.

    Attributes:
        items: List of items in current page.
        next_cursor: Cursor for next page, or None when last page reached.
        total_count: Total count of items in collection.
    """

    items: List[Any]
    next_cursor: Optional[PageCursor]
    total_count: int

    @classmethod
    def from_dict(
        cls, d: Dict[str, Any], item_parser: Callable[[Dict[str, Any]], T]
    ) -> "PaginatedResult":
        """Parse a paginated result from RPC response.

        Args:
            d: Raw response dictionary.
            item_parser: Function to parse each item dict.

        Returns:
            Parsed paginated result.
        """
        items = [item_parser(item) for item in d["items"]]
        next_cursor_raw = d.get("next_cursor")
        next_cursor = (
            None
            if next_cursor_raw is None
            else PageCursor.from_dict({"value": next_cursor_raw})
        )
        total_count = int(d.get("total_count", 0))
        return cls(items=items, next_cursor=next_cursor, total_count=total_count)


# ────────────────────────────────────────────────────────────────────────────
# Supply Chain Client
# ────────────────────────────────────────────────────────────────────────────


class SupplyChainClient:
    """Supply chain module client for paginated queries.

    Provides cursor-based pagination with deterministic ordering and
    multi-predicate filtering for brands, products, provenance events,
    certifications, labor reports, and environmental reports.

    All queries enforce a maximum page size of 100 items and return a
    ``next_cursor`` token for fetching subsequent pages.

    Example:
        >>> from audit_ledger.supply_chain import SupplyChainClient
        >>>
        >>> client = SupplyChainClient(transport)
        >>>
        >>> # Fetch first page of brands
        >>> page1 = client.list_brands(limit=50)
        >>> print(f"Found {len(page1.items)} brands")
        >>>
        >>> # Fetch next page using cursor
        >>> if page1.next_cursor:
        ...     page2 = client.list_brands(
        ...         cursor=page1.next_cursor,
        ...         limit=50
        ...     )
        >>>
        >>> # Filter products by brand and timestamp range
        >>> products = client.list_products(
        ...     filters=SupplyChainQueryFilters(
        ...         brand_id='acme',
        ...         timestamp_min=1672531200,
        ...         timestamp_max=1704067200
        ...     ),
        ...     limit=25
        ... )
    """

    def __init__(self, transport: Callable[[str, List[Any]], Any]) -> None:
        """Initialize supply chain client.

        Args:
            transport: RPC transport function (method, params) -> result.
        """
        self.transport = transport

    def list_brands(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List brands with cursor-based pagination.

        Returns at most ``limit`` brands (default 50, max 100) in deterministic
        insertion order. Use the returned ``next_cursor`` to fetch subsequent
        pages.

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated brands result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_brands_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, Brand.from_dict)

    def list_products(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List products with cursor-based pagination and filtering.

        Supports filtering by:
        - Brand ID (``brand_id``)
        - Timestamp range (``timestamp_min``, ``timestamp_max``)
        - Submitter address (``submitter``)

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated products result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_products_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, ProductSKU.from_dict)

    def list_provenance_events(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List provenance events with cursor-based pagination and filtering.

        Supports filtering by:
        - Timestamp range (``timestamp_min``, ``timestamp_max``)
        - Submitter/producer address (``submitter``)
        - Verification status (``status``: 'verified' or 'unverified')

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated provenance events result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_provenance_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, Provenance.from_dict)

    def list_certifications(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List certifications with cursor-based pagination and expiry filtering.

        Supports filtering by:
        - Active/expired status (``status``: 'active' or 'expired')
        - Issuer address (``submitter``)
        - Timestamp range for issued date (``timestamp_min``, ``timestamp_max``)

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated certifications result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_certifications_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, Certification.from_dict)

    def list_labor_reports(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List labor reports with cursor-based pagination and facility filtering.

        Supports filtering by:
        - Facility ID (``facility_id``)
        - Reporter address (``submitter``)
        - Timestamp range for report date (``timestamp_min``, ``timestamp_max``)

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated labor reports result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_labor_reports_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, LaborConditions.from_dict)

    def list_environmental_reports(
        self,
        cursor: Optional[PageCursor] = None,
        limit: int = DEFAULT_PAGE_LIMIT,
        filters: Optional[SupplyChainQueryFilters] = None,
    ) -> PaginatedResult:
        """List environmental reports with cursor-based pagination and facility filtering.

        Supports filtering by:
        - Facility ID (``facility_id``)
        - Reporter address (``submitter``)
        - Timestamp range for report period start (``timestamp_min``, ``timestamp_max``)

        Args:
            cursor: Opaque cursor from previous page, or None for first page.
            limit: Requested page size (clamped to MAX_PAGE_LIMIT).
            filters: Optional query filters.

        Returns:
            Paginated environmental reports result.
        """
        limit = self._clamp_limit(limit)
        cursor_dict = cursor.to_dict() if cursor else None
        filters_dict = filters.to_dict() if filters else {}

        result = self.transport(
            "list_environmental_reports_paginated", [cursor_dict, limit, filters_dict]
        )

        return PaginatedResult.from_dict(result, EnvironmentalImpact.from_dict)

    def fetch_all(
        self,
        fetch_page: Callable[[Optional[PageCursor], int], PaginatedResult],
    ) -> List[Any]:
        """Fetch all items from a paginated collection by following cursors.

        **WARNING**: Use with caution on large collections. This will make
        multiple RPC calls until all pages are exhausted. Consider using manual
        pagination with cursors for better control over resource usage.

        Args:
            fetch_page: Function that fetches a single page given cursor and limit.

        Returns:
            List of all items across all pages.
        """
        items: List[Any] = []
        cursor: Optional[PageCursor] = None

        while True:
            page = fetch_page(cursor, MAX_PAGE_LIMIT)
            items.extend(page.items)
            if page.next_cursor is None:
                break
            cursor = page.next_cursor

        return items

    def _clamp_limit(self, requested: int) -> int:
        """Clamp requested limit to valid range."""
        if requested <= 0:
            return DEFAULT_PAGE_LIMIT
        return min(requested, MAX_PAGE_LIMIT)


async def paginate_all_async(
    fetch_page: Callable[
        [Optional[PageCursor], int], Any
    ],  # Should be awaitable but avoiding complex typing
    initial_cursor: Optional[PageCursor] = None,
    page_size: int = DEFAULT_PAGE_LIMIT,
) -> AsyncIterator[PaginatedResult]:
    """Async generator that iterates through all pages of a collection.

    Automatically handles cursor management and yields items one page at a time.

    Args:
        fetch_page: Async function that fetches a single page.
        initial_cursor: Starting cursor, or None for first page.
        page_size: Items per page (clamped to MAX_PAGE_LIMIT).

    Yields:
        PaginatedResult instances until collection is exhausted.

    Example:
        >>> async for page in paginate_all_async(
        ...     client.list_products,
        ...     page_size=50
        ... ):
        ...     print(f"Processing {len(page.items)} products")
        ...     for product in page.items:
        ...         # Process each product
        ...         pass
    """
    cursor = initial_cursor
    page_size = min(page_size, MAX_PAGE_LIMIT)

    while True:
        page = await fetch_page(cursor, page_size)
        yield page
        if page.next_cursor is None:
            break
        cursor = page.next_cursor
