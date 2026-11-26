use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, CornerRadius, CursorIcon, Frame, Layout, Margin, ProgressBar, Response,
    RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};

use crate::icon::app_logo_color_image;
use crate::model::{BinarySource, BinaryStatus, TunnelId, TunnelInfo, TunnelStatus};
use crate::theme::{CONTROL_RADIUS, Palette, apply_theme};
use crate::tunnel::TunnelManager;

const COPY_FEEDBACK_DURATION: Duration = Duration::from_secs(2);
const REPAINT_INTERVAL: Duration = Duration::from_millis(500);
const QUICK_PORTS: [&str; 4] = [":3000", ":5173", ":8000", ":8080"];

pub struct QuickTunnelApp {
    manager: TunnelManager,
    logo_texture: egui::TextureHandle,
    dark_mode: bool,
    address_input: String,
    input_error: Option<String>,
    latest_tunnel_id: Option<TunnelId>,
    expanded_logs: HashSet<TunnelId>,
    latest_copied_at: Option<Instant>,
    tunnel_copied_at: HashMap<TunnelId, Instant>,
    focus_input_once: bool,
}

impl QuickTunnelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        let manager = TunnelManager::new(Some(Arc::new(move || {
            ctx.request_repaint();
        })));
        Self::with_manager(cc, manager)
    }

    pub fn with_manager(cc: &eframe::CreationContext<'_>, manager: TunnelManager) -> Self {
        let dark_mode = true;
        apply_theme(&cc.egui_ctx, dark_mode);
        let logo_texture = cc.egui_ctx.load_texture(
            "quicktunnel_logo",
            app_logo_color_image(),
            egui::TextureOptions::LINEAR,
        );
        Self {
            manager,
            logo_texture,
            dark_mode,
            address_input: String::new(),
            input_error: None,
            latest_tunnel_id: None,
            expanded_logs: HashSet::new(),
            latest_copied_at: None,
            tunnel_copied_at: HashMap::new(),
            focus_input_once: true,
        }
    }

    fn try_start_tunnel(&mut self) {
        let raw = self.address_input.trim().to_string();
        match self.manager.start_tunnel(&raw) {
            Ok(id) => {
                self.latest_tunnel_id = Some(id);
                self.latest_copied_at = None;
                self.input_error = None;
            }
            Err(err) => {
                self.input_error = Some(err);
            }
        }
    }

    fn prune_copy_timers(&mut self, now: Instant) -> bool {
        if let Some(ts) = self.latest_copied_at
            && now.saturating_duration_since(ts) >= COPY_FEEDBACK_DURATION
        {
            self.latest_copied_at = None;
        }
        self.tunnel_copied_at
            .retain(|_, ts| now.saturating_duration_since(*ts) < COPY_FEEDBACK_DURATION);

        self.latest_copied_at.is_some() || !self.tunnel_copied_at.is_empty()
    }

    fn is_latest_copied(&self, now: Instant) -> bool {
        self.latest_copied_at
            .is_some_and(|ts| now.saturating_duration_since(ts) < COPY_FEEDBACK_DURATION)
    }

    fn is_tunnel_copied(&self, id: TunnelId, now: Instant) -> bool {
        self.tunnel_copied_at
            .get(&id)
            .is_some_and(|ts| now.saturating_duration_since(*ts) < COPY_FEEDBACK_DURATION)
    }

    fn render_header(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        binary_status: &BinaryStatus,
        palette: &Palette,
    ) {
        ui.horizontal(|ui| {
            ui.add(
                egui::Image::new(&self.logo_texture).fit_to_exact_size(Vec2::splat(34.0)),
            );
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new("QuickTunnel")
                        .size(18.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(
                    RichText::new("Cloudflare Quick Tunnels")
                        .size(11.5)
                        .color(palette.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let theme_label = if self.dark_mode { "Light" } else { "Dark" };
                let theme_tip = if self.dark_mode {
                    "Switch to light theme"
                } else {
                    "Switch to dark theme"
                };
                if subtle_button(ui, theme_label, palette)
                    .on_hover_text(theme_tip)
                    .clicked()
                {
                    self.dark_mode = !self.dark_mode;
                    apply_theme(ctx, self.dark_mode);
                }

                render_binary_pill(ui, binary_status, palette);
            });
        });
    }

    fn render_setup_banner(
        &mut self,
        ui: &mut Ui,
        binary_status: &BinaryStatus,
        palette: &Palette,
    ) {
        match binary_status {
            BinaryStatus::Ready { .. } => {}
            BinaryStatus::Missing => {
                palette
                    .tinted_frame(palette.warning_soft, palette.warning_border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 8.0;

                        ui.label(
                            RichText::new("cloudflared is required to create Quick Tunnels.")
                                .size(12.5)
                                .color(palette.text),
                        );

                        ui.horizontal(|ui| {
                            if primary_button(
                                ui,
                                "Download cloudflared",
                                palette,
                                Vec2::new(0.0, 28.0),
                            )
                            .clicked()
                            {
                                self.manager.start_binary_download();
                            }

                            if subtle_button(ui, "Recheck", palette).clicked() {
                                self.manager.refresh_binary_status();
                            }
                        });
                    });
            }
            BinaryStatus::Downloading {
                downloaded_bytes,
                total_bytes,
            } => {
                palette
                    .tinted_frame(palette.warning_soft, palette.warning_border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 7.0;

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Downloading cloudflared...")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format_progress(*downloaded_bytes, *total_bytes))
                                        .monospace()
                                        .size(11.5)
                                        .color(palette.text_muted),
                                );
                            });
                        });

                        let bar = match total_bytes {
                            Some(total) if *total > 0 => {
                                let frac = (*downloaded_bytes as f32 / *total as f32).clamp(0.0, 1.0);
                                ProgressBar::new(frac)
                            }
                            _ => ProgressBar::new(0.35).animate(true),
                        };

                        ui.add(
                            bar.desired_height(8.0)
                                .fill(palette.accent)
                                .corner_radius(CornerRadius::same(4)),
                        );
                    });
            }
            BinaryStatus::DownloadFailed(err) => {
                palette
                    .tinted_frame(palette.danger_soft, palette.danger_border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 8.0;

                        ui.label(
                            RichText::new(format!("Download failed: {err}"))
                                .size(12.5)
                                .color(palette.danger),
                        );

                        ui.horizontal(|ui| {
                            if primary_button(ui, "Retry", palette, Vec2::new(0.0, 28.0)).clicked()
                            {
                                self.manager.start_binary_download();
                            }

                            if subtle_button(ui, "Recheck", palette).clicked() {
                                self.manager.refresh_binary_status();
                            }
                        });
                    });
            }
        }
    }

    fn render_create_card(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tunnels: &[TunnelInfo],
        palette: &Palette,
        now: Instant,
    ) {
        palette.card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;

            ui.label(
                RichText::new("Local address")
                    .size(12.0)
                    .strong()
                    .color(palette.text_muted),
            );

            let mut submit = false;
            ui.horizontal(|ui| {
                let btn_width = 104.0;
                let spacing = ui.spacing().item_spacing.x;
                let input_width = (ui.available_width() - btn_width - spacing).max(120.0);

                let input_response = ui.add_sized(
                    [input_width, 32.0],
                    TextEdit::singleline(&mut self.address_input)
                        .hint_text(
                            RichText::new("localhost:3000 or 8080").color(palette.text_subtle),
                        )
                        .desired_width(input_width)
                        .vertical_align(Align::Center)
                        .margin(Margin::symmetric(10, 6)),
                );

                if self.focus_input_once {
                    input_response.request_focus();
                    self.focus_input_once = false;
                }

                if input_response.changed() {
                    self.input_error = None;
                }

                if input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }

                if primary_button(ui, "Start Tunnel", palette, Vec2::new(btn_width, 32.0)).clicked()
                {
                    submit = true;
                }
            });

            if submit {
                self.try_start_tunnel();
            }

            // Quick-fill port chips
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                ui.label(
                    RichText::new("Quick:")
                        .size(11.5)
                        .color(palette.text_subtle),
                );
                for chip in QUICK_PORTS {
                    if chip_button(ui, chip, palette).clicked() {
                        self.address_input = format!("localhost{chip}");
                        self.input_error = None;
                    }
                }
            });

            // Inline error message
            if let Some(err) = &self.input_error {
                ui.label(RichText::new(err).size(12.0).color(palette.danger));
            }

            // Highlighted latest active tunnel strip
            if let Some(latest) = self.latest_active_tunnel(tunnels) {
                ui.add_space(2.0);
                self.render_latest_highlight(ui, ctx, latest, palette, now);
            }
        });
    }

    fn latest_active_tunnel<'a>(&self, tunnels: &'a [TunnelInfo]) -> Option<&'a TunnelInfo> {
        let latest_id = self.latest_tunnel_id?;
        tunnels
            .iter()
            .find(|t| t.id == latest_id && t.status.is_active())
    }

    fn render_latest_highlight(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tunnel: &TunnelInfo,
        palette: &Palette,
        now: Instant,
    ) {
        match &tunnel.status {
            TunnelStatus::Starting => {
                palette
                    .tinted_frame(palette.warning_soft, palette.warning_border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            paint_status_dot(ui, palette.warning, 3.5);
                            ui.label(
                                RichText::new("Starting tunnel...")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.warning),
                            );
                            ui.label(
                                RichText::new(&tunnel.target_url)
                                    .monospace()
                                    .size(11.5)
                                    .color(palette.text_muted),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(egui::Spinner::new().size(13.0).color(palette.warning));
                            });
                        });
                    });
            }
            TunnelStatus::Running { public_url } => {
                palette
                    .tinted_frame(palette.live_soft, palette.live_border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 6.0;

                        ui.horizontal(|ui| {
                            paint_status_dot(ui, palette.live, 3.5);
                            ui.label(
                                RichText::new("Live tunnel")
                                    .size(12.0)
                                    .strong()
                                    .color(palette.live),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(&tunnel.target_url)
                                        .monospace()
                                        .size(11.5)
                                        .color(palette.text_muted),
                                );
                            });
                        });

                        if clickable_url(ui, public_url, palette).clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(public_url.clone()));
                        }

                        ui.horizontal(|ui| {
                            let copied = self.is_latest_copied(now);
                            let copy_resp = if copied {
                                success_button(ui, "Copied!", palette)
                            } else {
                                primary_button(ui, "Copy Link", palette, Vec2::new(0.0, 26.0))
                            };
                            if copy_resp.clicked() {
                                ctx.copy_text(public_url.clone());
                                self.latest_copied_at = Some(now);
                            }

                            if subtle_button(ui, "Open", palette).clicked() {
                                ctx.open_url(egui::OpenUrl::new_tab(public_url.clone()));
                            }
                        });
                    });
            }
            TunnelStatus::Stopping => {
                palette
                    .tinted_frame(palette.stopped_soft, palette.border)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            paint_status_dot(ui, palette.stopped, 3.5);
                            ui.label(
                                RichText::new("Stopping tunnel...")
                                    .size(12.5)
                                    .color(palette.text_muted),
                            );
                        });
                    });
            }
            TunnelStatus::Stopped | TunnelStatus::Failed(_) => {}
        }
    }

    fn render_tunnels_section(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tunnels: &[TunnelInfo],
        active_count: usize,
        palette: &Palette,
        now: Instant,
    ) {
        let has_inactive = tunnels.iter().any(|t| !t.status.is_active());

        // Section header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Tunnels")
                    .size(14.5)
                    .strong()
                    .color(palette.text),
            );

            let (pill_fill, pill_border, pill_text) = if active_count > 0 {
                (palette.live_soft, palette.live_border, palette.live)
            } else {
                (palette.stopped_soft, palette.border, palette.text_muted)
            };
            palette.pill_frame(pill_fill, pill_border).show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{active_count} active"))
                        .size(11.0)
                        .strong()
                        .color(pill_text),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if active_count >= 1
                    && danger_button(ui, "Stop All", palette)
                        .on_hover_text("Stop all active tunnels")
                        .clicked()
                {
                    self.manager.stop_all();
                }

                if has_inactive && subtle_button(ui, "Clear stopped", palette).clicked() {
                    let inactive_ids: HashSet<TunnelId> = tunnels
                        .iter()
                        .filter(|t| !t.status.is_active())
                        .map(|t| t.id)
                        .collect();
                    self.manager.clear_inactive();
                    self.expanded_logs.retain(|id| !inactive_ids.contains(id));
                    self.tunnel_copied_at
                        .retain(|id, _| !inactive_ids.contains(id));
                    if self
                        .latest_tunnel_id
                        .is_some_and(|id| inactive_ids.contains(&id))
                    {
                        self.latest_tunnel_id = None;
                    }
                }
            });
        });

        ui.add_space(4.0);

        ScrollArea::vertical()
            .id_salt("tunnels_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                if tunnels.is_empty() {
                    render_empty_state(ui, palette);
                    return;
                }

                ui.spacing_mut().item_spacing.y = 8.0;
                for tunnel in tunnels {
                    self.render_tunnel_card(ui, ctx, tunnel, palette, now);
                }
            });
    }

    fn render_tunnel_card(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tunnel: &TunnelInfo,
        palette: &Palette,
        now: Instant,
    ) {
        palette.card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;

            // Row 1: status dot + label | monospace uptime
            let (status_color, status_label) = match &tunnel.status {
                TunnelStatus::Starting => (palette.warning, "Starting..."),
                TunnelStatus::Running { .. } => (palette.live, "Live"),
                TunnelStatus::Stopping => (palette.warning, "Stopping..."),
                TunnelStatus::Stopped => (palette.stopped, "Stopped"),
                TunnelStatus::Failed(_) => (palette.danger, "Failed"),
            };

            ui.horizontal(|ui| {
                paint_status_dot(ui, status_color, 4.0);
                ui.label(
                    RichText::new(status_label)
                        .size(12.5)
                        .strong()
                        .color(status_color),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(tunnel.uptime_display())
                            .monospace()
                            .size(11.5)
                            .color(palette.text_muted),
                    );
                });
            });

            // Row 2: local target address
            ui.label(
                RichText::new(&tunnel.target_url)
                    .monospace()
                    .size(12.5)
                    .color(palette.text),
            );

            // Row 3: public URL or error
            match &tunnel.status {
                TunnelStatus::Running { public_url } => {
                    Frame::new()
                        .fill(palette.surface_subtle)
                        .stroke(Stroke::new(1.0_f32, palette.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if clickable_url(ui, public_url, palette).clicked() {
                                ctx.open_url(egui::OpenUrl::new_tab(public_url.clone()));
                            }
                        });
                }
                TunnelStatus::Failed(err) => {
                    ui.label(RichText::new(err).size(12.0).color(palette.danger));
                }
                TunnelStatus::Starting | TunnelStatus::Stopping | TunnelStatus::Stopped => {}
            }

            // Row 4: action buttons
            let logs_open = self.expanded_logs.contains(&tunnel.id);
            ui.horizontal(|ui| {
                if let TunnelStatus::Running { public_url } = &tunnel.status {
                    let copied = self.is_tunnel_copied(tunnel.id, now);
                    let copy_resp = if copied {
                        success_button(ui, "Copied!", palette)
                    } else {
                        subtle_button(ui, "Copy", palette)
                    };
                    if copy_resp.clicked() {
                        ctx.copy_text(public_url.clone());
                        self.tunnel_copied_at.insert(tunnel.id, now);
                    }

                    if subtle_button(ui, "Open", palette).clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(public_url.clone()));
                    }
                }

                let logs_label = if logs_open { "Hide logs" } else { "Logs" };
                if subtle_button(ui, logs_label, palette).clicked() {
                    if logs_open {
                        self.expanded_logs.remove(&tunnel.id);
                    } else {
                        self.expanded_logs.insert(tunnel.id);
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if tunnel.status.is_active() {
                        let is_stopping = matches!(tunnel.status, TunnelStatus::Stopping);
                        ui.add_enabled_ui(!is_stopping, |ui| {
                            if danger_button(ui, "Stop", palette).clicked() {
                                let _ = self.manager.stop_tunnel(tunnel.id);
                            }
                        });
                    } else if subtle_button(ui, "Remove", palette).clicked() {
                        self.manager.remove_tunnel(tunnel.id);
                        self.expanded_logs.remove(&tunnel.id);
                        self.tunnel_copied_at.remove(&tunnel.id);
                        if self.latest_tunnel_id == Some(tunnel.id) {
                            self.latest_tunnel_id = None;
                        }
                    }
                });
            });

            // Expandable logs viewer
            if self.expanded_logs.contains(&tunnel.id) {
                Frame::new()
                    .fill(palette.log_bg)
                    .stroke(Stroke::new(1.0_f32, palette.border))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ScrollArea::vertical()
                            .id_salt(("tunnel_logs", tunnel.id))
                            .max_height(110.0)
                            .stick_to_bottom(true)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if tunnel.recent_logs.is_empty() {
                                    ui.label(
                                        RichText::new("No logs captured yet.")
                                            .monospace()
                                            .size(11.0)
                                            .color(palette.text_subtle),
                                    );
                                } else {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    for line in &tunnel.recent_logs {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(line)
                                                    .monospace()
                                                    .size(11.0)
                                                    .color(palette.text_muted),
                                            )
                                            .wrap(),
                                        );
                                    }
                                }
                            });
                    });
            }
        });
    }
}

