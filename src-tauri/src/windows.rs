//! Native window lifecycle, widget sizing, position persistence, and edge snapping.

use std::{future::pending, pin::Pin, time::Duration};

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, WindowEvent,
};
use tokio::{sync::watch, time::Sleep};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::log_trunc;

use crate::{
    commands::Route,
    settings::{Settings, SettingsHandle, SettingsState, WidgetVariant},
};

const MOVE_DEBOUNCE: Duration = Duration::from_millis(500);
const SNAP_DISTANCE: f64 = 24.0;
const SNAP_INSET: f64 = 12.0;

/// Latest-only native window events consumed by the owned lifecycle task.
pub struct WindowEvents {
    widget_moved: watch::Receiver<Option<PhysicalPosition<i32>>>,
}

/// Registers synchronous native event handlers for all three configured windows.
///
/// # Errors
///
/// Returns an error when any documented window is missing from the Tauri configuration.
pub fn install(app: &AppHandle) -> Result<WindowEvents, String> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is missing".to_owned())?;
    let main_for_close = main.clone();
    main.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Err(error) = main_for_close.hide() {
                warn!(error = %log_trunc(&error.to_string()), "could not hide main window on close");
            }
        }
    });

    let popover = app
        .get_webview_window("popover")
        .ok_or_else(|| "popover window is missing".to_owned())?;
    let popover_for_focus = popover.clone();
    popover.on_window_event(move |event| {
        if matches!(event, WindowEvent::Focused(false))
            && let Err(error) = popover_for_focus.hide()
        {
            warn!(error = %log_trunc(&error.to_string()), "could not hide unfocused popover");
        }
    });

    let widget = app
        .get_webview_window("widget")
        .ok_or_else(|| "widget window is missing".to_owned())?;
    let (moved_tx, widget_moved) = watch::channel(None);
    widget.on_window_event(move |event| {
        if let WindowEvent::Moved(position) = event {
            moved_tx.send_replace(Some(*position));
        }
    });

    Ok(WindowEvents { widget_moved })
}

/// Applies settings changes and serializes debounced widget position saves.
pub async fn run(
    app: AppHandle,
    mut settings_updates: watch::Receiver<SettingsState>,
    settings: SettingsHandle,
    mut events: WindowEvents,
    cancel: CancellationToken,
) {
    let initial = settings_updates.borrow().settings.clone();
    apply_window_settings(&app, &initial, None);
    show_first_launch(&app, &initial);
    let mut applied = initial;
    let mut move_deadline: Option<Pin<Box<Sleep>>> = None;

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            changed = settings_updates.changed() => {
                if changed.is_err() {
                    return;
                }
                let next = settings_updates.borrow_and_update().settings.clone();
                apply_window_settings(&app, &next, Some(&applied));
                applied = next;
            }
            changed = events.widget_moved.changed() => {
                if changed.is_err() {
                    return;
                }
                events.widget_moved.borrow_and_update();
                move_deadline = Some(Box::pin(tokio::time::sleep(MOVE_DEBOUNCE)));
            }
            () = wait_for_deadline(&mut move_deadline) => {
                move_deadline = None;
                settle_widget_position(&app, &settings).await;
            }
        }
    }
}

async fn wait_for_deadline(deadline: &mut Option<Pin<Box<Sleep>>>) {
    match deadline {
        Some(deadline) => deadline.await,
        None => pending().await,
    }
}

fn apply_window_settings(app: &AppHandle, next: &Settings, previous: Option<&Settings>) {
    if previous.is_none_or(|previous| previous.show_dock_icon != next.show_dock_icon) {
        apply_activation_policy(app, next.show_dock_icon);
    }
    let Some(widget) = app.get_webview_window("widget") else {
        warn!("widget window is unavailable while applying settings");
        return;
    };
    if previous.is_none_or(|previous| previous.widget.variant != next.widget.variant) {
        let (width, height) = widget_size(next.widget.variant);
        if let Err(error) = widget.set_size(LogicalSize::new(width, height)) {
            warn!(error = %log_trunc(&error.to_string()), "could not resize widget");
        }
    }
    let position_changed = previous.is_none_or(|previous| {
        previous.widget.x != next.widget.x || previous.widget.y != next.widget.y
    });
    if position_changed
        && let (Some(x), Some(y)) = (next.widget.x, next.widget.y)
        && widget.outer_position().ok() != Some(PhysicalPosition::new(x, y))
        && let Err(error) = widget.set_position(PhysicalPosition::new(x, y))
    {
        warn!(error = %log_trunc(&error.to_string()), "could not restore widget position");
    }
    if previous.is_none_or(|previous| previous.widget.visible != next.widget.visible) {
        let result = if next.widget.visible {
            widget.show()
        } else {
            widget.hide()
        };
        if let Err(error) = result {
            warn!(error = %log_trunc(&error.to_string()), "could not apply widget visibility");
        }
    }
}

