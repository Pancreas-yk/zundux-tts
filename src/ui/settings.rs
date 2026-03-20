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
                ui.horizontal(|ui| {
                    if ui.button("辞書を読み込む").clicked() {
                        state.pending_load_user_dict = true;
                    }
                    ui.label(
                        egui::RichText::new("(読み込んでいただくことで既存の割り当てを見られます)")
                            .small()
                            .weak(),
                    );
                });
                ui.add_space(4.0);

                // VOICEVOX dictionary entries
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

                // Silent word entries (local)
                let mut silent_to_delete = None;
                for (i, word) in state.config.silent_words.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} → {{silent}}",
                            word
                        ));
                        if ui.small_button("削除").clicked() {
                            silent_to_delete = Some(i);
                        }
                    });
                }
                if let Some(idx) = silent_to_delete {
                    state.config.silent_words.remove(idx);
                    let _ = state.config.save();
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
                            .desired_width(100.0)
                            .hint_text("{silent}で無音化"),
                    );
                    if ui.button("追加").clicked()
                        && !state.new_dict_surface.trim().is_empty()
                        && !state.new_dict_pronunciation.trim().is_empty()
                    {
                        let surface = state.new_dict_surface.trim().to_string();
                        let pronunciation = state.new_dict_pronunciation.trim().to_string();
                        if pronunciation == "{silent}" {
                            // Store locally as a silent word
                            if !state.config.silent_words.contains(&surface) {
                                state.config.silent_words.push(surface);
                                let _ = state.config.save();
                            }
                        } else {
                            state.pending_add_dict_word = Some((surface, pronunciation));
                        }
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

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("マイク入力:");
                if ui.button("更新").clicked() {
                    state.pending_refresh_mic_sources = true;
                }
            });

            if state.available_mic_sources.is_empty() {
                ui.label(
                    egui::RichText::new("マイクが見つかりません（更新を押してください）")
                        .small()
                        .weak(),
                );
            } else {
                let current = state
                    .config
                    .mic_source_name
                    .clone()
                    .unwrap_or_default();
                let current_desc = state
                    .available_mic_sources
                    .iter()
                    .find(|(name, _)| *name == current)
                    .map(|(_, desc)| desc.as_str())
                    .unwrap_or("未選択");

                egui::ComboBox::from_id_salt("mic_source_combo")
                    .selected_text(current_desc)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (name, desc) in &state.available_mic_sources {
                            let is_selected = state
                                .config
                                .mic_source_name
                                .as_deref()
                                == Some(name.as_str());
                            if ui.selectable_label(is_selected, desc).clicked() {
                                state.config.mic_source_name = Some(name.clone());
                                // Reconnect loopback with new source if mic is on
                                if state.mic_passthrough {
                                    state.pending_reconnect_mic = true;
                                }
                            }
                        }
                    });
            }
            ui.label(
                egui::RichText::new("MIC: ONで使用するマイクデバイスを選択します")
                    .small()
                    .weak(),
            );
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
            ui.label(
                egui::RichText::new("名前変更は「削除」→「作成」で反映されます（アプリの再起動は不要）")
                    .small()
                    .weak(),
            );
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

        ui.add_space(8.0);

        // Appearance
        ui.collapsing("外観", |ui| {
            use crate::ui::theme::Theme;

            // Window background: color + transparency side by side
            show_color_with_opacity(
                ui,
                state,
                "ウィンドウ背景",
                "window_background",
                |t| &mut t.window_background,
            );
            // Titlebar background: color + transparency side by side
            show_color_with_opacity(
                ui,
                state,
                "タイトルバー背景",
                "titlebar_background",
                |t| &mut t.titlebar_background,
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("カラー設定（#RRGGBB または #RRGGBBAA）").small());
            ui.add_space(4.0);

            let color_fields: &[(&str, &str, fn(&mut crate::ui::theme::Theme) -> &mut [u8; 4])] = &[
                ("タイトルバー文字", "titlebar_text", |t| &mut t.titlebar_text),
                ("パネル背景", "panel_background", |t| &mut t.panel_background),
                ("メイン文字", "text_primary", |t| &mut t.text_primary),
                ("サブ文字", "text_secondary", |t| &mut t.text_secondary),
                ("控えめ文字", "text_muted", |t| &mut t.text_muted),
                ("アクセント", "accent", |t| &mut t.accent),
                ("アクセント(ホバー)", "accent_hover", |t| &mut t.accent_hover),
                ("ボタン背景", "button_background", |t| &mut t.button_background),
                ("入力欄背景", "input_background", |t| &mut t.input_background),
                ("チップ背景", "chip_background", |t| &mut t.chip_background),
                ("タブ背景(選択)", "tab_active_background", |t| &mut t.tab_active_background),
            ];

            for &(label, key, accessor) in color_fields {
                let current = accessor(&mut state.config.theme).clone();
                let buf = state
                    .color_edit_buffers
                    .entry(key.to_string())
                    .or_insert_with(|| Theme::to_hex(current));

                ui.horizontal(|ui| {
                    // Color preview swatch
                    let preview_color = state.config.theme.color(current);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(16.0, 16.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 2.0, preview_color);

                    ui.label(format!("{}:", label));
                    let response = ui.add(
                        egui::TextEdit::singleline(buf)
                            .desired_width(100.0)
                            .hint_text("#RRGGBBAA"),
                    );
                    if response.changed() {
                        if let Some(parsed) = Theme::parse_hex(buf) {
                            *accessor(&mut state.config.theme) = parsed;
                            state.needs_theme_update = true;
                        }
                    }
                });
            }

            ui.add_space(4.0);
            if ui.button("デフォルトに戻す").clicked() {
                state.config.theme = Theme::default();
                state.color_edit_buffers.clear();
                state.needs_theme_update = true;
            }
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

/// Color hex input (RGB only) + transparency slider for a single RGBA field.
fn show_color_with_opacity(
    ui: &mut egui::Ui,
    state: &mut AppState,
    label: &str,
    key: &str,
    accessor: fn(&mut crate::ui::theme::Theme) -> &mut [u8; 4],
) {
    use crate::ui::theme::Theme;

    let current = *accessor(&mut state.config.theme);
    let rgb_hex = format!("#{:02X}{:02X}{:02X}", current[0], current[1], current[2]);
    let buf = state
        .color_edit_buffers
        .entry(key.to_string())
        .or_insert_with(|| rgb_hex.clone());

    ui.horizontal(|ui| {
        // Color preview swatch
        let preview_color = state.config.theme.color(current);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, preview_color);

        ui.label(format!("{}:", label));
        let response = ui.add(
            egui::TextEdit::singleline(buf)
                .desired_width(80.0)
                .hint_text("#RRGGBB"),
        );
        if response.changed() {
            if let Some(parsed) = Theme::parse_hex(buf) {
                let field = accessor(&mut state.config.theme);
                // Only update RGB, preserve current alpha
                field[0] = parsed[0];
                field[1] = parsed[1];
                field[2] = parsed[2];
                state.needs_theme_update = true;
            }
        }

        // White (#FFFFFF) uses premultiplied alpha — lowering alpha has no visible effect
        let rgb = accessor(&mut state.config.theme);
        let is_white = rgb[0] == 255 && rgb[1] == 255 && rgb[2] == 255;

        let mut opacity = accessor(&mut state.config.theme)[3] as f32 / 255.0 * 100.0;
        if ui
            .add_enabled(
                !is_white,
                egui::Slider::new(&mut opacity, 0.0..=100.0)
                    .suffix("%"),
            )
            .changed()
        {
            let alpha = (opacity / 100.0 * 255.0).round() as u8;
            accessor(&mut state.config.theme)[3] = alpha;
            state.needs_theme_update = true;
        }
    });
    if *accessor(&mut state.config.theme) == [255, 255, 255, current[3]] && current[0] == 255 && current[1] == 255 && current[2] == 255 {
        ui.label(
            egui::RichText::new("  白(#FFFFFF)はpremultiplied alphaのため透明度を変更できません")
                .small()
                .weak(),
        );
    }
}
