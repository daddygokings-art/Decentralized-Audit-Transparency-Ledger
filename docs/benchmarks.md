# Benchmark Report

> **These observations come from Soroban *test-environment* runs of hand-written
> test cases, not from a benchmark suite.** They describe the test runtime's
> behaviour, which is not the deployed contract's behaviour, and the gas and
> storage operations they call out were never actually captured — see the
> recommendation at the foot of this file, which is the gap
> [`tools/contract-bench`](performance/benchmark-methodology.md) now closes.
>
> For measured figures with a documented method, see
> [Contract benchmark methodology](performance/benchmark-methodology.md).

## Benchmark scenarios

### Sequential logging
- 10,000 events logged sequentially in one submitter stream.
- Verified commit success and total event count.

### Multi-type logging
- 10 event types with 1,000 events each.
- Verified logging across event type diversity.

### Mixed metadata sizes
- Logged three scenarios with 10 B, 100 B, and 1 KB metadata.
- Observed consistent performance across metadata sizes.

### Concurrent submitters
- 100 unique submitter addresses, each logging 100 events.
- Verified that submitter-specific metadata and addresses do not break event sequencing.

### Near-capacity logging
- Logged 9,999 events to reach 99.99% of a 10,000+ capacity.
- Verified final events can still be logged prior to global cap enforcement.

## Observations

- Per-event logging in the Soroban test runtime is effectively linear in the number of writes for the created test cases.
- Event emission and storage write count remain stable when using low metadata sizes, though 1 KB metadata increases storage footprint proportionally.
- No timeouts or panics were observed in the benchmark test cases when run in the Soroban test environment.

## Recommendations

- Add runtime profiling to capture gas and storage operations in the Soroban environment.
- Consider using `low_cost_mode` and event-emission configuration for production deployments with very high event rates.
