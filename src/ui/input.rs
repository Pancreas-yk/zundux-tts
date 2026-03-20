use crate::app::AppState;
use crate::ui::theme::Theme;
use egui::CornerRadius;

const TEMPLATE_MAX_DISPLAY_LEN: usize = 12;
const TEMPLATE_MAX_VISIBLE_ROWS: usize = 2;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = state.config.theme.clone();

    ui.add_space(theme.spacing_large);

    // -- Speaker selector --
    ui.label(
        egui::RichText::new("SPEAKER")
            .size(10.0)
            .color(theme.color(theme.text_muted)),
    );
    ui.add_space(theme.spacing_small);

    let selected_text = state
        .speakers
        .iter()
        .flat_map(|s| s.styles.iter().map(move |st| (s, st)))
        .find(|(_, st)| st.id == state.config.speaker_id)
        .map(|(s, st)| format!("{} - {}", s.name, st.name))
        .unwrap_or_else(|| format!("Speaker ID: {}", state.config.speaker_id));

    egui::ComboBox::from_id_salt("speaker_select")
        .selected_text(&selected_text)
        .width(ui.available_width() - theme.spacing_medium)
        .show_ui(ui, |ui| {
            for speaker in &state.speakers {
                for style in &speaker.styles {
                    let label = format!("{} - {}", speaker.name, style.name);
                    if ui
                        .selectable_value(&mut state.config.speaker_id, style.id, &label)
                        .changed()
                    {
                        let _ = state.config.save();
                    }
                }
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
                .frame(false)
                // Use Shift+Enter for newlines so that plain Enter can be used to
                // submit text.  This also matches the hint text shown to the user.
                .return_key(egui::KeyboardShortcut::new(
                    egui::Modifiers::SHIFT,
                    egui::Key::Enter,
                )),
        );

        // Auto-focus on the text input when the Input screen is active.
        // Skip the request while IME is composing to avoid resetting the
        // preedit state (egui resets ime_enabled on gained_focus/lost_focus).
        let ime_active = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        if !response.has_focus() && !response.lost_focus() && !ime_active {
            ui.memory_mut(|mem| mem.request_focus(egui::Id::new("main_text_input")));
        }

        if response.has_focus() {
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let shift_held = ui.input(|i| i.modifiers.shift);
            // When the user confirms an IME candidate with Enter, an ImeEvent::Commit
            // is present in the same frame.  In that case the Enter key should not
            // also trigger text submission.
            let ime_commit = ui.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Ime(egui::ImeEvent::Commit(_))))
            });
            if enter_pressed && !shift_held && !ime_commit && !state.input_text.trim().is_empty() {
                state.pending_send = Some(state.input_text.trim().to_string());
                state.input_text.clear();
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
    let max_y = start_y + (row_height * TEMPLATE_MAX_VISIBLE_ROWS as f32) + theme.spacing_small;
    let expanded_max_y = start_y + (row_height * 5.0) + theme.spacing_small;
    let effective_max_y = if state.templates_expanded {
        expanded_max_y
    } else {
        max_y
    };

    let mut overflow_count = 0;

    ui.horizontal_wrapped(|ui| {
        for (i, template) in templates.iter().enumerate() {
            if ui.cursor().top() > effective_max_y {
                overflow_count = templates.len() - i;
                break;
            }

            let display_text = truncate_text(template, TEMPLATE_MAX_DISPLAY_LEN);

            // Template chip button
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

        if overflow_count > 0 {
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
        } else if state.templates_expanded
            && templates.len() > TEMPLATE_MAX_VISIBLE_ROWS * 4
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
