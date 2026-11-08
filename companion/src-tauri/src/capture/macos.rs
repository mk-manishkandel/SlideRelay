//! macOS capture with ScreenCaptureKit (macOS 12.3+).
//!
//! We capture a single window with a "desktop independent window" filter:
//! only that window's pixels, even if other windows (our toolbar, Presenter
//! View, notifications) are on top of it.

use super::{ActiveCapture, Candidate, CaptureError};
use crate::stream::{FrameSink, RawFrame};
use screencapturekit::cm::SCFrameStatus;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::shareable_content::ContentSnapshot;
use screencapturekit::stream::configuration::pixel_format::PixelFormat;
use screencapturekit::stream::delegate_trait::SCStreamDelegateTrait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};

const POWERPOINT_BUNDLE_ID: &str = "com.microsoft.Powerpoint";

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

pub fn has_permission() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub fn request_permission() {
    unsafe {
        CGRequestScreenCaptureAccess();
    }
}

/// ScreenCaptureKit needs CoreGraphics initialised once per process.
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe { screencapturekit::ffi::sc_initialize_core_graphics() });
}

pub fn find_powerpoint_windows() -> Result<Vec<Candidate>, CaptureError> {
    init();
    if !has_permission() {
        return Err(CaptureError::NoPermission);
    }
    let content = SCShareableContent::get().map_err(|e| CaptureError::Failed(e.to_string()))?;
    let ContentSnapshot {
        applications,
        windows,
        ..
    } = content
        .snapshot()
        .ok_or_else(|| CaptureError::Failed("no window list".into()))?;

    let list = windows
        .into_iter()
        // Not only on-screen windows: on a single screen the slideshow lives in
        // its own full-screen Space, and is "off screen" whenever the teacher
        // switches to the PowerPoint panel. It still exists and can be captured.
        .filter(|w| w.frame.size.width > 50.0 && w.frame.size.height > 50.0)
        .filter(|w| {
            w.owning_app_index
                .and_then(|i| applications.get(i))
                .is_some_and(|a| {
                    a.bundle_identifier
                        .eq_ignore_ascii_case(POWERPOINT_BUNDLE_ID)
                })
        })
        .map(|w| {
            let title = w.title.unwrap_or_default();
            Candidate {
                id: u64::from(w.window_id),
                kind: super::classify_title(&title),
                title,
                bounds: (
                    w.frame.origin.x,
                    w.frame.origin.y,
                    w.frame.size.width,
                    w.frame.size.height,
                ),
            }
        })
        .collect();
    Ok(list)
}

/// Receives frames on ScreenCaptureKit's queue and hands them on.
struct Output {
    sink: FrameSink,
}

impl SCStreamOutputTrait for Output {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if !matches!(of_type, SCStreamOutputType::Screen) {
            return;
        }
        // Only "complete" frames carry new pixels; idle frames repeat the last one.
        if !matches!(sample.frame_status(), Some(SCFrameStatus::Complete)) {
            return;
        }
        let Some(pixels) = sample.pixel_buffer() else {
            return;
        };
        let Ok(guard) = pixels.lock(CVPixelBufferLockFlags::READ_ONLY) else {
            return;
        };
        // SAFETY: the buffer stays locked (and alive) while `guard` exists.
        let Some(data) = (unsafe { guard.as_slice() }) else {
            return;
        };
        self.sink.offer(RawFrame {
            width: guard.width() as u32,
            height: guard.height() as u32,
            stride: guard.bytes_per_row(),
            bgra: data.to_vec(),
        });
    }
}

/// Notices when macOS stops the stream (window closed, permission revoked).
struct Delegate {
    alive: Arc<AtomicBool>,
}

impl SCStreamDelegateTrait for Delegate {
    fn did_stop_with_error(&self, error: SCError) {
        tracing::info!("window capture stopped: {error}");
        self.alive.store(false, Ordering::SeqCst);
    }
}

struct MacCapture {
    window_id: u64,
    stream: Option<SCStream>,
    alive: Arc<AtomicBool>,
}

impl ActiveCapture for MacCapture {
    fn window_id(&self) -> u64 {
        self.window_id
    }
    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
    fn stop(mut self: Box<Self>) {
        self.shutdown();
    }
}

impl MacCapture {
    fn shutdown(&mut self) {
        if let Some(stream) = self.stream.take() {
            if let Err(e) = stream.stop_capture() {
                tracing::debug!("stop_capture: {e}");
            }
        }
    }
}

impl Drop for MacCapture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn start(
    window: &Candidate,
    sink: FrameSink,
    max_fps: u8,
    max_width: u32,
) -> Result<Box<dyn ActiveCapture>, CaptureError> {
    init();
    let fail = |e: SCError| CaptureError::Failed(e.to_string());
    let content = SCShareableContent::get().map_err(fail)?;
    let live = content
        .windows()
        .into_iter()
        .find(|w| u64::from(w.window_id()) == window.id)
        .ok_or_else(|| CaptureError::Failed("slideshow window disappeared".into()))?;

    // Capture at Retina resolution, but no wider than the quality setting.
    let (_, _, w_pt, h_pt) = window.bounds;
    let mut width = (w_pt * 2.0).round().max(2.0);
    let mut height = (h_pt * 2.0).round().max(2.0);
    if width > f64::from(max_width) {
        height = (height * f64::from(max_width) / width).round();
        width = f64::from(max_width);
    }

    let filter = SCContentFilter::create()
        .with_window(&live)
        .build()
        .map_err(fail)?;
    let config = SCStreamConfiguration::new()
        .with_width(width as u32)
        .with_height(height as u32)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(false)
        .with_queue_depth(3)
        .with_minimum_frame_interval(&CMTime::new(1, i32::from(max_fps.max(1))));

    let alive = Arc::new(AtomicBool::new(true));
    let mut stream = SCStream::new_with_delegate(
        &filter,
        &config,
        Delegate {
            alive: alive.clone(),
        },
    )
    .map_err(fail)?;
    stream
        .add_output_handler(Output { sink }, SCStreamOutputType::Screen)
        .map_err(fail)?;
    stream.start_capture().map_err(fail)?;
    tracing::info!(
        "capturing slideshow window {} ({}x{})",
        window.id,
        width,
        height
    );

    Ok(Box::new(MacCapture {
        window_id: window.id,
        stream: Some(stream),
        alive,
    }))
}
