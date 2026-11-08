//! Finds PowerPoint's slideshow (audience) window and captures ONLY that
//! window. The Presenter View is never captured.
//!
//! Safety rule: if we can't be sure which window is the slideshow, we
//! capture nothing (viewers see "Waiting for the presenter…"). Showing
//! nothing is always better than showing the presenter's notes.
//!
//! Platform code (`macos.rs`, `windows.rs`) only lists PowerPoint's windows
//! and runs the OS capture API. The decision of which window to capture is
//! made here, in plain tested code.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use crate::stream::FrameSink;

/// What a PowerPoint window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// The audience slideshow — the only thing we may capture.
    Slideshow,
    /// Presenter View (notes, timer, next slide) — never captured.
    Presenter,
    /// Normal editing window, dialogs, etc.
    Other,
}

/// One PowerPoint window found on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// OS window id (CGWindowID on macOS, HWND on Windows).
    pub id: u64,
    pub kind: WindowKind,
    pub title: String,
    /// Window rectangle in screen points: x, y, width, height.
    pub bounds: (f64, f64, f64, f64),
}

/// The result of looking for the slideshow.
#[derive(Debug, Clone, PartialEq)]
pub enum Detection {
    Found(Candidate),
    /// No slideshow is running.
    NoSlideshow,
    /// Presenter View is open but no audience window exists (one screen
    /// with Presenter View on). We can't show viewers anything.
    PresenterOnly,
}

