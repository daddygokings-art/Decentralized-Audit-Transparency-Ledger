#!/usr/bin/env bash
# Flaky Test Detection with Automatic Quarantine
# Detects flaky tests by running them multiple times and analyzing results
# Integrates with CI for historical tracking

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
QUARANTINE_FILE="$PROJECT_ROOT/tests/quarantine.json"
HISTORY_DIR="$PROJECT_ROOT/.flaky-test-history"
MAX_RUNS="${MAX_RUNS:-5}"
FLAKY_THRESHOLD="${FLAKY_THRESHOLD:-0.2}"
NOTIFICATION_WEBHOOK="${NOTIFICATION_WEBHOOK:-}"

mkdir -p "$HISTORY_DIR"

init_quarantine_file() {
    if [[ ! -f "$QUARANTINE_FILE" ]]; then
        echo '{"quarantined_tests": [], "last_updated": ""}' > "$QUARANTINE_FILE"
    fi
}

get_quarantined_tests() {
    if [[ -f "$QUARANTINE_FILE" ]]; then
        jq -r '.quarantined_tests[]' "$QUARANTINE_FILE" 2>/dev/null || echo ""
    fi
}

is_quarantined() {
    local test_name="$1"
    local quarantined
    quarantined=$(get_quarantined_tests)
    echo "$quarantined" | grep -qx "$test_name"
}

quarantine_test() {
    local test_name="$1"
    local reason="$2"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    init_quarantine_file

    local temp_file
    temp_file=$(mktemp)

    jq --arg name "$test_name" \
       --arg reason "$reason" \
       --arg ts "$timestamp" \
       '.quarantined_tests |= map(select(.name != $name)) + [{"name": $name, "reason": $reason, "quarantined_at": $ts, "status": "quarantined"}] | .last_updated = $ts' \
       "$QUARANTINE_FILE" > "$temp_file" && mv "$temp_file" "$QUARANTINE_FILE"

    echo "QUARANTINED: $test_name - $reason"
    send_notification "quarantine" "$test_name" "$reason"
}

unquarantine_test() {
    local test_name="$1"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    init_quarantine_file

    local temp_file
    temp_file=$(mktemp)

    jq --arg name "$test_name" \
       --arg ts "$timestamp" \
       '.quarantined_tests |= map(select(.name != $name)) | .last_updated = $ts' \
       "$QUARANTINE_FILE" > "$temp_file" && mv "$temp_file" "$temp_file" && mv "$temp_file" "$QUARANTINE_FILE"

    echo "UNQUARANTINED: $test_name"
    send_notification "unquarantine" "$test_name" "Test passed consistently"
}

send_notification() {
    local event_type="$1"
    local test_name="$2"
    local message="$3"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    local notification_file="$HISTORY_DIR/notifications.jsonl"
    echo "{\"timestamp\":\"$timestamp\",\"event\":\"$event_type\",\"test\":\"$test_name\",\"message\":\"$message\"}" >> "$notification_file"

    if [[ -n "$NOTIFICATION_WEBHOOK" ]]; then
        curl -s -X POST "$NOTIFICATION_WEBHOOK" \
            -H "Content-Type: application/json" \
            -d "{\"event\":\"$event_type\",\"test\":\"$test_name\",\"message\":\"$message\",\"timestamp\":\"$timestamp\"}" \
            || true
    fi
}

analyze_root_cause() {
    local test_name="$1"
    local run_results="$2"
    local analysis=""

    local pass_count
    pass_count=$(echo "$run_results" | grep -c "PASS" || true)
    local fail_count
    fail_count=$(echo "$run_results" | grep -c "FAIL" || true)

    if echo "$run_results" | grep -q "timeout\|Timeout\|TIMEOUT"; then
        analysis="TIMEOUT: Test may have timing dependencies or deadlocks"
    elif echo "$run_results" | grep -q "resource\|Resource\|memory\|Memory"; then
        analysis="RESOURCE: Test may have resource leaks or memory issues"
    elif [[ $pass_count -gt 0 && $fail_count -gt 0 ]]; then
        analysis="NONDETERMINISTIC: Test produces different results across runs. Check for: shared state, time-dependent logic, random data, external dependencies"
    elif [[ $fail_count -gt 0 ]]; then
        analysis="CONSISTENT_FAILURE: Test consistently fails - may indicate a real bug"
    else
        analysis="UNKNOWN: Unable to determine root cause from output patterns"
    fi

    echo "$analysis"
}

