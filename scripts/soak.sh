#!/usr/bin/env bash
# Automated, temp-only release soak for DEVELOPMENT.md section 14.3.

set -euo pipefail

export LC_ALL=C

repo_root="$(cd "$(dirname "$0")/.." && pwd -P)"
output_dir="$repo_root/target/soak"
build_target_dir="$repo_root/target/soak-build"
temp_base="${TMPDIR:-/tmp}"
temp_base="${temp_base%/}"
smoke_mode="${HOW_IS_IT_SOAK_SMOKE:-0}"
keep_root="${HOW_IS_IT_SOAK_KEEP_ROOT:-0}"
skip_build="${HOW_IS_IT_SOAK_SKIP_BUILD:-0}"

if [[ "$smoke_mode" == "1" ]]; then
  duration_seconds="${HOW_IS_IT_SOAK_DURATION_SECONDS:-20}"
  sample_seconds="${HOW_IS_IT_SOAK_SAMPLE_SECONDS:-5}"
  idle_seconds="${HOW_IS_IT_SOAK_IDLE_SECONDS:-20}"
  cpu_sample_seconds="${HOW_IS_IT_SOAK_CPU_SAMPLE_SECONDS:-2}"
  settle_seconds="${HOW_IS_IT_SOAK_SETTLE_SECONDS:-2}"
  ui_interval_ms="${HOW_IS_IT_SOAK_UI_INTERVAL_MS:-5000}"
  csv_path="$output_dir/smoke.csv"
  cpu_path="$output_dir/smoke-cpu.csv"
  leak_path="$output_dir/smoke-leaks.txt"
  summary_path="$output_dir/smoke-summary.txt"
  app_log_path="$output_dir/smoke-app.log"
else
  duration_seconds=3600
  sample_seconds=60
  idle_seconds=600
  cpu_sample_seconds=10
  settle_seconds=15
  ui_interval_ms=30000
  csv_path="$output_dir/soak.csv"
  cpu_path="$output_dir/soak-cpu.csv"
  leak_path="$output_dir/soak-leaks.txt"
  summary_path="$output_dir/soak-summary.txt"
  app_log_path="$output_dir/soak-app.log"
fi

for numeric in \
  "$duration_seconds" "$sample_seconds" "$idle_seconds" \
  "$cpu_sample_seconds" "$settle_seconds" "$ui_interval_ms"; do
  if ! [[ "$numeric" =~ ^[1-9][0-9]*$ ]]; then
    printf 'soak: timing values must be positive integers\n' >&2
    exit 2
  fi
done
if (( duration_seconds % sample_seconds != 0 )); then
  printf 'soak: duration must be divisible by the sample interval\n' >&2
  exit 2
fi
if (( idle_seconds % cpu_sample_seconds != 0 )); then
  printf 'soak: idle duration must be divisible by the CPU sample interval\n' >&2
  exit 2
fi
if [[ "$smoke_mode" != "1" && "$skip_build" == "1" ]]; then
  printf 'soak: the required 60-minute run cannot skip its release build\n' >&2
  exit 2
fi

mkdir -p "$output_dir"
: > "$csv_path"
: > "$cpu_path"
: > "$leak_path"
: > "$summary_path"
: > "$app_log_path"
: > "$output_dir/app.stdout.log"
: > "$output_dir/footprint.log"

soak_root="$(mktemp -d "$temp_base/how-is-it-soak.XXXXXX")"
app_pid=""
generator_pid=""
logged_child_pid=""

cleanup() {
  if [[ -n "$generator_pid" ]] && kill -0 "$generator_pid" 2>/dev/null; then
    kill "$generator_pid" 2>/dev/null || true
    wait "$generator_pid" 2>/dev/null || true
  fi
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill -TERM "$app_pid" 2>/dev/null || true
    sleep 1
    kill -KILL "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if [[ "$keep_root" != "1" && -d "$soak_root" ]]; then
    case "$soak_root" in
      "$temp_base"/how-is-it-soak.*) rm -rf -- "$soak_root" ;;
      *) printf 'soak: refusing to remove unexpected path %s\n' "$soak_root" >&2 ;;
    esac
  fi
}
trap cleanup EXIT INT TERM

fail() {
  printf 'soak: FAIL: %s\n' "$1" >&2
  exit 1
}

cpu_time_seconds() {
  local raw
  raw="$(ps -p "$app_pid" -o time=)" || return 1
  printf '%s\n' "$raw" | awk -F: '
    NF == 3 { printf "%.2f", ($1 * 3600) + ($2 * 60) + $3; next }
    NF == 2 { printf "%.2f", ($1 * 60) + $2 }
  '
}

if [[ "$skip_build" != "1" ]]; then
  printf 'soak: building release app bundle\n'
  (cd "$repo_root" && CARGO_TARGET_DIR="$build_target_dir" cargo tauri build --bundles app)
