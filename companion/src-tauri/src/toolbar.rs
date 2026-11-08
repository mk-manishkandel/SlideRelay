//! The floating toolbar shown while broadcasting, the full-screen QR code
//! for the projector, and the keyboard shortcuts.
//!
//! Neither window has a title bar or close button and neither takes keyboard
//! focus: PowerPoint keeps the keyboard (so arrow keys still change slides),
//! and there is nothing for "quit when the window closes" tools to act on.

use crate::config::ToolbarPosition;
use crate::session::BroadcastState;
use crate::state::{AppState, Command};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Monitor, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

pub const TOOLBAR: &str = "toolbar";
pub const QR: &str = "qr";

/// Toolbar size in screen points.
const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 52.0;
/// Gap between the toolbar and the screen edge, in points.
const MARGIN: f64 = 16.0;

// ---------- Placement (pure, unit-tested) ----------

/// A rectangle in physical pixels: x, y, width, height.
pub type Rect = (f64, f64, f64, f64);

/// Top-left corner (physical pixels) for the toolbar inside a screen's
/// usable area (which already excludes the menu bar, Dock and taskbar).
pub fn toolbar_origin(area: Rect, scale: f64, position: &ToolbarPosition) -> (f64, f64) {
    let (ax, ay, aw, ah) = area;
    let (w, h, m) = (WIDTH * scale, HEIGHT * scale, MARGIN * scale);
    let x = match position {
        ToolbarPosition::TopCenter | ToolbarPosition::BottomCenter => ax + (aw - w) / 2.0,
        ToolbarPosition::TopRight | ToolbarPosition::BottomRight => ax + aw - w - m,
    };
    let y = match position {
        ToolbarPosition::TopCenter | ToolbarPosition::TopRight => ay + m,
        ToolbarPosition::BottomCenter | ToolbarPosition::BottomRight => ay + ah - h - m,
    };
    (x.max(ax), y.max(ay))
}

/// Which screen (index into `screens`, as physical rects with their scale)
/// shows the slideshow, given its window bounds. macOS reports window
/// bounds in points, Windows in pixels.
pub fn slideshow_screen(screens: &[(Rect, f64)], bounds: Option<Rect>) -> Option<usize> {
    let (x, y, w, h) = bounds?;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    screens.iter().position(|&((sx, sy, sw, sh), scale)| {
        let s = if cfg!(target_os = "macos") {
            scale
        } else {
            1.0
        };
        let (sx, sy, sw, sh) = (sx / s, sy / s, sw / s, sh / s);
        cx >= sx && cx < sx + sw && cy >= sy && cy < sy + sh
    })
}

/// The teacher's own screen: with two or more screens, one that is NOT
/// showing the slideshow (that one is the projector); otherwise the main one.
pub fn teacher_screen(screen_count: usize, slideshow: Option<usize>, primary: usize) -> usize {
    match slideshow {
        Some(projector) if screen_count > 1 => {
            if primary != projector {
                primary
            } else {
                (0..screen_count)
                    .find(|&i| i != projector)
                    .unwrap_or(primary)
            }
        }
        _ => primary,
    }
}

fn rect(m: &Monitor) -> Rect {
    (
        m.position().x as f64,
        m.position().y as f64,
        m.size().width as f64,
        m.size().height as f64,
    )
}

fn work_rect(m: &Monitor) -> Rect {
    let a = m.work_area();
    (
        a.position.x as f64,
        a.position.y as f64,
        a.size.width as f64,
        a.size.height as f64,
    )
}

struct Screens {
    monitors: Vec<Monitor>,
    slideshow: Option<usize>,
    primary: usize,
}

fn screens(app: &AppHandle) -> Option<Screens> {
    let monitors = app.available_monitors().ok()?;
    if monitors.is_empty() {
        return None;
    }
    let primary_pos = app.primary_monitor().ok().flatten().map(|m| *m.position());
    let primary = monitors
        .iter()
        .position(|m| Some(*m.position()) == primary_pos)
        .unwrap_or(0);
    let bounds = *app
        .state::<Arc<AppState>>()
        .slideshow_bounds
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let list: Vec<(Rect, f64)> = monitors
        .iter()
        .map(|m| (rect(m), m.scale_factor()))
        .collect();
    let slideshow = slideshow_screen(&list, bounds);
    Some(Screens {
        monitors,
        slideshow,
        primary,
    })
}