/// Picks the slideshow window. Never returns a Presenter window.
pub fn choose(candidates: &[Candidate]) -> Detection {
    let mut slideshows: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.kind == WindowKind::Slideshow)
        .collect();
    if slideshows.is_empty() {
        return if candidates.iter().any(|c| c.kind == WindowKind::Presenter) {
            Detection::PresenterOnly
        } else {
            Detection::NoSlideshow
        };
    }
    // Several slideshows (rare: two presentations running) → the largest.
    slideshows.sort_by(|a, b| {
        let area = |c: &Candidate| c.bounds.2 * c.bounds.3;
        area(b)
            .partial_cmp(&area(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Detection::Found(slideshows[0].clone())
}

/// Classifies a window by its title (used on macOS, where window class
/// names aren't available). Presenter wins over slideshow if both match,
/// so an unexpected title can only ever make us capture LESS.
pub fn classify_title(title: &str) -> WindowKind {
    let t = title.to_lowercase();
    const PRESENTER: &[&str] = &[
        "presenter view",    // English
        "referentenansicht", // German
        "mode présentateur", // French
        "vista moderador",   // Spanish
        "visualizzazione relatore",
        "modo de exibição do apresentador",
        "presentatorweergave",
        "発表者ツール",
        "演示者视图",
    ];
    const SLIDESHOW: &[&str] = &[
        "slide show",             // English: "PowerPoint Slide Show - …"
        "bildschirmpräsentation", // German
        "diaporama",              // French
        "presentación con diapositivas",
        "presentazione",
        "apresentação de slides",
        "diavoorstelling",
        "スライド ショー",
        "幻灯片放映",
    ];
    // PowerPoint's own part of the title comes before " - " (the file name
    // follows). Only that part may say "Slide Show", so a deck that's merely
    // NAMED "slide show.pptx" isn't mistaken for the slideshow.
    let prefix = match t.split_once(" - ") {
        Some((head, _)) => head,
        None => "",
    };
    if PRESENTER.iter().any(|p| t.contains(p)) {
        WindowKind::Presenter
    } else if SLIDESHOW.iter().any(|s| prefix.contains(s)) {
        WindowKind::Slideshow
    } else {
        WindowKind::Other
    }
}

/// Classifies a window by its Windows class name. PowerPoint uses these
/// names in every language.
pub fn classify_class(class: &str) -> WindowKind {
    match class {
        "screenClass" => WindowKind::Slideshow,
        "PodiumParent" => WindowKind::Presenter,
        _ => WindowKind::Other,
    }
}

/// A running capture of one window.
pub trait ActiveCapture: Send {
    /// The window id being captured.
    fn window_id(&self) -> u64;
    /// False once the window closed or the capture failed.
    fn is_alive(&self) -> bool;
    /// Stops capturing (also happens on drop).
    fn stop(self: Box<Self>);
}

/// Lists PowerPoint's windows (platform specific).
pub fn find_powerpoint_windows() -> Result<Vec<Candidate>, CaptureError> {
    #[cfg(target_os = "macos")]
    return macos::find_powerpoint_windows();
    #[cfg(windows)]
    return windows::find_powerpoint_windows();
    #[cfg(not(any(target_os = "macos", windows)))]
    Err(CaptureError::Unsupported)
}

/// Starts capturing `window`, sending frames to `sink`.
pub fn start_capture(
    window: &Candidate,
    sink: FrameSink,
    max_fps: u8,
    max_width: u32,
) -> Result<Box<dyn ActiveCapture>, CaptureError> {
    #[cfg(target_os = "macos")]
    return macos::start(window, sink, max_fps, max_width);
    #[cfg(windows)]
    return windows::start(window, sink, max_fps, max_width);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (window, sink, max_fps, max_width);
        Err(CaptureError::Unsupported)
    }
}

/// True if the OS allows us to capture windows (macOS Screen Recording).
pub fn has_permission() -> bool {
    #[cfg(target_os = "macos")]
    return macos::has_permission();
    #[cfg(not(target_os = "macos"))]
    true
}

/// Shows the OS permission prompt (macOS) — only the first time it's asked.
pub fn request_permission() {
    #[cfg(target_os = "macos")]
    macos::request_permission();
}

/// Asks for Screen Recording permission and opens the matching System
/// Settings page (macOS; nothing to do on Windows).
pub fn open_permission_settings() {
    request_permission();
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

#[derive(Debug)]
pub enum CaptureError {
    /// macOS Screen Recording permission is missing.
    NoPermission,
    Failed(String),
    #[allow(dead_code)]
    Unsupported,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPermission => write!(f, "screen recording permission missing"),
            Self::Failed(e) => write!(f, "{e}"),
            Self::Unsupported => write!(f, "capture is not supported on this system"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u64, kind: WindowKind, w: f64, h: f64) -> Candidate {
        Candidate {
            id,
            kind,
            title: String::new(),
            bounds: (0.0, 0.0, w, h),
        }
    }

    #[test]
    fn picks_slideshow_never_presenter() {
        let list = vec![
            c(1, WindowKind::Other, 1200.0, 800.0),
            c(2, WindowKind::Presenter, 1920.0, 1080.0),
            c(3, WindowKind::Slideshow, 1280.0, 720.0),
        ];
        assert_eq!(choose(&list), Detection::Found(list[2].clone()));
    }

    #[test]
    fn presenter_only_is_reported_not_captured() {
        let list = vec![
            c(1, WindowKind::Other, 1200.0, 800.0),
            c(2, WindowKind::Presenter, 1920.0, 1080.0),
        ];
        assert_eq!(choose(&list), Detection::PresenterOnly);
    }

    #[test]
    fn nothing_running() {
        assert_eq!(choose(&[]), Detection::NoSlideshow);
        assert_eq!(
            choose(&[c(1, WindowKind::Other, 100.0, 100.0)]),
            Detection::NoSlideshow
        );
    }

    #[test]
    fn largest_of_several_slideshows() {
        let list = vec![
            c(1, WindowKind::Slideshow, 800.0, 600.0),
            c(2, WindowKind::Slideshow, 1920.0, 1080.0),
        ];
        assert_eq!(choose(&list), Detection::Found(list[1].clone()));
    }

    #[test]
    fn titles() {
        assert_eq!(
            classify_title("PowerPoint Slide Show - [Lecture 3.pptx]"),
            WindowKind::Slideshow
        );
        assert_eq!(
            classify_title("PowerPoint Presenter View - [Lecture 3.pptx]"),
            WindowKind::Presenter
        );
        assert_eq!(classify_title("Lecture 3.pptx"), WindowKind::Other);
        // A deck merely NAMED "slide show" is a normal editing window.
        assert_eq!(classify_title("My slide show.pptx"), WindowKind::Other);
        assert_eq!(
            classify_title("Notes - my slide show draft.pptx"),
            WindowKind::Other
        );
        // Both words → Presenter (capture less, never more).
        assert_eq!(
            classify_title("PowerPoint Presenter View - [My slide show.pptx]"),
            WindowKind::Presenter
        );
    }

    #[test]
    fn windows_classes() {
        assert_eq!(classify_class("screenClass"), WindowKind::Slideshow);
        assert_eq!(classify_class("PodiumParent"), WindowKind::Presenter);
        assert_eq!(classify_class("PPTFrameClass"), WindowKind::Other);
    }
}