fi

release_bundle="$build_target_dir/release/bundle/macos/How Is It.app"
[[ -d "$release_bundle" ]] || fail "release app bundle was not found"
instrumented_bundle="$soak_root/How Is It.app"
ditto "$release_bundle" "$instrumented_bundle"
entitlements="$soak_root/soak-entitlements.plist"
cat > "$entitlements" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.get-task-allow</key><true/></dict></plist>
EOF
app_binary="$instrumented_bundle/Contents/MacOS/how-is-it"
codesign --force --sign - --entitlements "$entitlements" "$instrumented_bundle"

home_root="$soak_root/home"
data_root="$soak_root/data"
codex_home="$soak_root/codex"
claude_dir="$soak_root/claude"
app_support="$data_root/dev.howisit.app"
logs_dir="$home_root/Library/Logs/dev.howisit.app"
rollout_file="$codex_home/sessions/soak/rollout.jsonl"
claude_file="$claude_dir/projects/soak/session.jsonl"
bridge_file="$app_support/claude-rate-limits.json"
bridge_rewrite_log="$soak_root/bridge-rewrites.log"
bridge_binary="$app_support/bin/howisit-statusline"
fake_codex="$soak_root/fake-codex"
active_marker="$soak_root/active"
fixture_timestamp="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
start_epoch="$(date '+%s')"
reset_epoch="$((start_epoch + 604800))"
daily_date="$(date -u '+%Y-%m-%d')"

mkdir -p \
  "$home_root" "$app_support/bin" "$codex_home/sessions/soak" \
  "$claude_dir/projects/soak" "$logs_dir"
: > "$rollout_file"
: > "$claude_file"
: > "$active_marker"

cat > "$fake_codex" <<EOF
#!/bin/sh
while IFS= read -r line; do
  id=\$(printf '%s\n' "\$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "\$line" in
    *'"method":"initialize"'*)
      printf '{"id":%s,"result":{}}\n' "\$id"
      ;;
    *'"method":"account/rateLimits/read"'*)
      printf '{"id":%s,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":2,"windowDurationMins":10080,"resetsAt":$reset_epoch},"planType":"pro"}}}\n' "\$id"
      ;;
    *'"method":"account/usage/read"'*)
      printf '{"id":%s,"result":{"dailyUsageBuckets":[{"startDate":"$daily_date","tokens":345678}]}}\n' "\$id"
      ;;
  esac
done
EOF
chmod 755 "$fake_codex"

cat > "$app_support/settings.json" <<EOF
{
  "schemaVersion": 1,
  "onboardingCompleted": true,
  "codexPath": "$fake_codex",
  "codexHome": "$codex_home",
  "claudeDir": "$claude_dir",
  "claudeBridgeEnabled": true,
  "claudeOauthEnabled": false,
  "thresholds": [75, 90, 100],
  "notifyOnReset": false,
  "launchAtLogin": false,
  "showDockIcon": false,
  "widget": {"visible": false, "x": null, "y": null, "variant": "Pill", "opacity": 1.0},
  "pollActiveSecs": 120,
  "pollIdleSecs": 600
}
EOF
cat > "$claude_dir/settings.json" <<EOF
{"statusLine":{"type":"command","command":"'$bridge_binary'"}}
EOF
cat > "$app_support/bridge-install.json" <<EOF
{"previousStatusLine":null,"previousStatusLinePresent":false,"installedAt":$((start_epoch - 1))}
EOF
cat > "$app_support/bridge.json" <<'EOF'
{"chainedCommand":null}
EOF
cat > "$bridge_file" <<EOF
{"schema":1,"writtenAt":$start_epoch,"sessionId":"soak","rateLimits":{"five_hour":{"used_percentage":10,"resets_at":$reset_epoch},"seven_day":{"used_percentage":20,"resets_at":$reset_epoch}}}
EOF

printf 'soak: launching isolated release app from %s\n' "$app_binary"
HOW_IS_IT_SOAK_ROOT="$soak_root" \
HOW_IS_IT_SOAK_UI_INTERVAL_MS="$ui_interval_ms" \
CODEX_HOME="$codex_home" \
CLAUDE_CONFIG_DIR="$claude_dir" \
  "$app_binary" >> "$output_dir/app.stdout.log" 2>&1 &
app_pid=$!
printf 'soak: app pid=%s\n' "$app_pid"

for _ in $(seq 1 60); do
  kill -0 "$app_pid" 2>/dev/null || fail "release app exited during startup"
  log_file="$(find "$logs_dir" -maxdepth 1 -type f -name 'how-is-it*.log' -print -quit)"
  if [[ -n "$log_file" ]] && grep -q 'codex app-server started' "$log_file"; then
    logged_child_pid="$(grep 'codex app-server started' "$log_file" | tail -1 | grep -Eo 'pid=[0-9]+' | cut -d= -f2 || true)"
    break
  fi
  sleep 0.5
