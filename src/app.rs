use eframe::egui;

use crate::theme::apply_theme;
use crate::tunnel::TunnelManager;

pub struct QuickTunnelApp {
    _manager: TunnelManager,
}

impl QuickTunnelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx, true);
        let ctx = cc.egui_ctx.clone();
        let manager = TunnelManager::new(Some(std::sync::Arc::new(move || {
            ctx.request_repaint();
        })));
        Self { _manager: manager }
    }
}

impl eframe::App for QuickTunnelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("QuickTunnel");
        });
    }
}
