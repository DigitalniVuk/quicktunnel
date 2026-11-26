const APP_ICON_PNG: &[u8] = include_bytes!("../assets/quicktunnel-128.png");

/// Decodes the embedded 128x128 RGBA QuickTunnel application icon for window/taskbar chrome.
pub fn app_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(APP_ICON_PNG)
        .expect("embedded assets/quicktunnel-128.png must be a valid PNG")
}

/// Decodes the embedded QuickTunnel logo as an `egui::ColorImage` for rendering in the UI.
pub fn app_logo_color_image() -> eframe::egui::ColorImage {
    let icon = app_icon();
    eframe::egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    fn pixel_at(icon: &eframe::egui::IconData, x: usize, y: usize) -> [u8; 4] {
        let idx = (y * icon.width as usize + x) * 4;
        [
            icon.rgba[idx],
            icon.rgba[idx + 1],
            icon.rgba[idx + 2],
            icon.rgba[idx + 3],
        ]
    }

    #[test]
    fn app_icon_dimensions_and_visual_structure() {
        let icon = app_icon();
        assert_eq!(icon.width, 128);
        assert_eq!(icon.height, 128);
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);

        // Outer corners of the squircle badge must be fully transparent.
        for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
            assert_eq!(
                pixel_at(&icon, x, y)[3],
                0,
                "expected transparent corner at ({x}, {y})"
            );
        }

        // Center pixel lies on the white upward beam and must be opaque white.
        let center = pixel_at(&icon, 64, 64);
        assert_eq!(center[3], 255, "expected opaque center pixel");
        assert!(
            center[0] >= 240 && center[1] >= 240 && center[2] >= 240,
            "expected white center beam at (64, 64), got {center:?}"
        );

        // Upper-left squircle body should be opaque Cloudflare orange.
        let orange_body = pixel_at(&icon, 24, 24);
        assert_eq!(orange_body[3], 255, "expected opaque orange body pixel");
        assert!(
            orange_body[0] >= 210
                && orange_body[1] >= 80
                && orange_body[1] <= 170
                && orange_body[2] <= 60,
            "expected Cloudflare orange tone at (24, 24), got {orange_body:?}"
        );

        // UI logo image matches icon dimensions.
        let logo = app_logo_color_image();
        assert_eq!(logo.size, [128, 128]);
    }

    #[test]
    fn build_icon_assets_are_valid() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        // Verify 512x512 PNG logo asset exists and decodes cleanly.
        let png_bytes = fs::read(manifest_dir.join("assets/quicktunnel.png"))
            .expect("assets/quicktunnel.png must exist");
        let png_icon = eframe::icon_data::from_png_bytes(&png_bytes)
            .expect("assets/quicktunnel.png must be a valid PNG");
        assert_eq!((png_icon.width, png_icon.height), (512, 512));

        // Verify multi-resolution Windows ICO header and embedded sizes.
        let ico_bytes = fs::read(manifest_dir.join("assets/quicktunnel.ico"))
            .expect("assets/quicktunnel.ico must exist");
        assert!(ico_bytes.len() > 22, "ICO file too small");
        assert_eq!(&ico_bytes[0..4], &[0, 0, 1, 0], "invalid ICO header");
        let count = u16::from_le_bytes([ico_bytes[4], ico_bytes[5]]) as usize;
        assert!(count >= 5, "expected multi-resolution ICO, got {count} entries");

        let mut sizes: Vec<u16> = Vec::with_capacity(count);
        for i in 0..count {
            let off = 6 + i * 16;
            let w = ico_bytes[off];
            let h = ico_bytes[off + 1];
            assert_eq!(w, h, "ICO entry {i} must be square");
            sizes.push(if w == 0 { 256 } else { w as u16 });
        }
        for required in [16, 32, 48, 64, 256] {
            assert!(
                sizes.contains(&required),
                "ICO missing required {required}x{required} entry; found {sizes:?}"
            );
        }

        // Verify Linux desktop entry references the icon name.
        let desktop = fs::read_to_string(manifest_dir.join("assets/quicktunnel.desktop"))
            .expect("assets/quicktunnel.desktop must exist");
        assert!(desktop.contains("Icon=quicktunnel"));
        assert!(desktop.contains("StartupWMClass=quicktunnel"));
    }
}
