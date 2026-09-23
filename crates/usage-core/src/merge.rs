use std::{
    cmp::Reverse,
    collections::{BTreeMap, btree_map::Entry},
};

use crate::{
    ConnectionStatus, LimitWindow, Percent, Provider, ProviderUsage, SourceHealth, SourceKind,
    UnixSeconds, UsageSnapshot, WindowKind,
};

const FRESH_SECONDS: i64 = 15 * 60;
const RESET_RETENTION_SECONDS: i64 = 24 * 60 * 60;

/// One normalized source-to-store usage message.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// The provider represented by this reading.
    pub provider: Provider,
    /// The source that emitted the reading.
    pub source: SourceKind,
    /// When the source observed the data.
    pub observed_at: UnixSeconds,
    /// The provider plan, when reported.
    pub plan: Option<String>,
    /// Only the windows present in this source message.
    pub windows: Vec<LimitWindow>,
    /// Whether this is a sparse upsert rather than a complete source snapshot.
    pub partial: bool,
}

/// Last-known state for one provider source.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceState {
    /// This source's own connection status.
    pub status: ConnectionStatus,
    /// Time of the source's latest successful reading.
    pub observed_at: Option<UnixSeconds>,
    /// Last plan value reported by this source.
    pub plan: Option<String>,
    /// Last-known windows keyed by their classified kind.
    pub windows: BTreeMap<WindowKind, LimitWindow>,
}

impl SourceState {
    /// Constructs an empty source state with an explicit initial status.
    #[must_use]
    pub const fn new(status: ConnectionStatus) -> Self {
        Self {
            status,
            observed_at: None,
            plan: None,
            windows: BTreeMap::new(),
        }
    }
}

/// Store state keyed independently by provider and source.
pub type State = BTreeMap<(Provider, SourceKind), SourceState>;

/// Ingests a full or sparse source reading.
///
/// Returns `false` only when the full reading is rejected as out of order.
#[must_use]
pub fn ingest_reading(state: &mut State, reading: Reading) -> bool {
    let key = (reading.provider, reading.source);

    if !reading.partial
        && state
            .get(&key)
            .and_then(|source| source.observed_at)
            .is_some_and(|stored_at| reading.observed_at < stored_at)
    {
        return false;
    }

    let source_state = state.entry(key).or_insert_with(|| {
        SourceState::new(ConnectionStatus::NotConfigured {
            hint: "Waiting for the first reading".to_owned(),
        })
    });

    if reading.partial {
        ingest_partial(source_state, &reading);
    } else {
        source_state.windows = reading
            .windows
            .into_iter()
            .map(|window| {
                let normalized = normalize_window(window, reading.source, reading.observed_at);
                (normalized.kind, normalized)
            })
            .collect();
    }

    if let Some(plan) = reading.plan {
        source_state.plan = Some(plan);
    }
    source_state.observed_at = Some(
        source_state
            .observed_at
            .map_or(reading.observed_at, |stored| {
                stored.max(reading.observed_at)
            }),
    );
    source_state.status = ConnectionStatus::Connected;
    true
}

/// Updates one source's status without erasing its last successful data.
pub fn ingest_status(
    state: &mut State,
    provider: Provider,
    source: SourceKind,
    status: ConnectionStatus,
) {
    match state.entry((provider, source)) {
        Entry::Vacant(entry) => {
            entry.insert(SourceState::new(status));
        }
        Entry::Occupied(mut entry) => {
            entry.get_mut().status = status;
        }
    }
}

/// Derives the stable provider view from independent source states.
#[must_use]
pub fn derive_provider_usage(state: &State, provider: Provider, now: UnixSeconds) -> ProviderUsage {
    let mut configured = provider_sources(state, provider);
    configured.sort_by_key(|(source, _)| (source.limit_priority(), **source));

    let authoritative = select_authoritative(&configured, now);
    let highest_priority = configured
        .first()
        .map(|(source, source_state)| (**source, *source_state));

    let windows = authoritative.map_or_else(Vec::new, |(source, source_state)| {
        derive_windows(&configured, *source, source_state, now)
    });
    let plan = authoritative
        .and_then(|(_, source_state)| source_state.plan.clone())
        .or_else(|| {
            configured
                .iter()
                .find_map(|(_, source_state)| source_state.plan.clone())
        });
    let status = derive_status(authoritative, highest_priority, now);
    let sources = configured
        .iter()
        .map(|(source, source_state)| SourceHealth {
            source: **source,
            status: source_state.status.clone(),
            last_success: source_state.observed_at,
        })
        .collect();

    ProviderUsage {
        provider,
        plan,
        windows,
        status,
        authoritative_source: authoritative.map(|(source, _)| *source),
        last_updated: authoritative.and_then(|(_, source_state)| source_state.observed_at),
        sources,
    }
}

/// Derives a complete two-provider snapshot.
#[must_use]
pub fn derive_snapshot(state: &State, now: UnixSeconds) -> UsageSnapshot {
    UsageSnapshot {
        claude: derive_provider_usage(state, Provider::Claude, now),
        codex: derive_provider_usage(state, Provider::Codex, now),
        generated_at: now,
    }
}

fn ingest_partial(source_state: &mut SourceState, reading: &Reading) {
    for window in &reading.windows {
        let mut incoming = normalize_window(window.clone(), reading.source, reading.observed_at);
        if let Some(stored) = source_state.windows.get(&incoming.kind) {
            if reset_is_earlier(incoming.resets_at, stored.resets_at) {
                continue;
            }
            if incoming.resets_at.is_none() {
                incoming.resets_at = stored.resets_at;
            }
        }
        source_state.windows.insert(incoming.kind, incoming);
    }
}