done
[[ -n "$logged_child_pid" ]] || fail "owned app-server PID was not logged"

node "$repo_root/scripts/soak-generator.mjs" \
  "$rollout_file" "$claude_file" "$bridge_file" "$bridge_rewrite_log" \
  "$fixture_timestamp" "$start_epoch" "$reset_epoch" &
generator_pid=$!

printf 'timestamp,elapsed_seconds,phys_footprint_bytes,child_count,child_pids\n' >> "$csv_path"
sample_total=$((duration_seconds / sample_seconds))
run_started="$(date '+%s')"
for sample_index in $(seq 0 "$sample_total"); do
  target_time=$((run_started + sample_index * sample_seconds))
  while (( $(date '+%s') < target_time )); do
    sleep 1
  done
  kill -0 "$app_pid" 2>/dev/null || fail "release app exited during accelerated events"
  kill -0 "$generator_pid" 2>/dev/null || fail "synthetic event generator exited early"
  footprint_output="$(footprint -p "$app_pid" -f bytes --noCategories 2>&1)" \
    || fail "footprint failed at sample $sample_index"
  printf '%s\n' "$footprint_output" >> "$output_dir/footprint.log"
  physical_bytes="$(printf '%s\n' "$footprint_output" | awk '$1 == "phys_footprint:" { print $2; exit }')"
  [[ "$physical_bytes" =~ ^[0-9]+$ ]] || fail "phys_footprint was not parseable"
  child_lines="$(pgrep -P "$app_pid" -f 'app-server' || true)"
  child_count="$(printf '%s\n' "$child_lines" | awk 'NF { count += 1 } END { print count + 0 }')"
  (( child_count <= 1 )) || fail "more than one owned app-server child exists"
  child_csv="$(printf '%s\n' "$child_lines" | awk 'NF { values = values (values ? ":" : "") $1 } END { print values }')"
  printf '%s,%d,%s,%s,%s\n' \
    "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$((sample_index * sample_seconds))" \
    "$physical_bytes" "$child_count" "$child_csv" >> "$csv_path"
  printf 'soak: sample %d/%d footprint=%s children=%s\n' \
    "$sample_index" "$sample_total" "$physical_bytes" "$child_count"
done

kill "$generator_pid" 2>/dev/null || true
wait "$generator_pid" 2>/dev/null || true
generator_pid=""
rm -f -- "$active_marker"
sleep "$settle_seconds"

cp "$log_file" "$app_log_path"
codex_line_count="$(wc -l < "$rollout_file" | tr -d ' ')"
claude_line_count="$(wc -l < "$claude_file" | tr -d ' ')"
bridge_rewrite_count="$(wc -l < "$bridge_rewrite_log" | tr -d ' ')"
minimum_event_count=$((duration_seconds * 95 / 10))
minimum_bridge_count=$((minimum_event_count / 5))
(( codex_line_count >= minimum_event_count )) \
  || fail "Codex generator produced only $codex_line_count lines"
(( claude_line_count >= minimum_event_count )) \
  || fail "Claude generator produced only $claude_line_count lines"
(( bridge_rewrite_count >= minimum_bridge_count )) \
  || fail "bridge generator completed only $bridge_rewrite_count atomic rewrites"
ui_visible_count="$(grep -c 'automated soak triggered ui_visible' "$app_log_path" || true)"
minimum_ui_visible_count=$((duration_seconds * 1000 / ui_interval_ms - 1))
(( minimum_ui_visible_count < 0 )) && minimum_ui_visible_count=0
(( ui_visible_count >= minimum_ui_visible_count )) \
  || fail "only $ui_visible_count real ui_visible triggers were logged"

printf 'timestamp,elapsed_seconds,cumulative_cpu_seconds\n' >> "$cpu_path"
cpu_samples=$((idle_seconds / cpu_sample_seconds))
idle_started="$(date '+%s')"
cpu_start="$(cpu_time_seconds)" || fail "initial cumulative CPU time could not be read"
for cpu_index in $(seq 0 "$cpu_samples"); do
  target_time=$((idle_started + cpu_index * cpu_sample_seconds))
  while (( $(date '+%s') < target_time )); do
    sleep 1
  done
  kill -0 "$app_pid" 2>/dev/null || fail "release app exited during idle CPU window"
  cumulative_cpu="$(cpu_time_seconds)" || fail "cumulative CPU sample could not be read"
  [[ "$cumulative_cpu" =~ ^[0-9]+([.][0-9]+)?$ ]] || fail "CPU sample was not parseable"
  printf '%s,%d,%s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" \
    "$((cpu_index * cpu_sample_seconds))" "$cumulative_cpu" >> "$cpu_path"
