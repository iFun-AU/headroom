#!/usr/bin/env node
/**
 * @file soak-generator.mjs
 * @description Deterministic 10 Hz provider fixtures for the release soak.
 */

import { appendFileSync, renameSync, writeFileSync } from "node:fs";

const [rolloutFile, claudeFile, bridgeFile, rewriteLog, timestamp, startRaw, resetRaw] = process.argv.slice(2);
if ([rolloutFile, claudeFile, bridgeFile, rewriteLog, timestamp, startRaw, resetRaw].some((value) => value === undefined)) {
  throw new Error("usage: soak-generator <rollout> <claude> <bridge> <rewrite-log> <timestamp> <start> <reset>");
}

const startEpoch = Number.parseInt(startRaw, 10);
const resetEpoch = Number.parseInt(resetRaw, 10);
if (!Number.isSafeInteger(startEpoch) || !Number.isSafeInteger(resetEpoch)) {
  throw new Error("soak generator epochs must be safe integers");
}

let sequence = 1;

function appendJsonLine(path, value) {
  appendFileSync(path, `${JSON.stringify(value)}\n`);
}

function tick() {
  const used = 10 + sequence % 70;
  appendJsonLine(rolloutFile, {
    timestamp,
    type: "event_msg",
    payload: {
      type: "token_count",
      info: {
        total_token_usage: { total_tokens: sequence * 10 },
        last_token_usage: { total_tokens: 10 },
      },
      rate_limits: {
        limit_id: "codex",
        primary: { used_percent: used, window_minutes: 10_080, resets_at: resetEpoch },
        plan_type: "pro",
      },
    },
  });
  appendJsonLine(claudeFile, {
    type: "assistant",
    timestamp,
    requestId: `soak-req-${sequence}`,
    message: {
      id: `soak-msg-${sequence}`,
      usage: { input_tokens: 7, output_tokens: 5 },
    },
  });
  if (sequence % 5 === 0) {
    const next = `${bridgeFile}.next`;
    const bridge = {
      schema: 1,
      writtenAt: startEpoch + Math.floor(sequence / 10),
      sessionId: "soak",
      rateLimits: {
        five_hour: { used_percentage: used, resets_at: resetEpoch },
        seven_day: { used_percentage: Math.floor(used / 2), resets_at: resetEpoch },
      },
    };
    writeFileSync(next, `${JSON.stringify(bridge)}\n`);
    renameSync(next, bridgeFile);
    appendFileSync(rewriteLog, `${sequence}\n`);
  }
  sequence += 1;
}

const timer = setInterval(tick, 100);
tick();

function stop() {
  clearInterval(timer);
  process.exit(0);
}

process.on("SIGINT", stop);
process.on("SIGTERM", stop);
