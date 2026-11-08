//! Frames: from the capture backend to every viewer's browser.
//!
//! ```text
//! capture (OS thread) ──RawFrame──▶ encoder thread ──JPEG──▶ Hub ──▶ each viewer
//!            (newest frame only)    (skip unchanged,          (watch channel:
//!                                    scale, encode once)        slow viewers just
//!                                                               get the newest)
//! ```
//!
//! Nothing ever queues up: the capture→encoder hand-off keeps only one frame,
//! and the Hub holds only the latest encoded frame. A viewer on slow Wi-Fi
//! simply skips the frames it was too slow for.

use crate::config::Quality;
use bytes::Bytes;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// A captured frame in BGRA byte order.
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    /// Bytes per row (may include padding).
    pub stride: usize,
    pub bgra: Vec<u8>,
}

/// A JPEG ready to send.
#[derive(Debug)]
pub struct EncodedFrame {
    pub seq: u64,
    pub jpeg: Bytes,
}

/// What viewers should see right now. Sent to viewers as JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewerMode {
    /// Broadcast running, but no slideshow yet.
    Waiting,
    Live,
    Paused,
    Blank,
    /// Broadcast stopped (or this link is old).
    Ended,
}

/// Shares the newest frame and the viewer mode with all viewers.
pub struct Hub {
    frame_tx: watch::Sender<Option<Arc<EncodedFrame>>>,
    mode_tx: watch::Sender<ViewerMode>,
    /// The newest encoded frame, even while paused (sent on Resume).
    latest: Mutex<Option<Arc<EncodedFrame>>>,
    seq: AtomicU64,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        Self {
            frame_tx: watch::channel(None).0,
            mode_tx: watch::channel(ViewerMode::Ended).0,
            latest: Mutex::new(None),
            seq: AtomicU64::new(0),
        }
    }

    pub fn mode(&self) -> ViewerMode {
        *self.mode_tx.borrow()
    }

    /// Changes what viewers see. Resuming re-sends the newest frame so
    /// viewers catch up with anything that changed while paused.
    pub fn set_mode(&self, mode: ViewerMode) {
        let old = self.mode();
        if old == mode {
            return;
        }
        if mode == ViewerMode::Ended {
            // A new broadcast must never show a slide from an old one.
            *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = None;
            self.frame_tx.send_replace(None);
        }
        self.mode_tx.send_replace(mode);
        if mode == ViewerMode::Live && old != ViewerMode::Live {
            let latest = self
                .latest
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if latest.is_some() {
                self.frame_tx.send_replace(latest);
            }
        }
    }

    /// Called by the encoder with a freshly encoded frame.
    pub fn push(&self, jpeg: Vec<u8>) {
        let frame = Arc::new(EncodedFrame {
            seq: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
            jpeg: Bytes::from(jpeg),
        });
        *self.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame.clone());
        if self.mode() == ViewerMode::Live {
            self.frame_tx.send_replace(Some(frame));
        }
    }

    /// For each viewer connection.
    pub fn subscribe(
        &self,
    ) -> (
        watch::Receiver<Option<Arc<EncodedFrame>>>,
        watch::Receiver<ViewerMode>,
    ) {
        (self.frame_tx.subscribe(), self.mode_tx.subscribe())
    }
}

// ---------- Encoder ----------

/// Encoder settings derived from the presenter's Quality choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeSettings {
    pub max_width: u32,
    pub jpeg_quality: u8,
}

impl From<&Quality> for EncodeSettings {
    fn from(q: &Quality) -> Self {
        match q {
            Quality::Low => Self {
                max_width: 1280,
                jpeg_quality: 60,
            },
            Quality::Balanced => Self {
                max_width: 1600,
                jpeg_quality: 72,
            },
            Quality::High => Self {
                max_width: 1920,
                jpeg_quality: 82,
            },
        }
    }
}

/// Hands frames from a capture backend to the encoder thread.
#[derive(Clone)]
pub struct FrameSink {
    tx: SyncSender<RawFrame>,
}