impl eframe::App for QuickTunnelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let palette = Palette::for_mode(self.dark_mode);
        let binary_status = self.manager.binary_status();
        let tunnels = self.manager.tunnels();
        let active_count = self.manager.active_count();

        // Clear latest highlight once that tunnel is no longer active or removed
        if let Some(latest_id) = self.latest_tunnel_id {
            let still_active = tunnels
                .iter()
                .any(|t| t.id == latest_id && t.status.is_active());
            if !still_active {
                self.latest_tunnel_id = None;
            }
        }

        let has_active_copy_timer = self.prune_copy_timers(now);

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(palette.bg)
                    .inner_margin(Margin::symmetric(14, 12)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                self.render_header(ui, ctx, &binary_status, &palette);

                if !binary_status.is_ready() {
                    self.render_setup_banner(ui, &binary_status, &palette);
                }

                self.render_create_card(ui, ctx, &tunnels, &palette, now);

                self.render_tunnels_section(ui, ctx, &tunnels, active_count, &palette, now);
            });

        // Only repaint periodically when live uptime, download progress, or copy toast is active.
        let has_active_tunnel = active_count > 0;
        let is_downloading = matches!(binary_status, BinaryStatus::Downloading { .. });
        if has_active_tunnel || is_downloading || has_active_copy_timer {
            ctx.request_repaint_after(REPAINT_INTERVAL);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.manager.stop_all();
    }
}

