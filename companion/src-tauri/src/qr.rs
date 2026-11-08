//! QR code for the student link, as an SVG string.

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Makes a crisp black-on-white QR code (scannable from the back of a room
/// when shown large on the projector).
pub fn svg_for(url: &str) -> anyhow::Result<String> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M)?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(512, 512)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[cfg(test)]
mod tests {
    #[test]
    fn makes_svg() {
        let svg = super::svg_for("http://192.168.1.20/K7PQ").unwrap();
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
    }
}
