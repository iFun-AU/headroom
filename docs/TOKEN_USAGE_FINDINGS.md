# Token usage: feasibility findings

> **Status (2026-09-23): superseded by `DEVELOPMENT.md` v2.1.** The finalised spec keeps token history as the Claude / Codex detail-tab charts. Every series is labelled with its scope ("on this Mac" or "account", §7.5). There's no limit or pace line on token charts, and projection is percentage-based only. The fork-safe counting rule and the UTC-day finding below were folded into §2.1 / §2.2 / §7.5. This file is kept as the research record.

> **Author:** AI Assistant · **Date:** 2026-09-23 · **Tested with:** Claude Code 2.1.273, codex-cli 0.154.0, macOS 27.0
> **Question:** Can How Is It show token usage, not just the % of plan limits?

## Short answer

**Yes, partly.** We can show *how many tokens were used* (per hour, per day, per limit window, by type and by model) for both services. We **can't** show *tokens remaining* or *"X of Y tokens"*, because neither provider exposes a token cap for subscription plans. The limits are only published as a used-percentage.

| Question a user might ask | Claude | Codex |
|---|---|---|
| How many tokens did I use today / this hour? | ✅ local logs (this Mac, Claude Code only) | ✅ local session files (this Mac) + account-wide daily totals |
| How many tokens in the current 5 h / weekly window? | ✅ sum local logs since `resets_at − window` | ✅ same, from session files |
| Breakdown: input / output / cache, per model | ✅ | ✅ (input, cached input, output, reasoning) |
| Usage from other devices / web / desktop apps | ❌ not available locally | ⚠️ daily only, via `account/usage/read`, today missing |
| How many tokens are left before the limit? | ❌ no token cap exposed | ❌ no token cap exposed |
| Cost in $ | ⚠️ per-session estimate only (status line); needs a price table | ⚠️ per-thread estimate only (`estimatedUsageUsdMicros`) |

---

## What was measured on this Mac

