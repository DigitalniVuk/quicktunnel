#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quicktunnel::app::QuickTunnelApp;
use quicktunnel::icon::app_icon;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id("quicktunnel")
            .with_title("QuickTunnel — Cloudflare Tunnel")
            .with_inner_size([480.0, 580.0])
            .with_min_inner_size([380.0, 420.0])
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "QuickTunnel",
        options,
        Box::new(|cc| Ok(Box::new(QuickTunnelApp::new(cc)))),
    )
}