fn normalize_window(
    mut window: LimitWindow,
    source: SourceKind,
    observed_at: UnixSeconds,
) -> LimitWindow {
    window.source = source;
    window.observed_at = observed_at;
    window.reset_pending = false;
    window
}

fn reset_is_earlier(incoming: Option<UnixSeconds>, stored: Option<UnixSeconds>) -> bool {
    matches!((incoming, stored), (Some(incoming), Some(stored)) if incoming < stored)
}

fn provider_sources(state: &State, provider: Provider) -> Vec<(&SourceKind, &SourceState)> {
    state
        .iter()
        .filter_map(|((candidate, source), source_state)| {
            (*candidate == provider).then_some((source, source_state))
        })
        .collect()
}

fn select_authoritative<'a>(
    configured: &[(&'a SourceKind, &'a SourceState)],
    now: UnixSeconds,
) -> Option<(&'a SourceKind, &'a SourceState)> {
    if let Some(fresh) = configured
        .iter()
        .copied()
        .find(|(_, source_state)| source_state.observed_at.is_some_and(|at| is_fresh(at, now)))
    {
        return Some(fresh);
    }

    configured
        .iter()
        .copied()
        .filter(|(_, source_state)| source_state.observed_at.is_some())
        .min_by_key(|(source, source_state)| {
            (
                Reverse(source_state.observed_at),
                source.limit_priority(),
                **source,
            )
        })
}

fn derive_windows(
    configured: &[(&SourceKind, &SourceState)],
    authoritative_source: SourceKind,
    authoritative_state: &SourceState,
    now: UnixSeconds,
) -> Vec<LimitWindow> {
    let authoritative_at = authoritative_state.observed_at;
    let mut windows = authoritative_state
        .windows
        .values()
        .filter_map(|window| {
            let refined =
                newest_refinement(configured, authoritative_source, authoritative_at, window)
                    .unwrap_or(window)
                    .clone();
            apply_reset(refined, now)
        })
        .collect::<Vec<_>>();
    windows.sort_by_key(|window| window.kind);
    windows
}

fn newest_refinement<'a>(
    configured: &[(&'a SourceKind, &'a SourceState)],
    authoritative_source: SourceKind,
    authoritative_at: Option<UnixSeconds>,
    authoritative_window: &LimitWindow,
) -> Option<&'a LimitWindow> {
    configured
        .iter()
        .copied()
        .filter(|(source, _)| **source != authoritative_source)
        .filter(|(_, state)| state.observed_at > authoritative_at)
        .filter_map(|(source, state)| {
            state
                .windows
                .get(&authoritative_window.kind)
                .filter(|candidate| {
                    reset_is_same_or_later(candidate.resets_at, authoritative_window.resets_at)
                })
                .map(|window| (source, state, window))
        })
        .min_by_key(|(source, state, _)| {
            (
                Reverse(state.observed_at),
                source.limit_priority(),
                **source,
            )
        })
        .map(|(_, _, window)| window)
}

fn reset_is_same_or_later(candidate: Option<UnixSeconds>, baseline: Option<UnixSeconds>) -> bool {
    match (candidate, baseline) {
        (Some(candidate), Some(baseline)) => candidate >= baseline,
        (Some(_) | None, None) => true,
        (None, Some(_)) => false,
    }
}

fn apply_reset(mut window: LimitWindow, now: UnixSeconds) -> Option<LimitWindow> {
    window.reset_pending = false;
    let Some(resets_at) = window.resets_at else {
        return Some(window);
    };
    if resets_at > now {
        return Some(window);
    }
    if now.0.saturating_sub(resets_at.0) > RESET_RETENTION_SECONDS {
        return None;
    }

    window.used = Percent::ZERO;
    window.reset_pending = true;
    Some(window)
}

fn derive_status(
    authoritative: Option<(&SourceKind, &SourceState)>,
    highest_priority: Option<(SourceKind, &SourceState)>,
    now: UnixSeconds,
) -> ConnectionStatus {
    let Some((authoritative_source, authoritative_state)) = authoritative else {
        return highest_priority.map_or_else(
            || ConnectionStatus::NotConfigured {
                hint: "Configure a usage source in Settings".to_owned(),
            },
            |(_, source_state)| source_state.status.clone(),
        );
    };
    let Some(observed_at) = authoritative_state.observed_at else {
        return ConnectionStatus::Stale;
    };
    if !is_fresh(observed_at, now) {
        return ConnectionStatus::Stale;
    }

    if highest_priority.is_some_and(|(source, _)| source == *authoritative_source) {
        return ConnectionStatus::Connected;
    }

    highest_priority.map_or(ConnectionStatus::Connected, |(_, source_state)| {
        ConnectionStatus::Degraded {
            reason: degradation_reason(&source_state.status),
        }
    })
}

fn degradation_reason(status: &ConnectionStatus) -> String {
    match status {
        ConnectionStatus::Degraded { reason } | ConnectionStatus::Unsupported { reason } => {
            reason.clone()
        }
        ConnectionStatus::NotConfigured { hint } | ConnectionStatus::AuthExpired { hint } => {
            hint.clone()
        }
        ConnectionStatus::Error { message } => message.clone(),
        ConnectionStatus::Stale | ConnectionStatus::Connected => {
            "Higher-priority source is stale".to_owned()
        }
    }
}

fn is_fresh(observed_at: UnixSeconds, now: UnixSeconds) -> bool {
    now.0.saturating_sub(observed_at.0) < FRESH_SECONDS
}
