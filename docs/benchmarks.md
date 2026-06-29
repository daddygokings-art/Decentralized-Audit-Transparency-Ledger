# Benchmark Report

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