run_test_multiple_times() {
    local test_name="$1"
    local results=""
    local pass_count=0
    local fail_count=0

    for i in $(seq 1 "$MAX_RUNS"); do
        local output
        local exit_code=0
        output=$(cargo test "$test_name" -- --nocapture 2>&1) || exit_code=$?

        if [[ $exit_code -eq 0 ]]; then
            results+="PASS\n"
            ((pass_count++))
        else
            results+="FAIL: $output\n"
            ((fail_count++))
        fi
    done

    echo -e "$results"
    echo "---SUMMARY---"
    echo "PASS: $pass_count"
    echo "FAIL: $fail_count"
}

calculate_flakiness_rate() {
    local pass_count="$1"
    local fail_count="$2"
    local total=$((pass_count + fail_count))

    if [[ $total -eq 0 ]]; then
        echo "0"
        return
    fi

    if [[ $pass_count -gt 0 && $fail_count -gt 0 ]]; then
        local min_count=$((pass_count < fail_count ? pass_count : fail_count))
        echo "scale=4; $min_count / $total" | bc
    else
        echo "0"
    fi
}

record_history() {
    local test_name="$1"
    local pass_count="$2"
    local fail_count="$3"
    local flakiness_rate="$4"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    local run_id
    run_id=$(date -u +"%Y%m%d_%H%M%S")

    local history_file="$HISTORY_DIR/test-history.jsonl"
    echo "{\"run_id\":\"$run_id\",\"timestamp\":\"$timestamp\",\"test\":\"$test_name\",\"pass_count\":$pass_count,\"fail_count\":$fail_count,\"flakiness_rate\":$flakiness_rate}" >> "$history_file"
}

generate_report() {
    local report_file="$HISTORY_DIR/latest-report.md"

    cat > "$report_file" << EOF
# Flaky Test Detection Report

**Generated:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")

## Quarantined Tests

EOF

    if [[ -f "$QUARANTINE_FILE" ]]; then
        local quarantined_count
        quarantined_count=$(jq '.quarantined_tests | length' "$QUARANTINE_FILE")
        echo "Total quarantined: $quarantined_count" >> "$report_file"
        echo "" >> "$report_file"

        jq -r '.quarantined_tests[] | "- **\(.name)**: \(.reason) (quarantined: \(.quarantined_at))"' "$QUARANTINE_FILE" >> "$report_file" 2>/dev/null || echo "No quarantined tests." >> "$report_file"
    fi

    echo "" >> "$report_file"
    echo "## Historical Trends" >> "$report_file"
    echo "" >> "$report_file"

    if [[ -f "$HISTORY_DIR/test-history.jsonl" ]]; then
        echo "| Run ID | Test | Pass | Fail | Flakiness Rate |" >> "$report_file"
        echo "|--------|------|------|------|----------------|" >> "$report_file"
        tail -20 "$HISTORY_DIR/test-history.jsonl" | while IFS= read -r line; do
            local run_id test pass fail rate
            run_id=$(echo "$line" | jq -r '.run_id')
            test=$(echo "$line" | jq -r '.test')
            pass=$(echo "$line" | jq -r '.pass_count')
            fail=$(echo "$line" | jq -r '.fail_count')
            rate=$(echo "$line" | jq -r '.flakiness_rate')
            echo "| $run_id | $test | $pass | $fail | $rate |" >> "$report_file"
        done
    fi

    echo "Report generated: $report_file"
}