### Claude Code: `~/.claude/projects/**/*.jsonl`
- Every `assistant` line carries `message.usage`: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens` (100 % of lines), plus `cache_creation`, `service_tier`, `speed`, `server_tool_use`. The model is in `message.model`.
- **Deduplication is essential:** 15,052 duplicate lines against 11,195 unique responses in the last 9 days (resumed sessions copy history). Dedupe on `(message.id, requestId)`, as `DEVELOPMENT.md` §2.5 already says.
- Last 24 h: **518 M tokens**, of which **cache reads are 511.3 M (98.7 %)**, cache writes 5.7 M, output 1.02 M, and uncached input 3.4 k.
- By model (7 d): claude-opus-5-5 391 M, claude-opus-5 84 M, claude-sonnet-5 44 M.
- `~/.claude/stats-cache.json` has daily per-model tokens, but it's **stale** (last computed 2026-02-09). Don't use it.
- The status line JSON (official docs) has token fields, but only for the **current session's context window** (`context_window.total_input_tokens` etc.) and a per-session `cost.total_cost_usd`. There are no account totals, so it's not useful for usage history.
- Official alternative: **OpenTelemetry metrics** (`CLAUDE_CODE_ENABLE_TELEMETRY=1`, metric `claude_code.token.usage` with `type` = input/output/cacheRead/cacheCreation and `model`, exported every 60 s). It works, but it means changing the user's Claude Code env config and running an OTLP receiver. The logs already have the same data, so **it's not recommended**.

### Codex: `~/.codex/sessions/**/rollout-*.jsonl` + `codex app-server`
- Each `token_count` event has `info.total_token_usage` / `info.last_token_usage` with `input_tokens`, `cached_input_tokens`, `cache_write_input_tokens`, `output_tokens`, `reasoning_output_tokens`, `total_tokens`. `total_tokens = input_tokens + output_tokens`: input includes cached, and output includes reasoning. The model comes from the preceding `turn_context` line.
- 7 d on this Mac: **~1.08 B tokens**, **96 % cached input**. By model: gpt-6-astra 1.11 B, gpt-5.6-luna 279 M, and others.
- `account/usage/read` (app-server) returns **account-wide** `dailyUsageBuckets` (262 days) and a `summary` (`lifetimeTokens`, `peakDailyTokens`, `currentStreakDays`, `longestStreakDays`, `longestRunningTurnSec`). The buckets are **UTC days**, and **today's bucket is not present yet**.
- Cross-check, local session files vs account-wide, Sep 16–22 (UTC days):

  | Day | Account | This Mac | Share |
  |---|---|---|---|
  | 09-16 | 187.0 M | 176.2 M | 94 % |
  | 09-17 | 171.2 M | 161.6 M | 94 % |
  | 09-18 | 84.4 M | 81.6 M | 97 % |
  | 09-19 | 46.8 M | 46.3 M | 99 % |
  | 09-20 | 151.3 M | 127.2 M | 84 % |
  | 09-21 | 330.3 M | 308.2 M | 93 % |
  | 09-22 | 195.1 M | 176.9 M | 91 % |
  | **7 d** | **1.166 B** | **1.078 B** | **92 %** |

  The gap is usage that didn't happen in this Mac's Codex CLI or app (other devices, cloud tasks).
- Rollouts also showed a second limit bucket, `codex_bengalfox`, with its own 5 h + weekly windows. `account/rateLimits/read` didn't list it today. Expect **more than one limit id**, and don't assume `"codex"` is the only one.

---

## ⚠ Bug in the current spec (`DEVELOPMENT.md` §2.2)

§2.2 computes Codex hourly tokens as `total_tokens − previous_total_for_this_file` and treats the first line as the full total. **Forked or resumed sessions start a new file whose first `total_token_usage` already includes the whole parent conversation**, so that history gets counted again. On this Mac that inflated two days to **152 %** and **258 %** of the account totals.

**Fix (verified; it produced the table above):** per file,
1. First `token_count` with non-null `info` → add `last_token_usage.total_tokens` only.
2. `total > prev` → add `total − prev`.
3. `total < prev` (counter reset) → add `last_token_usage.total_tokens`.
4. `total == prev` (repeated event) → add 0.

Apply the same per-type deltas for the breakdown (input / cached / output / reasoning).

---

## Pitfall: "total tokens" is misleading

Cache reads are 96–99 % of all tokens. On the APIs they're billed at a small fraction of the fresh-input price, and they probably count for less against plan limits, though neither provider publishes how limits weight token types. Headline totals of hundreds of millions per day are accurate but mean little next to a "62 % of session" bar, and any tokens-per-% ratio will swing with the cache hit rate.

**Recommendation:** lead with **output tokens** and **fresh input** (uncached input + cache writes), and show cache reads as a secondary number or a "92 % cached" badge.

---

## Recommended way to include it

1. **Card line (Overview, popover):** under each bar, add a quiet secondary row, for example `4.8M tokens this window · 97% cached`. It's computed from local data since `resets_at − duration`, and labeled *on this Mac*.
2. **Detail tab:** the hourly and daily charts in tokens (already decision D1/D2 in the design prompt), plus a stacked breakdown toggle (Output / Fresh input / Cache) and a small per-model list.
3. **Codex account-wide:** use `account/usage/read` for completed days in the daily chart, labeled "All Codex usage", and local data for today.
4. **Don't show:** tokens remaining, "X of Y tokens", or tokens-per-% conversions (no cap is exposed, and the ratio is unstable).
5. **Cost:** leave it out of v1. An estimate would need a model price table we'd have to maintain, and it doesn't reflect subscription billing.

### Spec changes this would need (not yet applied)
- `TokenEvent` / `Bucket` get a per-type breakdown (`input`, `cacheWrite`, `cacheRead`, `output`) and `model`, instead of a single `tokens`.
- A new `windowTokens` per `LimitWindow`, summed from history since window start (with a scope label).
- The §2.2 counting fix above, plus a regression fixture: a forked rollout whose first line carries history.
- Design: a secondary token row on cards, and a breakdown toggle on the detail charts.

## Open questions for the owner
1. Is "this Mac only" acceptable for Claude token numbers? Nothing account-wide is available for Claude.
2. Which headline number do you want: output tokens, fresh input + output, or total including cache?
3. Should the Codex daily chart prefer account-wide totals (complete, but lagging a day) or local data (live, but ~92 %)?
