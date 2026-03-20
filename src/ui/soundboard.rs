use crate::app::AppState;
use crate::ui::theme::Theme;
use egui::CornerRadius;

const BUTTON_MAX_DISPLAY_LEN: usize = 14;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = state.config.theme.clone();

    ui.add_space(theme.spacing_large);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("SOUNDBOARD")
                .size(10.0)
                .color(theme.color(theme.text_muted)),
        );
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("再スキャン")
                        .size(10.0)
                        .color(theme.color(theme.text_secondary)),
                )
                .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                .fill(theme.color(theme.chip_background)),
            )
            .clicked()
        {
            state.pending_soundboard_scan = true;
        }
    });

    ui.add_space(theme.spacing_medium);

    if state.soundboard_files.is_empty() {
        ui.label(
            egui::RichText::new("音声ファイルが見つかりません")
                .size(11.0)
                .color(theme.color(theme.text_muted)),
        );
        ui.label(
            egui::RichText::new(format!("フォルダ: {}", state.config.soundboard_path))
                .size(10.0)
                .color(theme.color(theme.text_muted)),
        );
    } else {
        show_sound_chips(ui, state, &theme);
    }

    if state.is_soundboard_playing {
        ui.add_space(theme.spacing_small);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("再生中...")
                    .size(10.0)
                    .color(theme.color(theme.accent)),
            );
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("停止")
                            .size(10.0)
                            .color(theme.color(theme.status_error)),
                    )
                    .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                    .fill(theme.color(theme.chip_background)),
                )
                .clicked()
            {
                state.pending_soundboard_stop = true;
            }
        });
    }
}

fn truncate_name(name: &str, max_len: usize) -> String {
    // Strip common extensions for display
    let stem = name
        .strip_suffix(".wav")
        .or_else(|| name.strip_suffix(".mp3"))
        .or_else(|| name.strip_suffix(".ogg"))
        .or_else(|| name.strip_suffix(".flac"))
        .unwrap_or(name);

    if stem.chars().count() <= max_len {
        stem.to_string()
    } else {
        let truncated: String = stem.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

fn show_sound_chips(ui: &mut egui::Ui, state: &mut AppState, theme: &Theme) {
    let chip_rounding = CornerRadius::same(theme.chip_rounding as u8);
    let files = state.soundboard_files.clone();
    let enabled = !state.is_soundboard_playing;

    ui.horizontal_wrapped(|ui| {
        for (name, path) in &files {
            let display_name = truncate_name(name, BUTTON_MAX_DISPLAY_LEN);

            let btn = ui.add_enabled(
                enabled,
                egui::Button::new(
                    egui::RichText::new(&display_name)
                        .color(theme.color(theme.text_secondary))
                        .size(11.0),
                )
                .corner_radius(chip_rounding)
                .fill(theme.color(theme.chip_background)),
            );

            // Show full name on hover if truncated
            let stem = name
                .strip_suffix(".wav")
                .or_else(|| name.strip_suffix(".mp3"))
                .or_else(|| name.strip_suffix(".ogg"))
                .or_else(|| name.strip_suffix(".flac"))
                .unwrap_or(name);
            if stem.chars().count() > BUTTON_MAX_DISPLAY_LEN {
                btn.clone().on_hover_text(stem);
            }

            if btn.clicked() {
                state.pending_soundboard_play = Some(path.clone());
            }
        }
    });
}