detect_flaky_tests() {
    echo "=== Flaky Test Detection ==="
    echo "Max runs per test: $MAX_RUNS"
    echo "Flaky threshold: $FLAKY_THRESHOLD"
    echo ""

    init_quarantine_file

    local test_list
    test_list=$(cargo test -- --list 2>/dev/null | grep "::" | sed 's/:.*$//' | sort -u || true)

    if [[ -z "$test_list" ]]; then
        echo "No tests found."
        return 0
    fi

    local total_tests=0
    local flaky_found=0

    while IFS= read -r test_name; do
        [[ -z "$test_name" ]] && continue
        ((total_tests++))

        if is_quarantined "$test_name"; then
            echo "SKIP (quarantined): $test_name"
            continue
        fi

        echo "Testing: $test_name ($MAX_RUNS runs)..."

        local run_output
        run_output=$(run_test_multiple_times "$test_name")

        local pass_count fail_count
        pass_count=$(echo "$run_output" | grep -c "^PASS$" || true)
        fail_count=$(echo "$run_output" | grep -c "^FAIL:" || true)

        local flakiness_rate
        flakiness_rate=$(calculate_flakiness_rate "$pass_count" "$fail_count")

        record_history "$test_name" "$pass_count" "$fail_count" "$flakiness_rate"

        local is_flaky
        is_flaky=$(echo "$flakiness_rate > 0" | bc -l)

        if [[ "$is_flaky" -eq 1 ]]; then
            ((flaky_found++))
            local root_cause
            root_cause=$(analyze_root_cause "$test_name" "$run_output")
            echo "  FLAKY DETECTED (rate: $flakiness_rate)"
            echo "  Root cause analysis: $root_cause"
            quarantine_test "$test_name" "Flakiness rate: $flakiness_rate. $root_cause"
        else
            echo "  STABLE (pass: $pass_count, fail: $fail_count)"
        fi
    done <<< "$test_list"

    echo ""
    echo "=== Summary ==="
    echo "Total tests analyzed: $total_tests"
    echo "Flaky tests detected: $flaky_found"

    generate_report
}

check_quarantined_tests() {
    echo "=== Checking Quarantined Tests ==="

    local quarantined
    quarantined=$(get_quarantined_tests)

    if [[ -z "$quarantined" ]]; then
        echo "No quarantined tests to check."
        return 0
    fi

    while IFS= read -r test_name; do
        [[ -z "$test_name" ]] && continue
        echo "Re-checking: $test_name..."

        local run_output
        run_output=$(run_test_multiple_times "$test_name")

        local pass_count fail_count
        pass_count=$(echo "$run_output" | grep -c "^PASS$" || true)
        fail_count=$(echo "$run_output" | grep -c "^FAIL:" || true)

        if [[ $fail_count -eq 0 ]]; then
            echo "  Test now stable - removing from quarantine"
            unquarantine_test "$test_name"
        else
            echo "  Test still flaky (pass: $pass_count, fail: $fail_count)"
        fi
    done <<< "$quarantined"
}

case "${1:-detect}" in
    detect)
        detect_flaky_tests
        ;;
    check)
        check_quarantined_tests
        ;;
    report)
        generate_report
        ;;
    quarantine)
        [[ -z "${2:-}" ]] && { echo "Usage: $0 quarantine <test_name> [reason]"; exit 1; }
        quarantine_test "$2" "${3:-Manual quarantine}"
        ;;
    unquarantine)
        [[ -z "${2:-}" ]] && { echo "Usage: $0 unquarantine <test_name>"; exit 1; }
        unquarantine_test "$2"
        ;;
    *)
        echo "Usage: $0 {detect|check|report|quarantine|unquarantine}"
        echo ""
        echo "Commands:"
        echo "  detect      - Run flaky test detection on all tests"
        echo "  check       - Re-check quarantined tests for stability"
        echo "  report      - Generate flaky test report"
        echo "  quarantine  - Manually quarantine a test"
        echo "  unquarantine - Remove a test from quarantine"
        exit 1
        ;;
esac