impl FrameSink {
    /// Offers a frame. If the encoder is still busy with the previous one,
    /// this frame is dropped (the next one will be newer anyway).
    pub fn offer(&self, frame: RawFrame) {
        match self.tx.try_send(frame) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

/// Starts the encoder thread. It stops by itself when every `FrameSink`
/// clone has been dropped.
pub fn start_encoder(hub: Arc<Hub>, settings: Arc<Mutex<EncodeSettings>>) -> FrameSink {
    let (tx, rx) = sync_channel::<RawFrame>(1);
    std::thread::Builder::new()
        .name("sliderelay-encoder".into())
        .spawn(move || encoder_loop(rx, hub, settings))
        .expect("could not start encoder thread");
    FrameSink { tx }
}

fn encoder_loop(rx: Receiver<RawFrame>, hub: Arc<Hub>, settings: Arc<Mutex<EncodeSettings>>) {
    let mut last_signature: Vec<u8> = Vec::new();
    let mut last_settings: Option<EncodeSettings> = None;
    while let Ok(frame) = rx.recv() {
        let s = *settings.lock().unwrap_or_else(|e| e.into_inner());
        // Skip frames identical to the previous one (nothing changed on the
        // slide), unless the quality settings changed.
        let sig = signature(&frame);
        if sig == last_signature && last_settings == Some(s) {
            continue;
        }
        match encode(&frame, s) {
            Ok(jpeg) => {
                hub.push(jpeg);
                last_signature = sig;
                last_settings = Some(s);
            }
            Err(e) => tracing::warn!("frame encode failed: {e}"),
        }
    }
    tracing::debug!("encoder stopped");
}

/// A cheap fingerprint: a sparse grid of pixels. Any real slide change
/// (new slide, animation step, video frame) changes at least one of them.
pub fn signature(frame: &RawFrame) -> Vec<u8> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut sig = Vec::with_capacity(64 * 64 * 3 + 8);
    sig.extend_from_slice(&frame.width.to_le_bytes());
    sig.extend_from_slice(&frame.height.to_le_bytes());
    if w == 0 || h == 0 {
        return sig;
    }
    let step_x = (w / 96).max(1);
    let step_y = (h / 64).max(1);
    for y in (0..h).step_by(step_y) {
        let row = y * frame.stride;
        for x in (0..w).step_by(step_x) {
            let i = row + x * 4;
            if let Some(px) = frame.bgra.get(i..i + 3) {
                sig.extend_from_slice(px);
            }
        }
    }
    sig
}

/// Scales (if wider than `max_width`) and encodes to JPEG.
pub fn encode(frame: &RawFrame, s: EncodeSettings) -> anyhow::Result<Vec<u8>> {
    // Tightly packed copy (drop row padding) for the resizer/encoder.
    let row_bytes = frame.width as usize * 4;
    let packed: std::borrow::Cow<[u8]> = if frame.stride == row_bytes {
        std::borrow::Cow::Borrowed(&frame.bgra[..row_bytes * frame.height as usize])
    } else {
        let mut v = Vec::with_capacity(row_bytes * frame.height as usize);
        for y in 0..frame.height as usize {
            let start = y * frame.stride;
            v.extend_from_slice(&frame.bgra[start..start + row_bytes]);
        }
        std::borrow::Cow::Owned(v)
    };

    let (w, h, pixels) = if frame.width > s.max_width {
        let new_w = s.max_width;
        let new_h = ((frame.height as u64 * new_w as u64) / frame.width as u64).max(1) as u32;
        let src = fast_image_resize::images::ImageRef::new(
            frame.width,
            frame.height,
            &packed,
            fast_image_resize::PixelType::U8x4,
        )?;
        let mut dst =
            fast_image_resize::images::Image::new(new_w, new_h, fast_image_resize::PixelType::U8x4);
        let mut resizer = fast_image_resize::Resizer::new();
        resizer.resize(
            &src,
            &mut dst,
            &fast_image_resize::ResizeOptions::new().resize_alg(
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
            ),
        )?;
        (new_w, new_h, std::borrow::Cow::Owned(dst.into_vec()))
    } else {
        (frame.width, frame.height, packed)
    };

    let mut out = Vec::with_capacity((w * h / 8) as usize);
    let encoder = jpeg_encoder::Encoder::new(&mut out, s.jpeg_quality);
    encoder.encode(
        &pixels,
        u16::try_from(w)?,
        u16::try_from(h)?,
        jpeg_encoder::ColorType::Bgra,
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RawFrame {
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            bgra.extend_from_slice(&color);
        }
        RawFrame {
            width: w,
            height: h,
            stride: w as usize * 4,
            bgra,
        }
    }

    #[test]
    fn encodes_and_scales_down() {
        let f = solid(2000, 1000, [255, 0, 0, 255]);
        let jpeg = encode(
            &f,
            EncodeSettings {
                max_width: 1000,
                jpeg_quality: 70,
            },
        )
        .unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]); // JPEG magic
                                               // Width is stored in the SOF0 marker; just check it decodes small.
        assert!(jpeg.len() < 100_000);
    }

    #[test]
    fn handles_row_padding() {
        let mut f = solid(10, 4, [1, 2, 3, 255]);
        // Re-pack with 8 bytes of padding per row.
        let mut padded = Vec::new();
        for row in f.bgra.chunks(40) {
            padded.extend_from_slice(row);
            padded.extend_from_slice(&[0; 8]);
        }
        f.bgra = padded;
        f.stride = 48;
        assert!(encode(
            &f,
            EncodeSettings {
                max_width: 1920,
                jpeg_quality: 70
            }
        )
        .is_ok());
    }

    #[test]
    fn signature_detects_change_only() {
        let a = solid(320, 180, [10, 10, 10, 255]);
        let b = solid(320, 180, [10, 10, 10, 255]);
        let mut c = solid(320, 180, [10, 10, 10, 255]);
        // Change a block in the middle, like a new bullet appearing.
        for y in 80..100 {
            for x in 100..200 {
                let i = y * c.stride + x * 4;
                c.bgra[i] = 200;
            }
        }
        assert_eq!(signature(&a), signature(&b));
        assert_ne!(signature(&a), signature(&c));
    }

    #[test]
    fn pause_holds_frames_and_resume_catches_up() {
        let hub = Hub::new();
        let (frames, _) = hub.subscribe();
        hub.set_mode(ViewerMode::Live);
        hub.push(vec![1]);
        assert_eq!(frames.borrow().as_ref().unwrap().jpeg.as_ref(), &[1]);

        hub.set_mode(ViewerMode::Paused);
        hub.push(vec![2]); // slide changed while paused
        assert_eq!(frames.borrow().as_ref().unwrap().jpeg.as_ref(), &[1]);

        hub.set_mode(ViewerMode::Live);
        assert_eq!(frames.borrow().as_ref().unwrap().jpeg.as_ref(), &[2]);
    }

    #[test]
    fn blank_sends_no_frames_and_end_forgets_them() {
        let hub = Hub::new();
        let (frames, _) = hub.subscribe();
        hub.set_mode(ViewerMode::Live);
        hub.push(vec![1]);
        hub.set_mode(ViewerMode::Blank);
        hub.push(vec![2]);
        assert_eq!(frames.borrow().as_ref().unwrap().jpeg.as_ref(), &[1]);
        hub.set_mode(ViewerMode::Ended);
        assert!(frames.borrow().is_none());
        // A new broadcast doesn't resurrect the old slide.
        hub.set_mode(ViewerMode::Live);
        assert!(frames.borrow().is_none());
    }

    #[test]
    fn slow_viewer_gets_newest_frame_only() {
        let hub = Hub::new();
        let (mut frames, _) = hub.subscribe();
        hub.set_mode(ViewerMode::Live);
        for i in 0..10u8 {
            hub.push(vec![i]);
        }
        // The viewer was busy; when it looks, it sees only the newest.
        assert!(frames.has_changed().unwrap());
        assert_eq!(
            frames.borrow_and_update().as_ref().unwrap().jpeg.as_ref(),
            &[9]
        );
        assert!(!frames.has_changed().unwrap());
    }
}
