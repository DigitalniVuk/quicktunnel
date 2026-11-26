fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "windows")]
    embed_windows_resources();
}

#[cfg(target_os = "windows")]
fn embed_windows_resources() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("quicktunnel.ico");
    let ico_bytes = generate_ico_64x64();
    fs::write(&ico_path, ico_bytes).expect("failed to write generated quicktunnel.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("non-UTF8 OUT_DIR path"));
    res.set("ProductName", "QuickTunnel");
    res.set(
        "FileDescription",
        "QuickTunnel - Cloudflare Quick Tunnel Desktop App",
    );
    res.set("CompanyName", "QuickTunnel Contributors");
    res.set("OriginalFilename", "quicktunnel.exe");
    res.set("InternalName", "quicktunnel");
    res.set(
        "LegalCopyright",
        "Copyright (C) 2026 QuickTunnel Contributors. Licensed under GPL-3.0-or-later.",
    );
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    version="0.1.0.0"
    processorArchitecture="*"
    name="QuickTunnel.DesktopApp"
    type="win32"
  />
  <description>QuickTunnel - Cloudflare Quick Tunnel Desktop App</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}" />
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}" />
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
    );

    res.compile().expect("failed to compile Windows resources");
}

#[cfg(target_os = "windows")]
fn generate_ico_64x64() -> Vec<u8> {
    const SIZE: usize = 64;

    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn aa_coverage(distance: f32) -> f32 {
        1.0 - smoothstep(-0.65, 0.65, distance)
    }

    fn sd_rounded_box(px: f32, py: f32, half_w: f32, half_h: f32, radius: f32) -> f32 {
        let qx = px.abs() - half_w + radius;
        let qy = py.abs() - half_h + radius;
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let inside = qx.max(qy).min(0.0);
        outside + inside - radius
    }

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

    fn sd_portal_motif(px: f32, py: f32) -> f32 {
        let outer_arch = sd_arch_ring(px, py, -2.5, 17.5, 12.5, 16.5);
        let inner_arch = sd_arch_ring(px, py, 1.0, 8.8, 4.6, 16.5);
        let threshold_bar = sd_rounded_box(px, py - 16.5, 20.0, 2.2, 2.2);
        outer_arch.min(inner_arch).min(threshold_bar)
    }

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = SIZE as f32 * 0.5;
    const TOP_COLOR: (f32, f32, f32) = (243.0, 128.0, 32.0);
    const BOT_COLOR: (f32, f32, f32) = (217.0, 95.0, 14.0);
    const SHADOW_COLOR: (f32, f32, f32) = (120.0, 42.0, 4.0);

    for y in 0..SIZE {
        let py = y as f32 + 0.5 - center;
        for x in 0..SIZE {
            let px = x as f32 + 0.5 - center;
            let idx = (y * SIZE + x) * 4;

            let bg_dist = sd_rounded_box(px, py, 29.0, 29.0, 13.0);
            let bg_alpha = aa_coverage(bg_dist);
            if bg_alpha <= 0.0 {
                continue;
            }

            let grad_t = ((px * 0.35 + py * 0.65) / 29.0 * 0.5 + 0.5).clamp(0.0, 1.0);
            let mut r = lerp(TOP_COLOR.0, BOT_COLOR.0, grad_t);
            let mut g = lerp(TOP_COLOR.1, BOT_COLOR.1, grad_t);
            let mut b = lerp(TOP_COLOR.2, BOT_COLOR.2, grad_t);

            let shadow_dist = sd_portal_motif(px, py - 1.5);
            let shadow_alpha = (1.0 - smoothstep(-0.5, 2.2, shadow_dist)) * 0.20;
            r = lerp(r, SHADOW_COLOR.0, shadow_alpha);
            g = lerp(g, SHADOW_COLOR.1, shadow_alpha);
            b = lerp(b, SHADOW_COLOR.2, shadow_alpha);

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

    // Standard 32-bit BGRA BMP-DIB ICO container (compatible with all Windows resource compilers).
    let pixel_bytes = SIZE * SIZE * 4;
    let and_mask_row = ((SIZE + 31) / 32) * 4; // 8 bytes per row for 64px
    let and_mask_bytes = and_mask_row * SIZE;
    let dib_header_size = 40usize;
    let image_size = dib_header_size + pixel_bytes + and_mask_bytes;

    let mut out = Vec::with_capacity(6 + 16 + image_size);

    // ICONDIR (6 bytes)
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
    out.extend_from_slice(&1u16.to_le_bytes()); // count = 1

    // ICONDIRENTRY (16 bytes)
    out.push(SIZE as u8); // width
    out.push(SIZE as u8); // height
    out.push(0); // color count
    out.push(0); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // color planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&(image_size as u32).to_le_bytes()); // bytes in resource
    out.extend_from_slice(&(22u32).to_le_bytes()); // offset (6 + 16)

    // BITMAPINFOHEADER (40 bytes)
    out.extend_from_slice(&(dib_header_size as u32).to_le_bytes());
    out.extend_from_slice(&(SIZE as i32).to_le_bytes()); // width
    out.extend_from_slice(&((SIZE * 2) as i32).to_le_bytes()); // height * 2 (XOR + AND masks)
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((pixel_bytes + and_mask_bytes) as u32).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // x ppm
    out.extend_from_slice(&0i32.to_le_bytes()); // y ppm
    out.extend_from_slice(&0u32.to_le_bytes()); // clr used
    out.extend_from_slice(&0u32.to_le_bytes()); // clr important

    // XOR mask: bottom-up BGRA rows
    for y in (0..SIZE).rev() {
        for x in 0..SIZE {
            let idx = (y * SIZE + x) * 4;
            out.push(rgba[idx + 2]); // B
            out.push(rgba[idx + 1]); // G
            out.push(rgba[idx]); // R
            out.push(rgba[idx + 3]); // A
        }
    }

    // AND mask: bottom-up 1-bit transparency mask
    for y in (0..SIZE).rev() {
        let mut row = vec![0u8; and_mask_row];
        for x in 0..SIZE {
            let alpha = rgba[(y * SIZE + x) * 4 + 3];
            if alpha == 0 {
                row[x / 8] |= 1 << (7 - (x % 8));
            }
        }
        out.extend_from_slice(&row);
    }

    out
}