// ---------- macOS: float above full-screen slideshows ----------

/// Keeps a window visible on every Space, including above PowerPoint's
/// full-screen slideshow, without appearing in Cmd-Tab.
#[cfg(target_os = "macos")]
fn float_above_slideshow(window: &WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    let w = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Ok(ptr) = w.ns_window() else { return };
        // SAFETY: Tauri returns this window's live NSWindow, and we are on
        // the main thread as AppKit requires.
        let ns: &NSWindow = unsafe { &*ptr.cast::<NSWindow>() };
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        // NSStatusWindowLevel: above normal and full-screen app windows.
        ns.setLevel(25);
    });
}

#[cfg(not(target_os = "macos"))]
fn float_above_slideshow(_window: &WebviewWindow) {}

/// Gives the keyboard back to PowerPoint after a toolbar click, so the
/// teacher's next arrow key still changes the slide.
pub fn refocus_powerpoint(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(|| {
            use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
            use objc2_foundation::NSString;
            let id = NSString::from_str("com.microsoft.Powerpoint");
            let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&id);
            if let Some(powerpoint) = apps.firstObject() {
                powerpoint.activateWithOptions(NSApplicationActivationOptions::empty());
            }
        });
    }
    // Windows: the toolbar never takes focus (WS_EX_NOACTIVATE), so
    // PowerPoint keeps it.
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

// ---------- The toolbar ----------

fn toolbar_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window(TOOLBAR) {
        return Some(w);
    }
    let built = WebviewWindowBuilder::new(app, TOOLBAR, WebviewUrl::App("toolbar.html".into()))
        .title("SlideRelay")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .accept_first_mouse(true)
        .shadow(true)
        .visible(false)
        .build();
    match built {
        Ok(w) => {
            float_above_slideshow(&w);
            Some(w)
        }
        Err(e) => {
            tracing::error!("could not open the toolbar: {e}");
            None
        }
    }
}

/// Shows the toolbar on the teacher's screen, at their chosen position.
pub fn show_toolbar(app: &AppHandle) {
    let Some(window) = toolbar_window(app) else {
        return;
    };
    place_toolbar(app, &window);
    let _ = window.show();
}

/// Moves the toolbar if the slideshow moved to another screen or the
/// teacher changed the position setting.
pub fn reposition_toolbar(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TOOLBAR) {
        if window.is_visible().unwrap_or(false) {
            place_toolbar(app, &window);
        }
    }
}

fn place_toolbar(app: &AppHandle, window: &WebviewWindow) {
    let Some(s) = screens(app) else { return };
    let i = teacher_screen(s.monitors.len(), s.slideshow, s.primary);
    let m = &s.monitors[i];
    let position = app.state::<Arc<AppState>>().settings().toolbar_position;
    let (x, y) = toolbar_origin(work_rect(m), m.scale_factor(), &position);
    let _ = window.set_size(tauri::LogicalSize::new(WIDTH, HEIGHT));
    let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
}

pub fn hide_toolbar(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(TOOLBAR) {
        let _ = window.hide();
    }
}

// ---------- The QR code on the projector ----------

fn qr_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window(QR) {
        return Some(w);
    }
    let built = WebviewWindowBuilder::new(app, QR, WebviewUrl::App("qr.html".into()))
        .title("Scan to watch")
        .decorations(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(false)
        .build();
    match built {
        Ok(w) => {
            float_above_slideshow(&w);
            Some(w)
        }
        Err(e) => {
            tracing::error!("could not open the QR code: {e}");
            None
        }
    }
}

/// Covers the slideshow's screen (the projector) with the QR code. It's
/// hidden, never closed, by a click on it, the toolbar or the shortcut.
pub fn show_qr(app: &AppHandle) {
    let Some(window) = qr_window(app) else { return };
    if let Some(s) = screens(app) {
        let m = &s.monitors[s.slideshow.unwrap_or(s.primary)];
        let _ = window.set_position(*m.position());
        let _ = window.set_size(*m.size());
    }
    let _ = window.show();
}

pub fn hide_qr(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QR) {
        let _ = window.hide();
    }
}

