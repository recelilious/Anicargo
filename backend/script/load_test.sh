#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  backend/script/load_test.sh [options]

Options:
  --base <url>           Base URL (default: http://127.0.0.1:3000)
  --endpoint <path>      Endpoint path (default: /api/library)
  --method <verb>        HTTP method (default: GET)
  --token <token>        Bearer token (optional)
  --concurrency <n>      Concurrency (default: 20)
  --requests <n>         Total requests (default: 200)
  --duration <dur>       Duration (hey only, e.g. 10s)
  --body <json>          Request body (optional)
  --help                 Show help

Notes:
  - Uses 'hey' if available, otherwise falls back to curl + xargs.
  - If --duration is set but 'hey' is missing, the script will use --requests.
EOF
}

BASE_URL="http://127.0.0.1:3000"
ENDPOINT="/api/library"
METHOD="GET"
TOKEN=""
CONCURRENCY=20
REQUESTS=200
DURATION=""
BODY=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base)
      BASE_URL="$2"
      shift 2
      ;;
    --endpoint)
      ENDPOINT="$2"
      shift 2
      ;;
    --method)
      METHOD="$2"
      shift 2
      ;;
    --token)
      TOKEN="$2"
      shift 2
      ;;
    --concurrency)
      CONCURRENCY="$2"
      shift 2
      ;;
    --requests)
      REQUESTS="$2"
      shift 2
      ;;
    --duration)
      DURATION="$2"
      shift 2
      ;;
    --body)
      BODY="$2"
      shift 2
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

URL="${BASE_URL%/}${ENDPOINT}"

if ! [[ "$CONCURRENCY" =~ ^[0-9]+$ ]] || ! [[ "$REQUESTS" =~ ^[0-9]+$ ]]; then
  echo "concurrency and requests must be integers" >&2
  exit 1
fi

if (( CONCURRENCY < 1 )); then
  echo "concurrency must be >= 1" >&2
  exit 1
fi

if (( REQUESTS < 1 )); then
  echo "requests must be >= 1" >&2
  exit 1
fi

if command -v hey >/dev/null 2>&1; then
  echo "Using hey for load test..."
  hey_args=(-c "$CONCURRENCY")
  if [[ -n "$DURATION" ]]; then
    hey_args+=(-z "$DURATION")
  else
    hey_args+=(-n "$REQUESTS")
  fi
  if [[ -n "$TOKEN" ]]; then
    hey_args+=(-H "Authorization: Bearer $TOKEN")
  fi
  if [[ -n "$BODY" ]]; then
    hey_args+=(-d "$BODY")
  fi
  if [[ "$METHOD" != "GET" ]]; then
    hey_args+=(-m "$METHOD")
  fi

  output=$(hey "${hey_args[@]}" "$URL")
  echo "$output"

  rps=$(echo "$output" | awk -F': *' '/Requests\/sec/ {print $2}')
  avg_ms=$(echo "$output" | awk -F': *' '/Average/ {print $2}' | awk '{print $1}')
  p95_ms=$(echo "$output" | awk -F': *' '/95%/ {print $2}' | awk '{print $1}')
  total=$(echo "$output" | awk -F' ' '/Total/ {print $2}' | head -n 1)
  success=$(echo "$output" | awk '/Status code distribution/ {flag=1; next} flag {sum+=$2} END {print sum+0}')
else
  echo "hey not found; using curl + xargs fallback..."
  tmpfile=$(mktemp)
  start_ns=$(date +%s%N)

  curl_cmd=(curl -s -o /dev/null -w "%{http_code} %{time_total}\n")
  if [[ -n "$TOKEN" ]]; then
    curl_cmd+=(-H "Authorization: Bearer $TOKEN")
  fi
  if [[ -n "$BODY" ]]; then
    curl_cmd+=(-H "Content-Type: application/json" -d "$BODY")
  fi
  curl_cmd+=(-X "$METHOD" "$URL")

  seq "$REQUESTS" | xargs -n 1 -P "$CONCURRENCY" -I {} bash -c \
    "$(printf '%q ' "${curl_cmd[@]}")" >> "$tmpfile"

  end_ns=$(date +%s%N)
  elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))

  total=$(wc -l < "$tmpfile" | tr -d ' ')
  success=$(awk '$1 ~ /^2/ {count++} END {print count+0}' "$tmpfile")
  sum_time=$(awk '{sum+=$2} END {print sum+0}' "$tmpfile")
  avg_ms=$(awk -v sum="$sum_time" -v total="$total" 'BEGIN {printf "%.2f", (sum/total)*1000}')

  sort -n -k2 "$tmpfile" > "${tmpfile}.sorted"
  p50_index=$(( (total * 50 + 99) / 100 ))
  p95_index=$(( (total * 95 + 99) / 100 ))
  p50_ms=$(awk -v n="$p50_index" 'NR==n {printf "%.2f", $2*1000}' "${tmpfile}.sorted")
  p95_ms=$(awk -v n="$p95_index" 'NR==n {printf "%.2f", $2*1000}' "${tmpfile}.sorted")
  rps=$(awk -v total="$total" -v ms="$elapsed_ms" 'BEGIN {printf "%.2f", total/(ms/1000)}')

  rm -f "$tmpfile" "${tmpfile}.sorted"

  echo "Total requests: $total"
  echo "Success (2xx): $success"
  echo "Avg latency: ${avg_ms} ms"
  echo "P50 latency: ${p50_ms} ms"
  echo "P95 latency: ${p95_ms} ms"
  echo "Requests/sec: $rps"
fi

cores=$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo 4)
max_in_flight=$(( CONCURRENCY * 4 ))
if (( max_in_flight < 128 )); then max_in_flight=128; fi
if (( max_in_flight > 4096 )); then max_in_flight=4096; fi

db_max=$(( (CONCURRENCY + 1) / 2 ))
if (( db_max < 5 )); then db_max=5; fi
if (( db_max > 50 )); then db_max=50; fi

job_workers=$(( (cores + 1) / 2 ))
if (( job_workers < 2 )); then job_workers=2; fi
if (( job_workers > 8 )); then job_workers=8; fi

hls_workers=$(( cores / 2 ))
if (( hls_workers < 1 )); then hls_workers=1; fi
if (( hls_workers > 4 )); then hls_workers=4; fi

rate_user=$(awk -v rps="${rps:-0}" 'BEGIN {printf "%.0f", rps*60*0.6}')
rate_ip=$(awk -v rps="${rps:-0}" 'BEGIN {printf "%.0f", rps*60*1.0}')
if (( rate_user < 60 )); then rate_user=60; fi
if (( rate_ip < 120 )); then rate_ip=120; fi

cat <<EOF

Recommended settings (based on this run):
  server.max_in_flight = $max_in_flight
  db.max_connections = $db_max
  server.job_workers = $job_workers
  server.max_hls_concurrency = $hls_workers
  server.rate_limit_user_per_minute = $rate_user
  server.rate_limit_ip_per_minute = $rate_ip

Notes:
  - If P95 latency > 1000ms, keep max_in_flight lower or increase DB connections.
  - HLS concurrency should not exceed available CPU cores.
EOF
