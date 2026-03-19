use crate::app::AppState;
use crate::config::AppConfig;
use crate::validation;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("設定");
        ui.separator();

        // Startup settings (app + VOICEVOX combined)
        ui.collapsing("起動設定", |ui| {
            // App autostart
            let mut autostart_app = AppConfig::is_autostart_enabled();
            if ui
                .checkbox(&mut autostart_app, "システム起動時にアプリを自動起動")
                .changed()
            {
                state.config.auto_start_app = autostart_app;
                if let Err(e) = AppConfig::set_autostart(autostart_app) {
                    state.last_error = Some(format!("自動起動設定失敗: {}", e));
                }
            }
            ui.label(
                egui::RichText::new("  ~/.config/autostart/ にデスクトップエントリを配置します")
                    .small()
                    .weak(),
            );

            ui.add_space(4.0);

            // VOICEVOX auto-launch
            ui.checkbox(
                &mut state.config.auto_launch_voicevox,
                "アプリ起動時にVOICEVOXを自動起動",
            );
            ui.label(
                egui::RichText::new(
                    "  両方有効にすると、PC起動→アプリ→VOICEVOX が全自動になります",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(8.0);

        // VOICEVOX connection
        ui.collapsing("VOICEVOX接続", |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut state.config.voicevox_url);
            });
            if validation::is_valid_voicevox_url(&state.config.voicevox_url).is_err() {
                ui.colored_label(
                    state.config.theme.color(state.config.theme.status_warn),
                    "URLはhttp://localhost または http://127.0.0.1 のみ",
                );
            }
            ui.horizontal(|ui| {
                ui.label("実行パス:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.config.voicevox_path)
                        .hint_text("/path/to/VOICEVOX または docker run ..."),
                );
                if ui.button("参照").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.config.voicevox_path = path.to_string_lossy().to_string();
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("接続テスト").clicked() {
                    state.pending_health_check = true;
                }
                if ui.button("VOICEVOX起動").clicked() {
                    state.pending_launch_voicevox = true;
                }
                if state.voicevox_connected {
                    ui.colored_label(
                        state.config.theme.color(state.config.theme.status_ok),
                        "接続OK",
                    );
                } else if state.voicevox_launching {
                    ui.colored_label(
                        state.config.theme.color(state.config.theme.status_warn),
                        "起動中...",
                    );
                } else {
                    ui.colored_label(
                        state.config.theme.color(state.config.theme.status_error),
                        "未接続",
                    );
                }
            });
        });

        ui.add_space(8.0);

        // Voice parameters
        ui.collapsing("音声パラメータ", |ui| {
            let params = &mut state.config.synth_params;
            ui.horizontal(|ui| {
                ui.label("速度:");
                ui.add(egui::Slider::new(&mut params.speed_scale, 0.5..=2.0).step_by(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("ピッチ:");
                ui.add(egui::Slider::new(&mut params.pitch_scale, -0.15..=0.15).step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("抑揚:");
                ui.add(egui::Slider::new(&mut params.intonation_scale, 0.0..=2.0).step_by(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("音量:");
                ui.add(egui::Slider::new(&mut params.volume_scale, 0.0..=2.0).step_by(0.05));
            });
            if ui.button("デフォルトに戻す").clicked() {
                *params = crate::config::SynthParamsConfig::default();
            }
        });

        ui.add_space(8.0);

        // Speaker selection
        ui.collapsing("スピーカー選択", |ui| {
            for speaker in &state.speakers {
                for style in &speaker.styles {
                    let label = format!("{} ({})", speaker.name, style.name);
                    ui.radio_value(&mut state.config.speaker_id, style.id, &label);
                }
            }
            if state.speakers.is_empty() {
                ui.label("VOICEVOXに接続してスピーカー一覧を取得してください");
            }
        });

        ui.add_space(8.0);

        ui.collapsing("ユーザー辞書", |ui| {
            if state.voicevox_connected {
                if ui.button("辞書を読み込む").clicked() {
                    state.pending_load_user_dict = true;
                }
                ui.add_space(4.0);

                let mut to_delete = None;
                for (uuid, surface, pronunciation) in &state.user_dict {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} → {}", surface, pronunciation));
                        if ui.small_button("削除").clicked() {
                            to_delete = Some(uuid.clone());
                        }
                    });
                }
                if let Some(uuid) = to_delete {
                    state.pending_delete_dict_word = Some(uuid);
                }

                ui.add_space(4.0);
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("表記:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_dict_surface)
                            .desired_width(100.0),
                    );
                    ui.label("読み:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_dict_pronunciation)
                            .desired_width(100.0),
                    );
                    if ui.button("追加").clicked()
                        && !state.new_dict_surface.trim().is_empty()
                        && !state.new_dict_pronunciation.trim().is_empty()
                    {
                        state.pending_add_dict_word = Some((
                            state.new_dict_surface.trim().to_string(),
                            state.new_dict_pronunciation.trim().to_string(),
                        ));
                        state.new_dict_surface.clear();
                        state.new_dict_pronunciation.clear();
                    }
                });
            } else {
                ui.label("VOICEVOXに接続してください");
            }
        });

        ui.add_space(8.0);

        // Audio monitoring
        ui.collapsing("オーディオ", |ui| {
            ui.checkbox(
                &mut state.config.monitor_audio,
                "自分にも音声を再生（モニター）",
            );
            ui.label("有効にすると、仮想マイクに加えてスピーカーからも音声が聞こえます");
        });

        ui.add_space(8.0);

        ui.collapsing("OSC設定", |ui| {
            ui.checkbox(&mut state.config.osc_enabled, "OSCチャットボックスを有効化");
            ui.label("VRChatのチャットボックスにテキストを表示します");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("送信先アドレス:");
                ui.text_edit_singleline(&mut state.config.osc_address);
            });
            ui.horizontal(|ui| {
                ui.label("ポート:");
                let mut port_str = state.config.osc_port.to_string();
                if ui.text_edit_singleline(&mut port_str).changed() {
                    if let Ok(p) = port_str.parse::<u16>() {
                        state.config.osc_port = p;
                    }
                }
            });
        });

        ui.add_space(8.0);

        ui.collapsing("音声エフェクト", |ui| {
            ui.checkbox(&mut state.config.echo_enabled, "エコーを有効化");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("遅延(ms):");
                ui.add(egui::Slider::new(&mut state.config.echo_delay_ms, 50..=500).step_by(10.0));
            });
            ui.horizontal(|ui| {
                ui.label("減衰:");
                ui.add(egui::Slider::new(&mut state.config.echo_decay, 0.1..=0.8).step_by(0.05));
            });
        });

        ui.add_space(8.0);

        // Virtual device
        ui.collapsing("仮想デバイス", |ui| {
            ui.horizontal(|ui| {
                ui.label("デバイス名:");
                ui.text_edit_singleline(&mut state.config.virtual_device_name);
            });
            if !validation::is_valid_device_name(&state.config.virtual_device_name) {
                ui.colored_label(
                    state.config.theme.color(state.config.theme.status_warn),
                    "デバイス名は英数字、_、- のみ (最大64文字)",
                );
            }
            ui.horizontal(|ui| {
                if ui.button("作成").clicked() {
                    state.pending_create_device = true;
                }
                if ui.button("削除").clicked() {
                    state.pending_destroy_device = true;
                }
            });
            if state.device_ready {
                ui.colored_label(
                    state.config.theme.color(state.config.theme.status_ok),
                    format!("マイクソース: {}.monitor", state.config.virtual_device_name),
                );
            }
        });

        ui.add_space(8.0);

        ui.collapsing("サウンドボード", |ui| {
            ui.horizontal(|ui| {
                ui.label("フォルダパス:");
                ui.text_edit_singleline(&mut state.config.soundboard_path);
                if ui.button("参照").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.soundboard_path = path.to_string_lossy().to_string();
                        state.pending_soundboard_scan = true;
                    }
                }
            });
        });

        ui.add_space(8.0);

        // Templates
        ui.collapsing("テンプレート", |ui| {
            let mut to_remove = None;
            for (i, template) in state.config.templates.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(template);
                    if ui.button("削除").clicked() {
                        to_remove = Some(i);
                    }
                });
            }
            if let Some(idx) = to_remove {
                state.config.templates.remove(idx);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let te = egui::TextEdit::singleline(&mut state.new_template_text)
                    .hint_text("新しいテンプレート...")
                    .desired_width(200.0);
                ui.add(te);
                if ui.button("追加").clicked() && !state.new_template_text.trim().is_empty() {
                    state
                        .config
                        .templates
                        .push(state.new_template_text.trim().to_string());
                    state.new_template_text.clear();
                }
            });
        });

        ui.add_space(12.0);

        if ui.button("設定を保存").clicked() {
            match state.config.save() {
                Ok(()) => state.last_error = None,
                Err(e) => state.last_error = Some(format!("保存失敗: {}", e)),
            }
        }
    });
}
