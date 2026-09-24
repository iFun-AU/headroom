//! Native menu-bar tray construction, actions, and snapshot presentation.

use std::sync::Mutex;

use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_positioner::{Position, WindowExt};
use tracing::warn;
use usage_core::{ProviderUsage, UsageSnapshot, WindowKind, log_trunc};

use crate::{
    commands::{Route, WindowTarget, quit_app_impl, refresh_now_impl, show_window_impl},
    runtime::RuntimeState,
};

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open-dashboard";
const WIDGET_ID: &str = "show-widget";
const REFRESH_ID: &str = "refresh-now";
const SETTINGS_ID: &str = "open-settings";
const QUIT_ID: &str = "quit";

const NORMAL_ICON: &[u8] = include_bytes!("../icons/tray-normal.png");
const WARNING_ICON: &[u8] = include_bytes!("../icons/tray-warning.png");
const CRITICAL_ICON: &[u8] = include_bytes!("../icons/tray-critical.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayIconState {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayPresentation {
    title: String,
    tooltip: String,
    icon: TrayIconState,
}

struct TrayState {
    normal: Image<'static>,
    warning: Image<'static>,
    critical: Image<'static>,
    current: Mutex<TrayPresentation>,
}

impl TrayState {
    fn icon(&self, state: TrayIconState) -> Image<'static> {
        match state {
            TrayIconState::Normal => self.normal.clone(),
            TrayIconState::Warning => self.warning.clone(),
            TrayIconState::Critical => self.critical.clone(),
        }
    }
}

/// Builds the template tray icon and its exact native menu.
///
/// # Errors
///
/// Returns a Tauri error when an image, menu item, or tray resource cannot be created.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let normal = Image::from_bytes(NORMAL_ICON)?;
    let state = TrayState {
        normal: normal.clone(),
        warning: Image::from_bytes(WARNING_ICON)?,
        critical: Image::from_bytes(CRITICAL_ICON)?,
        current: Mutex::new(TrayPresentation {
            title: String::new(),
            tooltip: String::new(),
            icon: TrayIconState::Normal,
        }),
    };
    if !app.manage(state) {
        return Err(tauri::Error::AssetNotFound("tray state".into()));
    }

    let open = MenuItem::with_id(app, OPEN_ID, "Open Dashboard", true, None::<&str>)?;
    let widget = MenuItem::with_id(app, WIDGET_ID, "Show Floating Widget", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, REFRESH_ID, "Refresh Now", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, SETTINGS_ID, "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &widget,
            &separator_one,
            &refresh,
            &settings,
            &separator_two,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(normal)
        .icon_as_template(true)
        .show_menu_on_left_click(false)
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, &event))
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Updates tray text and icon only when their derived values change.
pub fn update(app: &AppHandle, snapshot: &UsageSnapshot) {
    let next = presentation(snapshot);
    let state = app.state::<TrayState>();
    let Ok(mut current) = state.current.lock() else {
        warn!("tray presentation lock was poisoned");
        return;
    };
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        warn!("tray icon is unavailable during snapshot update");
        return;
    };
    if current.title != next.title {
        if let Err(error) = tray.set_title(Some(&next.title)) {
            warn!(error = %log_trunc(&error.to_string()), "could not update tray title");
        } else {
            current.title.clone_from(&next.title);
        }
    }
    if current.tooltip != next.tooltip {
        if let Err(error) = tray.set_tooltip(Some(&next.tooltip)) {
            warn!(error = %log_trunc(&error.to_string()), "could not update tray tooltip");
        } else {
            current.tooltip.clone_from(&next.tooltip);
        }
    }
    if current.icon != next.icon {
        if let Err(error) = tray.set_icon(Some(state.icon(next.icon))) {
            warn!(error = %log_trunc(&error.to_string()), "could not update tray icon");
        } else {
            current.icon = next.icon;
        }
    }
}

/// Title shows each provider's weekly limit (decision D-025); the icon badge
/// still reflects the highest window of any kind so a session warning shows.
fn presentation(snapshot: &UsageSnapshot) -> TrayPresentation {
    let highest = snapshot
        .claude
        .windows
        .iter()
        .chain(snapshot.codex.windows.iter())
        .map(|window| window.used.get())
        .max_by(f64::total_cmp);
    let icon = match highest {
        Some(used) if used >= 90.0 => TrayIconState::Critical,
        Some(used) if used >= 75.0 => TrayIconState::Warning,
        _ => TrayIconState::Normal,
    };
    let weekly = [
        ("C", "Claude", weekly_used(&snapshot.claude)),
        ("X", "Codex", weekly_used(&snapshot.codex)),
    ]
    .into_iter()
    .filter_map(|(short, name, used)| used.map(|used| (short, name, used)))
    .collect::<Vec<_>>();
    if weekly.is_empty() {
        return TrayPresentation {
            title: String::new(),
            tooltip: "How Is It".to_owned(),
            icon,
        };
    }
    let title = weekly
        .iter()
        .map(|(short, _, used)| format!("{short} {used:.0}%"))
        .collect::<Vec<_>>()
        .join(" · ");
    let tooltip = weekly
        .iter()
        .map(|(_, name, used)| format!("{name} {used:.0}%"))
        .collect::<Vec<_>>()
        .join(", ");
    TrayPresentation {
        title: format!(" {title}"),
        tooltip: format!("Weekly limits: {tooltip}"),
        icon,
    }
}