fn render_binary_pill(ui: &mut Ui, status: &BinaryStatus, palette: &Palette) {
    let (dot, fill, border, text_color, label, tooltip) = match status {
        BinaryStatus::Ready {
            path,
            source,
            version,
        } => {
            let src = match source {
                BinarySource::SystemPath => "System PATH",
                BinarySource::ManagedLocal => "Managed local",
            };
            let tip = match version {
                Some(ver) => format!("cloudflared ({ver})\nSource: {src}\n{}", path.display()),
                None => format!("cloudflared ready\nSource: {src}\n{}", path.display()),
            };
            (
                palette.live,
                palette.live_soft,
                palette.live_border,
                palette.live,
                "Ready",
                tip,
            )
        }
        BinaryStatus::Downloading {
            downloaded_bytes,
            total_bytes,
        } => (
            palette.warning,
            palette.warning_soft,
            palette.warning_border,
            palette.warning,
            "Downloading...",
            format!(
                "Downloading cloudflared ({})",
                format_progress(*downloaded_bytes, *total_bytes)
            ),
        ),
        BinaryStatus::Missing => (
            palette.warning,
            palette.warning_soft,
            palette.warning_border,
            palette.warning,
            "Missing",
            "cloudflared binary not found".to_string(),
        ),
        BinaryStatus::DownloadFailed(err) => (
            palette.danger,
            palette.danger_soft,
            palette.danger_border,
            palette.danger,
            "Missing",
            format!("cloudflared download failed: {err}"),
        ),
    };

    let response = palette
        .pill_frame(fill, border)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                paint_status_dot(ui, dot, 3.5);
                ui.label(
                    RichText::new(label)
                        .size(11.5)
                        .strong()
                        .color(text_color),
                );
            });
        })
        .response;

    response.on_hover_text(tooltip);
}

