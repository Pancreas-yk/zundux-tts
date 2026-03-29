use crate::app::AppState;
use crate::ui::theme::Theme;
use egui::CornerRadius;

const TEMPLATE_MAX_DISPLAY_LEN: usize = 12;
const TEMPLATE_MAX_VISIBLE_ROWS: usize = 2;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = state.config.theme.clone();

    ui.add_space(theme.spacing_large);

    // -- Preset chips --
    let chip_rounding = CornerRadius::same(theme.chip_rounding as u8);
    ui.horizontal_wrapped(|ui| {
        for i in 0..state.config.presets.len() {
            let name = state.config.presets[i].name.clone();
            let is_active = state.active_preset_idx == Some(i);
            let text_color = if is_active {
                theme.color(theme.accent)
            } else {
                theme.color(theme.text_secondary)
            };
            let btn = ui.add(
                egui::Button::new(
                    egui::RichText::new(&name).color(text_color).size(11.0),
                )
                .corner_radius(chip_rounding)
                .fill(theme.color(theme.chip_background)),
            );
            if btn.clicked() && !is_active {
                state.active_preset_idx = Some(i);
                state.config.speaker_id = state.config.presets[i].speaker_id;
                state.config.synth_params = state.config.presets[i].synth_params.clone();
                let _ = state.config.save();
            }
        }
        if state.config.presets.is_empty() {
            ui.label(
                egui::RichText::new("設定でプリセットを作成してください")
                    .size(10.0)
                    .color(theme.color(theme.text_muted)),
            );
        }
    });

    ui.add_space(theme.spacing_large);

    // -- Text input --
    let input_frame = egui::Frame::NONE
        .fill(theme.color(theme.input_background))
        .corner_radius(CornerRadius::same(theme.input_rounding as u8))
        .inner_margin(egui::Margin::same(theme.spacing_medium as i8));

    input_frame.show(ui, |ui| {
        let response = ui.add(
            egui::TextEdit::multiline(&mut state.input_text)
                .id(egui::Id::new("main_text_input"))
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("テキストを入力してEnterで送信 (Shift+Enterで改行)")
                .frame(false),
        );

        // Auto-focus only on the first frame (avoid re-requesting focus every
        // frame, which resets IME preedit state and breaks Japanese input).
        if state.needs_initial_focus {
            ui.memory_mut(|mem| mem.request_focus(egui::Id::new("main_text_input")));
            state.needs_initial_focus = false;
        }

        if response.has_focus() {
            // Skip Enter-to-send when IME is actively composing or just committed.
            // Enabled/Disabled events (fired by fcitx5 on mode switch) are excluded so
            // they don't accidentally block Enter-to-send during normal typing.
            let ime_active = ui.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Ime(egui::ImeEvent::Preedit(_))
                            | egui::Event::Ime(egui::ImeEvent::Commit(_))
                    )
                })
            });

            if !ime_active {
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let shift_held = ui.input(|i| i.modifiers.shift);
                if enter_pressed && !shift_held && !state.input_text.trim().is_empty() {
                    state.pending_send = Some(state.input_text.trim().to_string());
                    state.input_text.clear();
                }
            }
        }
    });

    ui.add_space(theme.spacing_small);

    // -- Mic toggle + status --
    ui.horizontal(|ui| {
        let (mic_label, mic_color) = if state.mic_passthrough {
            ("MIC: ON", theme.color(theme.status_ok))
        } else {
            ("MIC: OFF", theme.color(theme.text_muted))
        };
        let mic_btn = ui.add(
            egui::Button::new(egui::RichText::new(mic_label).size(10.0).color(mic_color))
                .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                .fill(theme.color(theme.chip_background)),
        );
        mic_btn.clone().on_hover_text(if state.mic_passthrough {
            "クリックでずんだもんモードに戻る"
        } else {
            "クリックで自分のマイクに切り替え"
        });
        if mic_btn.clicked() {
            state.pending_toggle_mic = true;
        }

        // Stop speaking button (visible when playing or synthesizing)
        if state.is_playing || state.is_synthesizing {
            let stop_btn = ui.add(
                egui::Button::new(
                    egui::RichText::new("STOP")
                        .size(10.0)
                        .color(theme.color(theme.status_error)),
                )
                .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                .fill(theme.color(theme.chip_background)),
            );
            stop_btn.clone().on_hover_text("発話を停止する");
            if stop_btn.clicked() {
                state.pending_stop_speaking = true;
            }
        }

        if state.is_synthesizing {
            ui.label(
                egui::RichText::new("合成中...")
                    .size(10.0)
                    .color(theme.color(theme.accent)),
            );
        }
    });

    ui.add_space(theme.spacing_small);

    show_template_chips(ui, state, &theme);

    ui.add_space(theme.spacing_medium);
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

