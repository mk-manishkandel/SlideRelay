//! Windows capture with Windows.Graphics.Capture (Windows 10 2004+).
//!
//! PowerPoint's slideshow window has the class name `screenClass` and the
//! Presenter View has `PodiumParent`, in every language. We capture only the
//! `screenClass` window, without the yellow capture border and cursor.

use super::{ActiveCapture, Candidate, CaptureError};
use crate::stream::{FrameSink, RawFrame};
use std::time::Duration;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, IsIconic, IsWindowVisible,
};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

pub fn find_powerpoint_windows() -> Result<Vec<Candidate>, CaptureError> {
    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<HWND>);
        list.push(hwnd);
        BOOL(1)
    }
    unsafe {
        EnumWindows(Some(collect), LPARAM(&mut hwnds as *mut _ as isize))
            .map_err(|e| CaptureError::Failed(e.to_string()))?;
    }

    let mut out = Vec::new();
    for hwnd in hwnds {
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                continue;
            }
        }
        let window = Window::from_raw_hwnd(hwnd.0);
        let is_powerpoint = window
            .process_name()
            .map(|n| n.eq_ignore_ascii_case("POWERPNT.EXE"))
            .unwrap_or(false);
        if !is_powerpoint {
            continue;
        }
        let class = class_name(hwnd);
        let mut rect = RECT::default();
        unsafe {
            let _ = GetWindowRect(hwnd, &mut rect);
        }
        out.push(Candidate {
            id: hwnd.0 as usize as u64,
            kind: super::classify_class(&class),
            title: window_text(hwnd),
            bounds: (
                f64::from(rect.left),
                f64::from(rect.top),
                f64::from(rect.right - rect.left),
                f64::from(rect.bottom - rect.top),
            ),
        });
    }
    Ok(out)
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// Per-capture state living on the capture thread.
struct Handler {
    sink: FrameSink,
    packed: Vec<u8>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = FrameSink;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            sink: ctx.flags,
            packed: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let mut buffer = frame.buffer()?;
        let (width, height) = (buffer.width(), buffer.height());
        let bytes = buffer.as_nopadding_buffer(&mut self.packed).to_vec();
        self.sink.offer(RawFrame {
            width,
            height,
            stride: width as usize * 4,
            bgra: bytes,
        });
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        tracing::info!("slideshow window closed");
        Ok(())
    }
}

struct WinCapture {
    window_id: u64,
    control: Option<CaptureControl<Handler, Box<dyn std::error::Error + Send + Sync>>>,
}

impl ActiveCapture for WinCapture {
    fn window_id(&self) -> u64 {
        self.window_id
    }
    fn is_alive(&self) -> bool {
        self.control.as_ref().is_some_and(|c| !c.is_finished())
    }
    fn stop(mut self: Box<Self>) {
        self.shutdown();
    }
}

impl WinCapture {
    fn shutdown(&mut self) {
        if let Some(control) = self.control.take() {
            if let Err(e) = control.stop() {
                tracing::debug!("capture stop: {e}");
            }
        }
    }
}

impl Drop for WinCapture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn start(
    window: &Candidate,
    sink: FrameSink,
    max_fps: u8,
    _max_width: u32, // scaling happens in the encoder on Windows
) -> Result<Box<dyn ActiveCapture>, CaptureError> {
    let target = Window::from_raw_hwnd(window.id as usize as *mut std::ffi::c_void);
    let interval = Duration::from_millis(1000 / u64::from(max_fps.max(1)));
    let settings = Settings::new(
        target,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(interval),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        sink,
    );
    let control =
        Handler::start_free_threaded(settings).map_err(|e| CaptureError::Failed(e.to_string()))?;
    tracing::info!("capturing slideshow window {:#x}", window.id);
    Ok(Box::new(WinCapture {
        window_id: window.id,
        control: Some(control),
    }))
}
