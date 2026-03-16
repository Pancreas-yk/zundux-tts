use crate::app::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("メディア");
    ui.separator();

    if !state.media_has_ytdlp || !state.media_has_ffmpeg {
        ui.colored_label(egui::Color32::from_rgb(255, 200, 100), "必要なツール:");
        if !state.media_has_ytdlp {
            ui.label("  yt-dlp が見つかりません。インストールしてください。");
        }
        if !state.media_has_ffmpeg {
            ui.label("  ffmpeg が見つかりません。インストールしてください。");
        }
        ui.add_space(8.0);
    }

    ui.label("URL再生");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.media_url)
                .hint_text("URLを入力...")
                .desired_width(300.0),
        );
    });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let can_play = !state.media_url.trim().is_empty()
            && !state.media_playing
            && state.media_has_ytdlp
            && state.media_has_ffmpeg;
        if ui.add_enabled(can_play, egui::Button::new("再生")).clicked() {
            state.pending_media_play = Some(state.media_url.trim().to_string());
        }
        if ui.add_enabled(state.media_playing, egui::Button::new("停止")).clicked() {
            state.pending_media_stop = true;
        }
    });

    if state.media_playing {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("メディア再生中...");
        });
    }

    ui.add_space(16.0);
    ui.separator();

    // Desktop capture section - placeholder for now, will be filled in Task 10
    ui.label("デスクトップ音声キャプチャ");
    ui.add_space(4.0);
    ui.label("準備中...");
}
