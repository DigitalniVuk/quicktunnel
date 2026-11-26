const ICON_SIZE: usize = 64;

/// Smoothly interpolates between 0.0 and 1.0 across `[edge0, edge1]`.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Converts a signed distance (in pixels, negative inside) to an anti-aliased alpha coverage.
fn aa_coverage(distance: f32) -> f32 {
    1.0 - smoothstep(-0.65, 0.65, distance)
}

/// Exact signed distance to an axis-aligned rounded box centered at the origin.
fn sd_rounded_box(px: f32, py: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
    let qx = px.abs() - half_w + radius;
    let qy = py.abs() - half_h + radius;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let inside = qx.max(qy).min(0.0);
    outside + inside - radius
}

/// Exact signed distance to a solid tunnel arch (semicircular crown at `(0, crown_y)` with
/// `radius`, vertical sides extending down to a flat base at `y_base`).
fn sd_tunnel_arch(px: f32, py: f32, crown_y: f32, radius: f32, y_base: f32) -> f32 {
    let d_cap = if py < crown_y {
        px.hypot(py - crown_y) - radius
    } else {
        px.abs() - radius
    };
    let d_bot = py - y_base;
    if d_cap > 0.0 && d_bot > 0.0 {
        d_cap.hypot(d_bot)
    } else {
        d_cap.max(d_bot)
    }
}

/// Signed distance to a hollow tunnel arch ring open along the bottom baseline.
fn sd_arch_ring(
    px: f32,
    py: f32,
    crown_y: f32,
    outer_radius: f32,
    inner_radius: f32,
    y_base: f32,
) -> f32 {
    let d_outer = sd_tunnel_arch(px, py, crown_y, outer_radius, y_base);
    let d_inner_opening = if py < crown_y {
        px.hypot(py - crown_y) - inner_radius
    } else {
        px.abs() - inner_radius
    };
    d_outer.max(-d_inner_opening)
}

/// Combined signed distance for the stylized concentric tunnel portal motif.
fn sd_portal_motif(px: f32, py: f32) -> f32 {
    let outer_arch = sd_arch_ring(px, py, -2.5, 17.5, 12.5, 16.5);
    let inner_arch = sd_arch_ring(px, py, 1.0, 8.8, 4.6, 16.5);
    let threshold_bar = sd_rounded_box(px, py - 16.5, 20.0, 2.2, 2.2);
    outer_arch.min(inner_arch).min(threshold_bar)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Procedurally renders the 64x64 anti-aliased RGBA QuickTunnel application icon.
pub fn app_icon() -> eframe::egui::IconData {
    let mut rgba = vec![0u8; ICON_SIZE * ICON_SIZE * 4];
    let center = ICON_SIZE as f32 * 0.5;

    // Cloudflare orange gradient endpoints: #F38020 -> #D95F0E
    const TOP_COLOR: (f32, f32, f32) = (243.0, 128.0, 32.0);
    const BOT_COLOR: (f32, f32, f32) = (217.0, 95.0, 14.0);
    const SHADOW_COLOR: (f32, f32, f32) = (120.0, 42.0, 4.0);

    for y in 0..ICON_SIZE {
        let py = y as f32 + 0.5 - center;
        for x in 0..ICON_SIZE {
            let px = x as f32 + 0.5 - center;
            let idx = (y * ICON_SIZE + x) * 4;

            let bg_dist = sd_rounded_box(px, py, 29.0, 29.0, 13.0);
            let bg_alpha = aa_coverage(bg_dist);
            if bg_alpha <= 0.0 {
                continue;
            }

            let grad_t = ((px * 0.35 + py * 0.65) / 29.0 * 0.5 + 0.5).clamp(0.0, 1.0);
            let mut r = lerp(TOP_COLOR.0, BOT_COLOR.0, grad_t);
            let mut g = lerp(TOP_COLOR.1, BOT_COLOR.1, grad_t);
            let mut b = lerp(TOP_COLOR.2, BOT_COLOR.2, grad_t);

            // Subtle downward shadow behind the white tunnel portal for contrast.
            let shadow_dist = sd_portal_motif(px, py - 1.5);
            let shadow_alpha = (1.0 - smoothstep(-0.5, 2.2, shadow_dist)) * 0.20;
            r = lerp(r, SHADOW_COLOR.0, shadow_alpha);
            g = lerp(g, SHADOW_COLOR.1, shadow_alpha);
            b = lerp(b, SHADOW_COLOR.2, shadow_alpha);

            // Crisp white tunnel portal foreground.
            let motif_alpha = aa_coverage(sd_portal_motif(px, py));
            r = lerp(r, 255.0, motif_alpha);
            g = lerp(g, 255.0, motif_alpha);
            b = lerp(b, 255.0, motif_alpha);

            rgba[idx] = r.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 1] = g.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 2] = b.round().clamp(0.0, 255.0) as u8;
            rgba[idx + 3] = (bg_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    eframe::egui::IconData {
        rgba,
        width: ICON_SIZE as u32,
        height: ICON_SIZE as u32,
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);

        // Outer corners of the rounded square must be fully transparent.
        for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
            assert_eq!(
                pixel_at(&icon, x, y)[3],
                0,
                "expected transparent corner at ({x}, {y})"
            );
        }

        // Center pixel must be fully opaque.
        let center = pixel_at(&icon, 32, 32);
        assert_eq!(center[3], 255, "expected opaque center pixel");

        // Orange rounded-square body pixel should be opaque Cloudflare orange.
        let orange_body = pixel_at(&icon, 10, 10);
        assert_eq!(orange_body[3], 255, "expected opaque orange body pixel");
        assert!(
            orange_body[0] >= 215 && orange_body[1] >= 90 && orange_body[1] <= 145 && orange_body[2] <= 45,
            "expected Cloudflare orange tone at (10, 10), got {orange_body:?}"
        );

        // Top crown of the outer white arch should be opaque white.
        let arch_crown = pixel_at(&icon, 32, 14);
        assert_eq!(
            arch_crown,
            [255, 255, 255, 255],
            "expected crisp white tunnel arch crown at (32, 14)"
        );
    }
}
