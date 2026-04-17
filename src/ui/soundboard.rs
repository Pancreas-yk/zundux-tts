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
        if !state.soundboard_loudness.is_empty()
            && ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("全て自動調整")
                            .size(10.0)
                            .color(theme.color(theme.text_secondary)),
                    )
                    .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                    .fill(theme.color(theme.chip_background)),
                )
                .clicked()
        {
            state.pending_normalize_all = true;
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
        if !state.soundboard_loudness.is_empty() {
            ui.label(
                egui::RichText::new(format!(
                    "ターゲット: {} LUFS | クリックで再生",
                    state.config.target_lufs
                ))
                .size(9.0)
                .color(theme.color(theme.text_muted)),
            );
        }
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

    // -- Desktop audio capture --
    ui.add_space(theme.spacing_large);
    ui.separator();
    ui.add_space(theme.spacing_small);

    ui.label(
        egui::RichText::new("DESKTOP CAPTURE")
            .size(10.0)
            .color(theme.color(theme.text_muted)),
    );

    ui.add_space(theme.spacing_small);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("アプリ一覧を更新")
                        .size(10.0)
                        .color(theme.color(theme.text_secondary)),
                )
                .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                .fill(theme.color(theme.chip_background)),
            )
            .clicked()
        {
            state.pending_refresh_sink_inputs = true;
        }
        if state.is_capturing {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("キャプチャ停止")
                            .size(10.0)
                            .color(theme.color(theme.status_error)),
                    )
                    .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                    .fill(theme.color(theme.chip_background)),
                )
                .clicked()
            {
                state.pending_stop_capture = true;
            }
            ui.label(
                egui::RichText::new("キャプチャ中")
                    .size(10.0)
                    .color(theme.color(theme.status_ok)),
            );
        }
    });

    ui.add_space(theme.spacing_small);

    if state.sink_inputs.is_empty() {
        ui.label(
            egui::RichText::new("「アプリ一覧を更新」を押してください")
                .size(11.0)
                .color(theme.color(theme.text_muted)),
        );
    } else {
        for input in state.sink_inputs.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&input.name)
                        .size(11.0)
                        .color(theme.color(theme.text_secondary)),
                );
                let can_capture = !state.is_capturing && state.device_ready;
                if ui
                    .add_enabled(
                        can_capture,
                        egui::Button::new(
                            egui::RichText::new("キャプチャ")
                                .size(10.0)
                                .color(theme.color(theme.text_secondary)),
                        )
                        .corner_radius(CornerRadius::same(theme.chip_rounding as u8))
                        .fill(theme.color(theme.chip_background)),
                    )
                    .clicked()
                {
                    state.pending_start_capture = Some((input.id, input.sink.clone()));
                }
            });
        }
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

            let loudness = state.soundboard_loudness.get(path);

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

            if let Some(info) = loudness {
                let gain = crate::config::AppConfig::soundboard_gain_key(path)
                    .and_then(|k| state.config.soundboard_gains.get(&k).copied())
                    .unwrap_or(0.0);
                let effective_lufs = info.lufs + gain;

                let color = loudness_color(
                    effective_lufs,
                    state.config.target_lufs,
                    state.config.loudness_tolerance,
                    theme,
                );
                ui.label(
                    egui::RichText::new(format!("{:.0}", effective_lufs))
                        .size(9.0)
                        .color(color),
                );
            }

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

fn loudness_color(lufs: f64, target: f64, tolerance: f64, theme: &Theme) -> egui::Color32 {
    if lufs > target + tolerance {
        theme.color(theme.status_error)
    } else if lufs < target - tolerance {
        theme.color(theme.text_muted)
    } else {
        theme.color(theme.status_ok)
    }
}
