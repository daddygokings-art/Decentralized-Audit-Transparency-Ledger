# Pagination Reference

Full documentation for the `audit_ledger.pagination` module (issue #248).

---

## Cursor encoding

### `encode_cursor(offset: int) -> str`

Encode an integer offset into an opaque cursor string.

```python
from audit_ledger.pagination import encode_cursor, decode_cursor

cursor = encode_cursor(50)  # "50"
offset = decode_cursor(cursor)  # 50
```

### `decode_cursor(cursor: str) -> int`

Decode a cursor string back to an offset integer.

**Raises** `ValueError` if the cursor is malformed or negative.

---

## `PaginationState`

Tracks the current position in a paginated sequence.

```python
from audit_ledger.pagination import PaginationState

state = PaginationState(offset=0, limit=25)
print(state.has_next)  # True (total is unknown)
print(state.cursor)    # "0"

state.advance(25)
print(state.offset)      # 25
print(state.page_number) # 1
```

**Attributes**

| Attribute | Type | Description |
|-----------|------|-------------|
| `offset` | `int` | Current zero-based item offset. |
| `limit` | `int` | Items per page. |
| `total` | `Optional[int]` | Total item count (None until first fetch). |
| `fetched` | `int` | Total items fetched so far. |
| `page_number` | `int` | Pages fetched (1-based after first). |

**Properties**

- `has_next` — `True` if more items remain.
- `cursor` — Opaque cursor string for the current position.

**Methods**

- `advance(page_size)` — Update state after fetching *page_size* items.
- `reset()` — Reset back to the beginning.

---

## `TotalCountCache`

A time-based cache for the total event count.

```python
from audit_ledger.pagination import TotalCountCache

cache = TotalCountCache(ttl_seconds=30)

# Fetches via RPC on first call; uses cache for 30 seconds thereafter
total = cache.get_or_fetch(client.total_events)

# Force a fresh fetch on the next call
cache.invalidate()
```

**Parameters**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `ttl_seconds` | `10.0` | Cache validity window in seconds. |

**Methods**

- `get_or_fetch(fetch_fn)` — Return cached value or call `fetch_fn`.
- `invalidate()` — Clear the cached value.

**Properties**

- `cached_value` — Current cached total, or `None`.

---

## `PageIterator`

Lazily iterates pages returned by a paginated fetch function.

```python
from audit_ledger.pagination import PageIterator

for page in PageIterator(client.get_events, limit=50):
    print(f"Page at offset {page.offset}: {len(page.items)} items")
```

**Constructor**

```python
PageIterator(
    fetch_fn: Callable[[int, int], Page[T]],
    limit: int = 50,
    start_offset: int = 0,
)
```

**Properties**

- `state` — The current `PaginationState`.

**Methods**

- `reset()` — Reset the iterator to the beginning.

---

## `iter_all_items`

Yields every individual item across all pages (flattened).

```python
from audit_ledger.pagination import iter_all_items

for event in iter_all_items(client.get_events, limit=100):
    print(event.event_type)
```

**Signature**

```python
iter_all_items(
    fetch_fn: Callable[[int, int], Page[T]],
    limit: int = 50,
    start_offset: int = 0,
) -> Iterator[T]
```

---

## `fetch_page_by_cursor`

Fetch a single page using an opaque cursor.

```python
from audit_ledger.pagination import fetch_page_by_cursor

# First page
page = fetch_page_by_cursor(client.get_events, cursor=None, limit=25)

# Next page
next_cursor = str(page.offset + len(page.items))
page2 = fetch_page_by_cursor(client.get_events, cursor=next_cursor, limit=25)
```

**Signature**

```python
fetch_page_by_cursor(
    fetch_fn: Callable[[int, int], Page[T]],
    cursor: Optional[str],
    limit: int = 50,
) -> Page[T]
```

**Raises** `ValueError` if the cursor is malformed.