fn show_template_chips(ui: &mut egui::Ui, state: &mut AppState, theme: &Theme) {
    let templates = state.config.templates.clone();
    let chip_rounding = CornerRadius::same(theme.chip_rounding as u8);

    let start_y = ui.cursor().top();
    let row_height = 28.0;
    let collapsed_max_y =
        start_y + (row_height * TEMPLATE_MAX_VISIBLE_ROWS as f32) + theme.spacing_small;

    // Count how many templates would overflow the collapsed view without rendering them.
    // We track this outside the horizontal_wrapped closure so both the chip loop and
    // the expand/collapse button can use the same value.
    let mut overflow_count = 0;
    let mut had_overflow = false; // true if any chip was clipped even when expanded before

    ui.horizontal_wrapped(|ui| {
        for (i, template) in templates.iter().enumerate() {
            // Clip when collapsed and cursor has passed the allowed height.
            if !state.templates_expanded && ui.cursor().top() > collapsed_max_y {
                overflow_count = templates.len() - i;
                break;
            }

            let display_text = truncate_text(template, TEMPLATE_MAX_DISPLAY_LEN);
            let btn = ui.add(
                egui::Button::new(
                    egui::RichText::new(&display_text)
                        .color(theme.color(theme.text_secondary))
                        .size(11.0),
                )
                .corner_radius(chip_rounding)
                .fill(theme.color(theme.chip_background)),
            );
            if template.chars().count() > TEMPLATE_MAX_DISPLAY_LEN {
                btn.clone().on_hover_text(template);
            }
            if btn.clicked() {
                state.pending_send = Some(template.trim().to_string());
                state.input_text.clear();
            }
        }

        // Expand button (collapsed state with overflow)
        if overflow_count > 0 {
            had_overflow = true;
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("+{} more", overflow_count))
                            .color(theme.color(theme.text_muted))
                            .size(11.0),
                    )
                    .corner_radius(chip_rounding)
                    .fill(theme.color(theme.chip_background)),
                )
                .clicked()
            {
                state.templates_expanded = true;
            }
        }

        // Collapse button (expanded state) — shown whenever there is something to hide
        if state.templates_expanded && !had_overflow {
            // Check whether collapsing would actually hide anything by comparing
            // current cursor height to the collapsed threshold.
            let would_clip = ui.cursor().top() > collapsed_max_y
                || templates.len() > TEMPLATE_MAX_VISIBLE_ROWS * 3;
            if would_clip
                && ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Show less")
                                .color(theme.color(theme.text_muted))
                                .size(11.0),
                        )
                        .corner_radius(chip_rounding)
                        .fill(theme.color(theme.chip_background)),
                    )
                    .clicked()
            {
                state.templates_expanded = false;
            }
        }

        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("+ Add")
                        .color(theme.color(theme.text_muted))
                        .size(11.0),
                )
                .corner_radius(chip_rounding)
                .fill(theme.color(theme.chip_background)),
            )
            .clicked()
        {
            state.adding_template = true;
        }
    });

    if state.adding_template {
        ui.horizontal(|ui| {
            let response = ui.text_edit_singleline(&mut state.new_template_text);
            if ui.button("OK").clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                if !state.new_template_text.trim().is_empty() {
                    state
                        .config
                        .templates
                        .push(state.new_template_text.trim().to_string());
                    let _ = state.config.save();
                }
                state.new_template_text.clear();
                state.adding_template = false;
            }
            if ui.button("Cancel").clicked() {
                state.new_template_text.clear();
                state.adding_template = false;
            }
        });
    }
}
