#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# run_example.sh — start all 3 services and send a demo request
#
# Usage:
#   bash examples/tracing/run_example.sh
#   # or from py-scouter/:
#   make example.tracing
# ---------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$(dirname "$SCRIPT_DIR")"   # py-scouter/ root

HELLO_PORT=8081
GOODBYE_PORT=8082
ORCHESTRATOR_PORT=8083

pids=()

cleanup() {
    echo ""
    echo "Shutting down services..."
    for pid in "${pids[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
    echo "Done."
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# Start services
# ---------------------------------------------------------------------------
echo "Starting hello-service on port ${HELLO_PORT}..."
uv run uvicorn hello_service:app \
    --app-dir examples/tracing --port "$HELLO_PORT" --log-level warning &
pids+=($!)

echo "Starting goodbye-service on port ${GOODBYE_PORT}..."
uv run uvicorn goodbye_service:app \
    --app-dir examples/tracing --port "$GOODBYE_PORT" --log-level warning &
pids+=($!)

echo "Starting orchestrator-service on port ${ORCHESTRATOR_PORT}..."
uv run uvicorn orchestrator_service:app \
    --app-dir examples/tracing --port "$ORCHESTRATOR_PORT" --log-level warning &
pids+=($!)

# ---------------------------------------------------------------------------
# Wait for all 3 services to be ready
# ---------------------------------------------------------------------------
wait_for() {
    local url="$1"
    local name="$2"
    local retries=30
    echo -n "Waiting for ${name}..."
    while ! curl -sf "${url}" >/dev/null 2>&1; do
        sleep 0.5
        retries=$((retries - 1))
        if [[ $retries -eq 0 ]]; then
            echo " TIMEOUT"
            exit 1
        fi
        echo -n "."
    done
    echo " ready"
}

wait_for "http://localhost:${HELLO_PORT}/docs"        "hello-service"
wait_for "http://localhost:${GOODBYE_PORT}/docs"      "goodbye-service"
wait_for "http://localhost:${ORCHESTRATOR_PORT}/docs" "orchestrator-service"

# ---------------------------------------------------------------------------
# Demo request
# ---------------------------------------------------------------------------
echo ""
echo "Sending demo request to orchestrator..."
echo ""
RESPONSE=$(curl -sf -X POST "http://localhost:${ORCHESTRATOR_PORT}/greet" \
    -H 'Content-Type: application/json' \
    -d '{"name": "Alice"}')

echo "$RESPONSE" | python3 -m json.tool
echo ""

TRACE_ID=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['trace_id'])")
echo "Distributed trace_id: ${TRACE_ID}"
echo ""
echo "Query this trace in Scouter:"
echo "  scouter_client.get_trace_spans_from_filters(TraceFilters(trace_id='${TRACE_ID}'))"
echo ""
echo "Expected span tree:"
echo "  orchestrator.greet            [orchestrator-service]  <root>"
echo "  ├── GET /hello                [hello-service]         <child>"
echo "  │   └── hello.build_greeting  [hello-service]         <grandchild>"
echo "  └── GET /goodbye              [goodbye-service]       <child>"
echo "      └── goodbye.build_farewell [goodbye-service]      <grandchild>"
echo ""
echo "Press Ctrl+C to stop all services."
wait