#[cfg(target_os = "macos")]
fn apply_activation_policy(app: &AppHandle, show_dock_icon: bool) {
    let policy = if show_dock_icon {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    if let Err(error) = app.set_activation_policy(policy) {
        warn!(error = %log_trunc(&error.to_string()), "could not apply activation policy");
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_activation_policy(_app: &AppHandle, _show_dock_icon: bool) {}

fn show_first_launch(app: &AppHandle, settings: &Settings) {
    if settings.onboarding_completed {
        return;
    }
    let Some(main) = app.get_webview_window("main") else {
        warn!("main window is unavailable for onboarding");
        return;
    };
    if let Err(error) = main
        .emit("navigate", Route::Onboarding)
        .and_then(|()| main.show())
        .and_then(|()| main.set_focus())
    {
        warn!(error = %log_trunc(&error.to_string()), "could not show first-launch onboarding");
    }
}

async fn settle_widget_position(app: &AppHandle, settings: &SettingsHandle) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let position = match widget.outer_position() {
        Ok(position) => position,
        Err(error) => {
            warn!(error = %log_trunc(&error.to_string()), "could not read widget position");
            return;
        }
    };
    let final_position = match widget.current_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let logical_position = position.to_logical::<f64>(scale);
            let logical_size = match widget.outer_size() {
                Ok(size) => size.to_logical::<f64>(scale),
                Err(error) => {
                    warn!(error = %log_trunc(&error.to_string()), "could not read widget size");
                    return;
                }
            };
            let work_area = monitor.work_area();
            let work_position = work_area.position.to_logical::<f64>(scale);
            let work_size = work_area.size.to_logical::<f64>(scale);
            let snapped = snap_position(
                logical_position,
                logical_size,
                LogicalFrame {
                    x: work_position.x,
                    y: work_position.y,
                    width: work_size.width,
                    height: work_size.height,
                },
            );
            let physical = snapped.to_physical::<i32>(scale);
            if physical != position
                && let Err(error) = widget.set_position(snapped)
            {
                warn!(error = %log_trunc(&error.to_string()), "could not snap widget to screen edge");
                return;
            }
            physical
        }
        Ok(None) => position,
        Err(error) => {
            warn!(error = %log_trunc(&error.to_string()), "could not find widget monitor");
            position
        }
    };
    if let Err(error) = settings
        .update_widget_position(final_position.x, final_position.y)
        .await
    {
        warn!(error = %log_trunc(&error.to_string()), "could not persist widget position");
    }
}

fn widget_size(variant: WidgetVariant) -> (f64, f64) {
    match variant {
        WidgetVariant::Pill => (280.0, 72.0),
        WidgetVariant::Stack => (160.0, 180.0),
        WidgetVariant::Mini => (200.0, 24.0),
    }
}

#[derive(Debug, Clone, Copy)]
struct LogicalFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn snap_position(
    position: LogicalPosition<f64>,
    size: LogicalSize<f64>,
    frame: LogicalFrame,
) -> LogicalPosition<f64> {
    LogicalPosition::new(
        snap_axis(position.x, size.width, frame.x, frame.width),
        snap_axis(position.y, size.height, frame.y, frame.height),
    )
}

fn snap_axis(position: f64, size: f64, frame_start: f64, frame_size: f64) -> f64 {
    let frame_end = frame_start + frame_size;
    if (position - frame_start).abs() <= SNAP_DISTANCE {
        frame_start + SNAP_INSET
    } else if (frame_end - (position + size)).abs() <= SNAP_DISTANCE {
        frame_end - size - SNAP_INSET
    } else {
        position
    }
}

#[cfg(test)]
mod tests {
    use tauri::{LogicalPosition, LogicalSize};

    use super::{LogicalFrame, snap_position, widget_size};
    use crate::settings::WidgetVariant;

    #[test]
    fn widget_variants_have_exact_contract_sizes() {
        assert_eq!(widget_size(WidgetVariant::Pill), (280.0, 72.0));
        assert_eq!(widget_size(WidgetVariant::Stack), (160.0, 180.0));
        assert_eq!(widget_size(WidgetVariant::Mini), (200.0, 24.0));
    }

    #[test]
    fn edge_snap_uses_twenty_four_point_detection_and_twelve_point_inset() {
        let frame = LogicalFrame {
            x: -100.0,
            y: 20.0,
            width: 1_000.0,
            height: 700.0,
        };
        let size = LogicalSize::new(200.0, 72.0);
        assert_eq!(
            snap_position(LogicalPosition::new(-78.0, 25.0), size, frame),
            LogicalPosition::new(-88.0, 32.0)
        );
        assert_eq!(
            snap_position(LogicalPosition::new(720.0, 630.0), size, frame),
            LogicalPosition::new(688.0, 636.0)
        );
    }

    #[test]
    fn position_outside_detection_distance_is_unchanged() {
        let frame = LogicalFrame {
            x: 0.0,
            y: 0.0,
            width: 1_000.0,
            height: 800.0,
        };
        let position = LogicalPosition::new(100.0, 100.0);
        assert_eq!(
            snap_position(position, LogicalSize::new(200.0, 72.0), frame),
            position
        );
    }
}