done
cpu_end="$(cpu_time_seconds)" || fail "final cumulative CPU time could not be read"
average_cpu="$(awk -v first="$cpu_start" -v last="$cpu_end" -v elapsed="$idle_seconds" \
  'BEGIN { printf "%.3f", ((last - first) / elapsed) * 100 }')"
awk -v value="$average_cpu" 'BEGIN { exit !(value < 0.5) }' \
  || fail "idle CPU average ${average_cpu}% is not below 0.5%"

leaks "$app_pid" > "$leak_path" 2>&1 || true
platform_appintents_leaks=0
if ! grep -q '0 leaks for 0 total leaked bytes' "$leak_path"; then
  grep -q 'not debuggable' "$leak_path" && fail "leaks could not fully inspect the release app"
  leak_roots="$(grep -Ec '^      [0-9]+ .*ROOT (CYCLE|LEAK)' "$leak_path" || true)"
  appintents_roots="$(grep -Ec '^      [0-9]+ .*ROOT CYCLE: <NSXPCConnection .*Protocol: LNDaemonApplicationInterface  Connection: "com.apple.linkd.autoShortcut"' "$leak_path" || true)"
  [[ "$leak_roots" =~ ^[1-9][0-9]*$ && "$leak_roots" == "$appintents_roots" ]] \
    || fail "leaks reported allocations outside the known macOS AppIntents cycles"
  platform_appintents_leaks="$(sed -n 's/^Process .*: \([0-9][0-9]*\) leaks.*/\1/p' "$leak_path" | tail -1)"
  [[ "$platform_appintents_leaks" =~ ^[1-9][0-9]*$ ]] || fail "leak count was not parseable"
fi

if grep -Eriq 'bearer|accessToken|sk-' "$app_log_path" "$output_dir/app.stdout.log"; then
  fail "isolated soak logs contain a secret-shaped value"
fi

first_elapsed=600
last_elapsed=3600
if [[ "$smoke_mode" == "1" ]]; then
  first_elapsed="$sample_seconds"
  last_elapsed="$duration_seconds"
fi
first_bytes="$(awk -F, -v wanted="$first_elapsed" '$2 == wanted { print $3; exit }' "$csv_path")"
last_bytes="$(awk -F, -v wanted="$last_elapsed" '$2 == wanted { print $3; exit }' "$csv_path")"
[[ -n "$first_bytes" && -n "$last_bytes" ]] || fail "growth endpoints are missing"
growth_percent="$(awk -v first="$first_bytes" -v last="$last_bytes" 'BEGIN { printf "%.3f", ((last - first) / first) * 100 }')"
awk -v value="$growth_percent" 'BEGIN { exit !(value < 10.0) }' \
  || fail "footprint growth ${growth_percent}% is not below 10%"

current_child_pid="$(pgrep -P "$app_pid" -f 'app-server' || true)"
[[ "$current_child_pid" == "$logged_child_pid" ]] \
  || fail "live child PID does not match the PID logged at spawn"

exited_app_pid="$app_pid"
kill -TERM "$exited_app_pid"
for _ in $(seq 1 50); do
  if ! kill -0 "$exited_app_pid" 2>/dev/null; then
    break
  fi
  sleep 0.2
done
if kill -0 "$exited_app_pid" 2>/dev/null; then
  fail "release app did not quit within ten seconds"
fi
wait "$exited_app_pid" 2>/dev/null || true
app_pid=""
if pgrep -P "$exited_app_pid" >/dev/null 2>&1; then
  fail "owned children remain after quit"
fi
if kill -0 "$logged_child_pid" 2>/dev/null; then
  fail "logged app-server child remains after quit"
fi

{
  if [[ "$smoke_mode" == "1" ]]; then
    printf 'mode=smoke\n'
  else
    printf 'mode=full\n'
  fi
  printf 'app_binary=%s\n' "$app_binary"
  printf 'duration_seconds=%s\n' "$duration_seconds"
  printf 'growth_percent=%s\n' "$growth_percent"
  printf 'idle_cpu_percent=%s\n' "$average_cpu"
  printf 'codex_lines=%s\n' "$codex_line_count"
  printf 'claude_lines=%s\n' "$claude_line_count"
  printf 'bridge_rewrites=%s\n' "$bridge_rewrite_count"
  printf 'ui_visible_triggers=%s\n' "$ui_visible_count"
  printf 'logged_child_pid=%s\n' "$logged_child_pid"
  printf 'project_leaks=0\n'
  printf 'platform_appintents_leaks=%s\n' "$platform_appintents_leaks"
  printf 'result=pass\n'
} > "$summary_path"

trap - EXIT INT TERM
cleanup
printf 'soak: PASS growth=%s%% idle_cpu=%s%% project_leaks=0 platform_appintents=%s\n' \
  "$growth_percent" "$average_cpu" "$platform_appintents_leaks"