fn render_empty_state(ui: &mut Ui, palette: &Palette) {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(16, 26))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                ui.label(
                    RichText::new("No tunnels yet")
                        .size(13.5)
                        .strong()
                        .color(palette.text_muted),
                );
                ui.label(
                    RichText::new(
                        "Type a local address above and press Start Tunnel to get an instant public HTTPS link.",
                    )
                    .size(12.0)
                    .color(palette.text_subtle),
                );
            });
        });
}

fn paint_status_dot(ui: &mut Ui, color: Color32, radius: f32) {
    let diameter = radius * 2.0 + 2.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius, color);
}

fn clickable_url(ui: &mut Ui, url: &str, palette: &Palette) -> Response {
    ui.add(
        egui::Label::new(
            RichText::new(url)
                .monospace()
                .size(12.0)
                .color(palette.link)
                .underline(),
        )
        .wrap()
        .sense(Sense::click()),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
    .on_hover_text("Click to open in browser")
}

fn primary_button(ui: &mut Ui, label: &str, palette: &Palette, min_size: Vec2) -> Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = palette.accent;
        widgets.inactive.weak_bg_fill = palette.accent;
        widgets.inactive.bg_stroke = Stroke::NONE;
        widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.on_accent);

        widgets.hovered.bg_fill = palette.accent_hover;
        widgets.hovered.weak_bg_fill = palette.accent_hover;
        widgets.hovered.bg_stroke = Stroke::NONE;
        widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.on_accent);

        widgets.active.bg_fill = palette.accent;
        widgets.active.weak_bg_fill = palette.accent;
        widgets.active.bg_stroke = Stroke::NONE;
        widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.on_accent);

        ui.add(
            egui::Button::new(
                RichText::new(label)
                    .size(12.5)
                    .strong()
                    .color(palette.on_accent),
            )
            .corner_radius(CornerRadius::same(CONTROL_RADIUS))
            .min_size(min_size),
        )
    })
    .inner
}

