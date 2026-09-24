/**
 * @file credits.ts
 * @description Formats Claude extra usage and Codex credits with the same rules as the menu bar (D-030).
 */
import type { CreditAmount } from "./bindings/CreditAmount";
import type { Credits } from "./bindings/Credits";

const CURRENCY_SYMBOLS: Readonly<Record<string, string>> = { USD: "$", EUR: "€", GBP: "£" };

/** What a surface shows for one provider's paid usage. */
export interface CreditSummary {
  /** `ex` for Claude extra usage (money spent), `cr` for Codex credits. */
  readonly tag: "ex" | "cr";
  /** The amount alone, e.g. `$12.40`, `120` or `∞`. */
  readonly amount: string;
  /** The amount with its unit where needed, e.g. `$12.40` or `120 cr`. */
  readonly short: string;
  /** Full description for labels and tooltips. */
  readonly long: string;
  /** Spent share of the spending limit, when both are known in one currency. */
  readonly percent: number | null;
}

/** Formats exact minor units without floating-point rounding, e.g. `$12.40` or `12.40 AUD`. */
export function formatAmount(amount: CreditAmount): string {
  const sign = amount.minor < 0 ? "-" : "";
  const digits = String(Math.abs(amount.minor)).padStart(amount.exponent + 1, "0");
  const number = amount.exponent === 0 ? digits : `${digits.slice(0, -amount.exponent)}.${digits.slice(-amount.exponent)}`;
  if (amount.currency === null) return `${sign}${number}`;
  const symbol = CURRENCY_SYMBOLS[amount.currency];
  return symbol === undefined ? `${sign}${number} ${amount.currency}` : `${sign}${symbol}${number}`;
}

function spentPercent(used: CreditAmount, limit: CreditAmount | null): number | null {
  if (limit === null || limit.minor <= 0 || limit.currency !== used.currency || limit.exponent !== used.exponent) return null;
  return Math.min(100, Math.max(0, (used.minor / limit.minor) * 100));
}

/**
 * Summarizes a provider's paid usage, or `null` when there is nothing to show:
 * Claude extra usage that is off, or a Codex account without credits.
 */
export function creditSummary(name: string, credits: Credits | null): CreditSummary | null {
  if (credits === null) return null;
  if (credits.enabled && credits.used !== null) {
    const amount = formatAmount(credits.used);
    const long = credits.limit === null ? `${name} extra usage ${amount}` : `${name} extra usage ${amount} of ${formatAmount(credits.limit)}`;
    return { tag: "ex", amount, short: amount, long, percent: spentPercent(credits.used, credits.limit) };
  }
  if (credits.unlimited) return { tag: "cr", amount: "∞", short: "∞ cr", long: `${name} credits unlimited`, percent: null };
  if (!credits.enabled || credits.balance === null) return null;
  const amount = formatAmount(credits.balance);
  return { tag: "cr", amount, short: `${amount} cr`, long: `${name} credits ${amount}`, percent: null };
}