pub fn qr_visible(app: &AppHandle) -> bool {
    app.get_webview_window(QR)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

pub fn toggle_qr(app: &AppHandle) {
    if qr_visible(app) {
        hide_qr(app);
    } else {
        show_qr(app);
    }
}

// ---------- Keyboard shortcuts (only while broadcasting) ----------

/// Ctrl+Option (Mac) / Ctrl+Alt (Windows) + a letter: P pause, B blank,
/// Q QR code. PowerPoint doesn't use these, and they're only taken while
/// a broadcast is running.
fn shortcuts() -> [(Shortcut, Action); 3] {
    let mods = Some(Modifiers::CONTROL | Modifiers::ALT);
    [
        (Shortcut::new(mods, Code::KeyP), Action::Pause),
        (Shortcut::new(mods, Code::KeyB), Action::Blank),
        (Shortcut::new(mods, Code::KeyQ), Action::Qr),
    ]
}

#[derive(Clone, Copy)]
enum Action {
    Pause,
    Blank,
    Qr,
}

pub fn register_shortcuts(app: &AppHandle) {
    for (shortcut, _) in shortcuts() {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            // Another app may own it; the toolbar still works.
            tracing::warn!("keyboard shortcut {shortcut} unavailable: {e}");
        }
    }
}

pub fn unregister_shortcuts(app: &AppHandle) {
    for (shortcut, _) in shortcuts() {
        let _ = app.global_shortcut().unregister(shortcut);
    }
}

/// Called by the global-shortcut plugin when one of our keys is pressed.
pub fn on_shortcut(app: &AppHandle, pressed: &Shortcut) {
    let Some(action) = shortcuts()
        .into_iter()
        .find(|(s, _)| s == pressed)
        .map(|(_, a)| a)
    else {
        return;
    };
    let state = app.state::<Arc<AppState>>().inner().clone();
    let status = state.status();
    if status.state == BroadcastState::Idle {
        return;
    }
    let result = match action {
        Action::Pause => state.run(if status.paused {
            Command::Resume
        } else {
            Command::Pause
        }),
        Action::Blank => state.run(if status.blanked {
            Command::Unblank
        } else {
            Command::Blank
        }),
        Action::Qr => {
            toggle_qr(app);
            return;
        }
    };
    if let Err(e) = result {
        tracing::warn!("shortcut command failed: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_sits_inside_the_usable_area() {
        let area = (0.0, 50.0, 1920.0, 1000.0);
        let (x, y) = toolbar_origin(area, 1.0, &ToolbarPosition::TopCenter);
        assert_eq!((x, y), ((1920.0 - WIDTH) / 2.0, 50.0 + MARGIN));
        let (x, y) = toolbar_origin(area, 2.0, &ToolbarPosition::BottomRight);
        assert_eq!(x, 1920.0 - WIDTH * 2.0 - MARGIN * 2.0);
        assert_eq!(y, 50.0 + 1000.0 - HEIGHT * 2.0 - MARGIN * 2.0);
        // A tiny screen never pushes it off the top-left edge.
        let (x, y) = toolbar_origin(
            (100.0, 0.0, 200.0, 40.0),
            1.0,
            &ToolbarPosition::BottomRight,
        );
        assert_eq!((x, y), (100.0, 0.0));
    }

    #[test]
    fn toolbar_avoids_the_projector() {
        // Laptop is primary (0); slideshow on the projector (1).
        assert_eq!(teacher_screen(2, Some(1), 0), 0);
        // Slideshow on the primary screen: use the other one.
        assert_eq!(teacher_screen(2, Some(0), 0), 1);
        // One screen, or slideshow not found: the main screen.
        assert_eq!(teacher_screen(1, Some(0), 0), 0);
        assert_eq!(teacher_screen(3, None, 2), 2);
    }

    #[test]
    fn finds_the_slideshow_screen() {
        let screens = [
            ((0.0, 0.0, 1440.0, 900.0), 1.0),
            ((1440.0, 0.0, 1920.0, 1080.0), 1.0),
        ];
        assert_eq!(
            slideshow_screen(&screens, Some((1440.0, 0.0, 1920.0, 1080.0))),
            Some(1)
        );
        assert_eq!(
            slideshow_screen(&screens, Some((0.0, 0.0, 1440.0, 900.0))),
            Some(0)
        );
        assert_eq!(slideshow_screen(&screens, None), None);
    }
}