fn weekly_used(usage: &ProviderUsage) -> Option<f64> {
    usage
        .windows
        .iter()
        .filter(|window| window.kind == WindowKind::Weekly)
        .map(|window| window.used.get())
        .max_by(f64::total_cmp)
}

fn toggle_popover(app: &AppHandle) {
    let Some(popover) = app.get_webview_window("popover") else {
        warn!("popover window is unavailable");
        return;
    };
    match popover.is_visible() {
        Ok(true) => {
            if let Err(error) = popover.hide() {
                warn!(error = %log_trunc(&error.to_string()), "could not hide popover");
            }
        }
        Ok(false) => {
            if let Err(error) = popover
                .move_window_constrained(Position::TrayBottomCenter)
                .and_then(|()| popover.show())
                .and_then(|()| popover.set_focus())
            {
                warn!(error = %log_trunc(&error.to_string()), "could not show popover");
            }
        }
        Err(error) => {
            warn!(error = %log_trunc(&error.to_string()), "could not inspect popover visibility");
        }
    }
}

fn handle_menu_event(app: &AppHandle, event: &tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        OPEN_ID => spawn_show(app, WindowTarget::Main, Some(Route::Overview)),
        WIDGET_ID => spawn_show(app, WindowTarget::Widget, None),
        SETTINGS_ID => spawn_show(app, WindowTarget::Main, Some(Route::Settings)),
        REFRESH_ID => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let Some(state) = app.try_state::<RuntimeState>() else {
                    warn!("runtime is unavailable for tray refresh");
                    return;
                };
                log_action(refresh_now_impl(&state).await);
            });
        }
        QUIT_ID => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let Some(state) = app.try_state::<RuntimeState>() else {
                    warn!("runtime is unavailable for tray quit");
                    return;
                };
                quit_app_impl(&app, &state).await;
            });
        }
        _ => {}
    }
}

fn spawn_show(app: &AppHandle, target: WindowTarget, route: Option<Route>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<RuntimeState>() else {
            warn!("runtime is unavailable for tray window action");
            return;
        };
        log_action(show_window_impl(&app, &state, target, route).await);
    });
}

fn log_action(result: Result<(), crate::commands::CommandError>) {
    if let Err(error) = result {
        warn!(message = %error.message, "tray action failed");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use usage_core::{
        ConnectionStatus, LimitWindow, Percent, Provider, ProviderUsage, SourceKind, UnixSeconds,
        UsageSnapshot, WindowKind,
    };

    use super::{TrayIconState, presentation};
    use WindowKind::{Session, Weekly};

    #[test]
    fn title_shows_each_providers_weekly_limit() {
        let result = presentation(&snapshot(
            &[(Session, 62.0), (Weekly, 41.4)],
            &[(Weekly, 78.0)],
        ));
        assert_eq!(result.title, " C 41% · X 78%");
        assert_eq!(result.tooltip, "Weekly limits: Claude 41%, Codex 78%");
        assert_eq!(result.icon, TrayIconState::Warning);
    }

    #[test]
    fn icon_still_reflects_the_highest_window_of_any_kind() {
        let result = presentation(&snapshot(&[(Session, 95.0), (Weekly, 20.0)], &[]));
        assert_eq!(result.title, " C 20%");
        assert_eq!(result.icon, TrayIconState::Critical);

        let normal = presentation(&snapshot(&[], &[(Weekly, 74.9)]));
        assert_eq!(normal.title, " X 75%");
        assert_eq!(normal.icon, TrayIconState::Normal);
    }

    #[test]
    fn no_weekly_window_leaves_title_empty() {
        let empty = presentation(&snapshot(&[], &[]));
        assert_eq!(empty.title, "");
        assert_eq!(empty.icon, TrayIconState::Normal);

        let session_only = presentation(&snapshot(&[(Session, 80.0)], &[]));
        assert_eq!(session_only.title, "");
        assert_eq!(session_only.icon, TrayIconState::Warning);
    }

    fn snapshot(claude: &[(WindowKind, f64)], codex: &[(WindowKind, f64)]) -> UsageSnapshot {
        UsageSnapshot {
            claude: provider(Provider::Claude, SourceKind::ClaudeStatusline, claude),
            codex: provider(Provider::Codex, SourceKind::CodexAppServer, codex),
            generated_at: UnixSeconds(1),
        }
    }

    fn provider(
        provider: Provider,
        source: SourceKind,
        values: &[(WindowKind, f64)],
    ) -> ProviderUsage {
        ProviderUsage {
            provider,
            plan: None,
            windows: values
                .iter()
                .map(|(kind, value)| LimitWindow {
                    kind: *kind,
                    used: Percent::new(*value).expect("finite fixture"),
                    resets_at: None,
                    reset_pending: false,
                    source,
                    observed_at: UnixSeconds(1),
                })
                .collect(),
            status: ConnectionStatus::Connected,
            authoritative_source: Some(source),
            last_updated: Some(UnixSeconds(1)),
            sources: Vec::new(),
        }
    }
}
