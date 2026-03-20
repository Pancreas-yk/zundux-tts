use crate::ui::theme::Theme;
use egui::{Align, CornerRadius, Layout, Sense};

pub fn show(ctx: &egui::Context, theme: &Theme) {
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let titlebar_height = 32.0;

    egui::TopBottomPanel::top("titlebar")
        .exact_height(titlebar_height)
        .frame(egui::Frame::NONE.fill(theme.color(theme.titlebar_background)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let drag_rect = ui.available_rect_before_wrap();
                let drag_response = ui.interact(
                    drag_rect,
                    ui.id().with("titlebar_drag"),
                    Sense::click_and_drag(),
                );
                if drag_response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_response.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                let title_rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "ZunduxTTS",
                    egui::FontId::proportional(11.0),
                    theme.color(theme.titlebar_text),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Close button
                    let close_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(" x ")
                                .size(14.0)
                                .color(theme.color(theme.text_secondary))
                                .family(egui::FontFamily::Monospace),
                        )
                        .min_size(egui::vec2(32.0, 24.0))
                        .frame(false),
                    );
                    if close_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if close_btn.hovered() {
                        ui.painter().rect_filled(
                            close_btn.rect,
                            CornerRadius::same(4),
                            theme.color(theme.status_error),
                        );
                    }

                    // Maximize/restore button
                    let max_text = if is_maximized { " = " } else { " o " };
                    let max_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(max_text)
                                .size(14.0)
                                .color(theme.color(theme.text_secondary))
                                .family(egui::FontFamily::Monospace),
                        )
                        .min_size(egui::vec2(32.0, 24.0))
                        .frame(false),
                    );
                    if max_btn.hovered() {
                        ui.painter().rect_filled(
                            max_btn.rect,
                            CornerRadius::same(4),
                            egui::Color32::from_white_alpha(20),
                        );
                    }
                    if max_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize button
                    let min_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(" - ")
                                .size(14.0)
                                .color(theme.color(theme.text_secondary))
                                .family(egui::FontFamily::Monospace),
                        )
                        .min_size(egui::vec2(32.0, 24.0))
                        .frame(false),
                    );
                    if min_btn.hovered() {
                        ui.painter().rect_filled(
                            min_btn.rect,
                            CornerRadius::same(4),
                            egui::Color32::from_white_alpha(20),
                        );
                    }
                    if min_btn.clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });
}