fn danger_button(ui: &mut Ui, label: &str, palette: &Palette) -> Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = palette.danger_soft;
        widgets.inactive.weak_bg_fill = palette.danger_soft;
        widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.danger_border);
        widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.danger);

        widgets.hovered.bg_fill = palette.danger_soft;
        widgets.hovered.weak_bg_fill = palette.danger_soft;
        widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.danger);
        widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.danger);

        widgets.active.bg_fill = palette.danger_soft;
        widgets.active.weak_bg_fill = palette.danger_soft;
        widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.danger);
        widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.danger);

        ui.add(
            egui::Button::new(RichText::new(label).size(12.0).strong().color(palette.danger))
                .corner_radius(CornerRadius::same(CONTROL_RADIUS)),
        )
    })
    .inner
}

fn success_button(ui: &mut Ui, label: &str, palette: &Palette) -> Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = palette.live_soft;
        widgets.inactive.weak_bg_fill = palette.live_soft;
        widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.live_border);
        widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.live);

        widgets.hovered.bg_fill = palette.live_soft;
        widgets.hovered.weak_bg_fill = palette.live_soft;
        widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.live);
        widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.live);

        ui.add(
            egui::Button::new(RichText::new(label).size(12.0).strong().color(palette.live))
                .corner_radius(CornerRadius::same(CONTROL_RADIUS)),
        )
    })
    .inner
}

fn subtle_button(ui: &mut Ui, label: &str, palette: &Palette) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(palette.text))
            .fill(palette.surface_subtle)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .corner_radius(CornerRadius::same(CONTROL_RADIUS)),
    )
}

fn chip_button(ui: &mut Ui, label: &str, palette: &Palette) -> Response {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .monospace()
                .size(11.5)
                .color(palette.text_muted),
        )
        .fill(palette.chip_bg)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 22.0)),
    )
}

fn format_progress(downloaded: u64, total: Option<u64>) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    let dl_mb = downloaded as f64 / MIB;
    match total {
        Some(total_bytes) if total_bytes > 0 => {
            let total_mb = total_bytes as f64 / MIB;
            format!("{dl_mb:.1} / {total_mb:.1} MB")
        }
        _ => format!("{dl_mb:.1} MB"),
    }
}
