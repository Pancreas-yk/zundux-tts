use crate::app::AppState;
use crate::config::{AppConfig, TtsEngineType};
use crate::tts::voiceger::VOICEGER_LANGUAGES;
use crate::validation;

const PRESET_REF_WAV_BUTTONS_WIDTH: f32 = 120.0;

/// Render the preset list + editor for one engine group.
/// Call this from inside a `ui.collapsing(...)` closure.
fn show_preset_section(ui: &mut egui::Ui, state: &mut AppState, engine_group: &TtsEngineType) {
    let editing_this = (state.preset_adding || state.preset_edit_idx.is_some())
        && state
            .preset_edit_buf
            .as_ref()
            .map(|b| &b.engine == engine_group)
            .unwrap_or(false);

    let mut save_clicked = false;
    let mut cancel_clicked = false;

    if editing_this {
        let speakers_snapshot = state.speakers.clone();
        if let Some(buf) = state.preset_edit_buf.as_mut() {
            ui.horizontal(|ui| {
                ui.label("名前:");
                ui.add(
                    egui::TextEdit::singleline(&mut buf.name)
                        .desired_width(ui.available_width()),
                );
            });
            ui.add_space(4.0);

            let combo_id = match engine_group {
                TtsEngineType::Voicevox => "preset_edit_speaker_vox",
                TtsEngineType::Voiceger => "preset_edit_speaker_vgr",
            };
            let speaker_text = speakers_snapshot
                .iter()
                .flat_map(|s| s.styles.iter().map(move |st| (s, st)))
                .find(|(_, st)| st.id == buf.speaker_id)
                .map(|(s, st)| format!("{} - {}", s.name, st.name))
                .unwrap_or_else(|| format!("Speaker ID: {}", buf.speaker_id));

            ui.horizontal(|ui| {
                ui.label("スピーカー:");
                egui::ComboBox::from_id_salt(combo_id)
                    .selected_text(&speaker_text)
                    .show_ui(ui, |ui| {
                        for speaker in &speakers_snapshot {
                            for style in &speaker.styles {
                                let label = format!("{} - {}", speaker.name, style.name);
                                ui.selectable_value(&mut buf.speaker_id, style.id, &label);
                            }
                        }
                    });
            });
            ui.add_space(4.0);

            // Emotion selector (Voiceger only)
            if engine_group == &TtsEngineType::Voiceger {
                ui.horizontal(|ui| {
                    ui.label("感情:");
                    let emotion_label = if buf.voiceger_emotion.is_empty() {
                        "ノーマル"
                    } else {
                        buf.voiceger_emotion.as_str()
                    };
                    egui::ComboBox::from_id_salt("preset_edit_emotion_vgr")
                        .selected_text(emotion_label)
                        .show_ui(ui, |ui| {
                            for (name, _) in crate::tts::voiceger::VOICEGER_EMOTIONS {
                                ui.selectable_value(
                                    &mut buf.voiceger_emotion,
                                    name.to_string(),
                                    *name,
                                );
                            }
                        });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("参照WAV(任意):");
                    ui.add(
                        egui::TextEdit::singleline(&mut buf.voiceger_ref_audio_override)
                            .desired_width(ui.available_width() - PRESET_REF_WAV_BUTTONS_WIDTH)
                            .hint_text("空欄 = 感情/グローバル参照音声を使用"),
                    );
                    if ui.small_button("参照").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("WAV", &["wav"])
                            .pick_file()
                        {
                            buf.voiceger_ref_audio_override = path.to_string_lossy().to_string();
                        }
                    }
                    if ui.small_button("クリア").clicked() {
                        buf.voiceger_ref_audio_override.clear();
                    }
                });
                ui.label(
                    egui::RichText::new("※ 設定時は「参照WAV > 感情 > グローバル参照音声」の順で優先されます")
                        .small()
                        .weak(),
                );
                ui.add_space(4.0);
            }

            ui.horizontal(|ui| {
                ui.label("速度:");
                ui.add(egui::Slider::new(&mut buf.synth_params.speed_scale, 0.5..=2.0).step_by(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("ピッチ:");
                ui.add(egui::Slider::new(&mut buf.synth_params.pitch_scale, -0.15..=0.15).step_by(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("抑揚:");
                ui.add(egui::Slider::new(&mut buf.synth_params.intonation_scale, 0.0..=2.0).step_by(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("音量:");
                ui.add(egui::Slider::new(&mut buf.synth_params.volume_scale, 0.0..=2.0).step_by(0.05));
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                save_clicked = ui.button("保存").clicked();
                cancel_clicked = ui.button("キャンセル").clicked();
            });
        }
        ui.separator();
    }

    if save_clicked {
        if let Some(preset) = state.preset_edit_buf.take() {
            if !preset.name.trim().is_empty() {
                if state.preset_adding {
                    state.config.presets.push(preset);
                } else if let Some(idx) = state.preset_edit_idx {
                    if idx < state.config.presets.len() {
                        state.config.presets[idx] = preset;
                    }
                }
                let _ = state.config.save();
            }
        }
        state.preset_adding = false;
        state.preset_edit_idx = None;
    }
    if cancel_clicked {
        state.preset_adding = false;
        state.preset_edit_idx = None;
        state.preset_edit_buf = None;
    }

    // --- Preset list ---
    let mut to_edit: Option<usize> = None;
    let mut to_delete: Option<usize> = None;

    let group_indices: Vec<usize> = (0..state.config.presets.len())
        .filter(|&i| &state.config.presets[i].engine == engine_group)
        .collect();

    for i in group_indices {
        let preset = &state.config.presets[i];
        ui.horizontal(|ui| {
            let active = state.active_preset_idx == Some(i);
            if active {
                ui.label(egui::RichText::new("▶").color(
                    state.config.theme.color(state.config.theme.status_ok),
                ));
            } else {
                ui.label("　");
            }
            ui.label(&preset.name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("削除").clicked() {
                    to_delete = Some(i);
                }
                if ui.small_button("編集").clicked() {
                    to_edit = Some(i);
                }
            });
        });
    }

    if let Some(i) = to_edit {
        if state.preset_edit_idx != Some(i) {
            state.preset_edit_idx = Some(i);
            state.preset_adding = false;
            state.preset_edit_buf = Some(state.config.presets[i].clone());
        }
    }
    if let Some(i) = to_delete {
        state.config.presets.remove(i);
        if state.active_preset_idx == Some(i) {
            state.active_preset_idx = None;
        } else if let Some(idx) = state.active_preset_idx {
            if idx > i {
                state.active_preset_idx = Some(idx - 1);
            }
        }
        if state.preset_edit_idx == Some(i) {
            state.preset_edit_idx = None;
            state.preset_edit_buf = None;
        }
        let _ = state.config.save();
    }

    ui.add_space(4.0);
    if !editing_this && !state.preset_adding && state.preset_edit_idx.is_none() {
        ui.horizontal(|ui| {
            if ui.button("＋ 新規作成").clicked() {
                state.preset_adding = true;
                state.preset_edit_idx = None;
                let default_speaker_id = match engine_group {
                    TtsEngineType::Voicevox => state.config.speaker_id,
                    TtsEngineType::Voiceger => 0,
                };
                state.preset_edit_buf = Some(crate::config::SpeakerPreset {
                    name: String::new(),
                    speaker_id: default_speaker_id,
                    synth_params: state.config.synth_params.clone(),
                    engine: engine_group.clone(),
                    voiceger_emotion: String::new(),
                    voiceger_ref_audio_override: String::new(),
                });
            }
            if engine_group == &TtsEngineType::Voicevox {
                if ui.button("デフォルトに戻す").clicked() {
                    state.config.presets = crate::config::AppConfig::default_presets();
                    state.active_preset_idx = None;
                    let _ = state.config.save();
                }
            }
        });
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("設定");
        ui.separator();

        // ── General ──────────────────────────────────────────────────────────
        ui.collapsing("General", |ui| {

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

            // Engine auto-launch (independent per engine)
            ui.checkbox(
                &mut state.config.auto_launch_voicevox,
                "アプリ起動時にVOICEVOXを自動起動",
            );
            ui.checkbox(
                &mut state.config.auto_launch_voiceger,
                "アプリ起動時にVoicegerを自動起動",
            );
            ui.label(
                egui::RichText::new(
                    "  両方チェックすると同時起動します",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(8.0);

        // Engine selector
        ui.collapsing("TTSエンジン", |ui| {
            ui.horizontal(|ui| {
                let is_vox = state.config.active_engine == TtsEngineType::Voicevox;
                if ui.radio(is_vox, "VOICEVOX").clicked() && !is_vox {
                    state.config.active_engine = TtsEngineType::Voicevox;
                    let _ = state.config.save();
                }
                let is_vgr = state.config.active_engine == TtsEngineType::Voiceger;
                if ui.radio(is_vgr, "Voiceger").clicked() && !is_vgr {
                    state.config.active_engine = TtsEngineType::Voiceger;
                    let _ = state.config.save();
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let restart_label = match state.config.active_engine {
                    TtsEngineType::Voicevox => "VOICEVOXを再起動",
                    TtsEngineType::Voiceger => "Voicegerを再起動",
                };
                if ui.button(restart_label).clicked() {
                    match state.config.active_engine {
                        TtsEngineType::Voicevox => state.pending_restart_voicevox = true,
                        TtsEngineType::Voiceger => state.pending_restart_voiceger = true,
                    }
                }
                if state.voicevox_launching {
                    ui.label(
                        egui::RichText::new("起動中...")
                            .small()
                            .weak(),
                    );
                }
            });
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

        ui.collapsing("テンプレート", |ui| {
            ui.checkbox(
                &mut state.config.templates_default_expanded,
                "起動時にテンプレートを展開した状態にする",
            );
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            let mut to_remove = None;
            for (i, template) in state.config.templates.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(template);
                    if ui.button("削除").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { state.config.templates.remove(idx); }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut state.new_template_text)
                    .hint_text("新しいテンプレート...").desired_width(200.0));
                if ui.button("追加").clicked() && !state.new_template_text.trim().is_empty() {
                    state.config.templates.push(state.new_template_text.trim().to_string());
                    state.new_template_text.clear();
                }
            });
        });

        ui.add_space(8.0);

        ui.collapsing("外観", |ui| {
            use crate::ui::theme::Theme;
            show_color_with_opacity(ui, state, "ウィンドウ背景", "window_background", |t| &mut t.window_background);
            show_color_with_opacity(ui, state, "タイトルバー背景", "titlebar_background", |t| &mut t.titlebar_background);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("カラー設定（#RRGGBB または #RRGGBBAA）").small());
            ui.add_space(4.0);
            type ColorAccessor = fn(&mut crate::ui::theme::Theme) -> &mut [u8; 4];
            let color_fields: &[(&str, &str, ColorAccessor)] = &[
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
                ("サイズ変更ハンドル背景", "resize_handle_background", |t| &mut t.resize_handle_background),
            ];
            for &(label, key, accessor) in color_fields {
                let current = *accessor(&mut state.config.theme);
                let buf = state.color_edit_buffers.entry(key.to_string()).or_insert_with(|| Theme::to_hex(current));
                ui.horizontal(|ui| {
                    let preview_color = state.config.theme.color(current);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, preview_color);
                    ui.label(format!("{}:", label));
                    let response = ui.add(egui::TextEdit::singleline(buf).desired_width(100.0).hint_text("#RRGGBBAA"));
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

        }); // General

        ui.add_space(8.0);

        // ── VOICEVOX ─────────────────────────────────────────────────────────
        ui.collapsing("VOICEVOX", |ui| {

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
                        .hint_text("ローカルバイナリのパス（Dockerの場合は空でOK）"),
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

        // Speaker presets
        ui.collapsing("VOICEVOXプリセット", |ui| {
            show_preset_section(ui, state, &TtsEngineType::Voicevox);
        });

        ui.add_space(8.0);

        // VOICEVOX dictionary
        ui.collapsing("VOICEVOX辞書", |ui| {
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
                    ui.add(egui::TextEdit::singleline(&mut state.new_dict_surface).desired_width(100.0));
                    ui.label("読み:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.new_dict_pronunciation)
                            .desired_width(100.0),
                    );
                    if ui.button("追加").clicked()
                        && !state.new_dict_surface.trim().is_empty()
                        && !state.new_dict_pronunciation.trim().is_empty()
                    {
                        let surface = state.new_dict_surface.trim().to_string();
                        let pronunciation = state.new_dict_pronunciation.trim().to_string();
                        state.pending_add_dict_word = Some((surface, pronunciation));
                        state.new_dict_surface.clear();
                        state.new_dict_pronunciation.clear();
                    }
                });
            } else {
                ui.label("VOICEVOXに接続してください");
            }
        });

        }); // VOICEVOX

        ui.add_space(8.0);

        // ── Voiceger ─────────────────────────────────────────────────────────
        ui.collapsing("Voiceger", |ui| {

        // Voiceger connection settings
        ui.collapsing("Voiceger接続", |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut state.config.voiceger_url);
            });
            ui.horizontal(|ui| {
                ui.label("起動コマンド:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.config.voiceger_path)
                        .hint_text("空欄 = ~/voiceger_v2 のデフォルトコマンドを使用"),
                );
                if ui.button("参照").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.config.voiceger_path = path.to_string_lossy().to_string();
                        let _ = state.config.save();
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Voiceger起動").clicked() {
                    state.pending_launch_voiceger = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("参照音声:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.config.voiceger_ref_audio)
                        .hint_text("参照WAVのパス"),
                );
                if ui.button("参照").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("WAV", &["wav"])
                        .pick_file()
                    {
                        state.config.voiceger_ref_audio = path.to_string_lossy().to_string();
                        let _ = state.config.save();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("参照言語:");
                egui::ComboBox::from_id_salt("voiceger_prompt_lang")
                    .selected_text(&state.config.voiceger_prompt_lang)
                    .show_ui(ui, |ui| {
                        for (code, name, _) in VOICEGER_LANGUAGES {
                            ui.selectable_value(
                                &mut state.config.voiceger_prompt_lang,
                                code.to_string(),
                                *name,
                            );
                        }
                    });
            });
            ui.checkbox(
                &mut state.config.voiceger_ref_free,
                "参照なしモード (ref_free) を常時有効",
            );
            ui.label(
                egui::RichText::new(
                    "※ ON で参照音声/参照テキストを使わず合成します。短い英字入力（例: wa）や短い日本語（例: いいかな？）は、長文中の一節でもOFF時に自動で参照なしへ切り替わります。",
                )
                .small()
                .weak(),
            );
            ui.horizontal(|ui| {
                ui.label("参照テキスト:");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.config.voiceger_prompt_text)
                        .hint_text("参照音声が喋っている内容"),
                );
                if resp.lost_focus() {
                    let _ = state.config.save();
                }
            });
            ui.add_space(4.0);
            if ui.button("デフォルトに戻す").clicked() {
                state.config.reset_voiceger_defaults();
                let _ = state.config.save();
            }
            ui.label(
                egui::RichText::new(
                    "  参照音声は {voicegerのフォルダ}/reference/ 以下にあります",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(8.0);

        // Voiceger presets (with emotion selection)
        ui.collapsing("Voicegerプリセット", |ui| {
            show_preset_section(ui, state, &TtsEngineType::Voiceger);
        });

        ui.add_space(8.0);

        // Voiceger dictionary: per-language pronunciation replacements + shared silent words
        ui.collapsing("Voiceger辞書", |ui| {
            ui.label(
                egui::RichText::new("送信前にテキストを置換します（言語ごとに独立）")
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);

            // Language selector tabs
            ui.horizontal(|ui| {
                for (code, name, _) in crate::tts::voiceger::VOICEGER_LANGUAGES {
                    let selected = state.voiceger_dict_lang == *code;
                    if ui.selectable_label(selected, *name).clicked() {
                        state.voiceger_dict_lang = code.to_string();
                    }
                }
            });
            ui.separator();

            let lang = state.voiceger_dict_lang.clone();

            // Entries for selected language
            let mut to_delete_key: Option<String> = None;
            if let Some(lang_dict) = state.config.voiceger_dict.get(&lang) {
                let mut sorted: Vec<(String, String)> = lang_dict.iter().map(|(k,v)|(k.clone(),v.clone())).collect();
                sorted.sort_by(|a,b| a.0.cmp(&b.0));
                for (surface, reading) in &sorted {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} → {}", surface, reading));
                        if ui.small_button("削除").clicked() {
                            to_delete_key = Some(surface.clone());
                        }
                    });
                }
            }
            if let Some(key) = to_delete_key {
                if let Some(d) = state.config.voiceger_dict.get_mut(&lang) {
                    d.remove(&key);
                }
                let _ = state.config.save();
            }

            // Silent words (shared across engines)
            if !state.config.silent_words.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("無音化（両エンジン共通）")
                        .size(9.0)
                        .color(state.config.theme.color(state.config.theme.text_muted)),
                );
                let mut silent_to_delete = None;
                for (i, word) in state.config.silent_words.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} → (無音)", word));
                        if ui.small_button("削除").clicked() { silent_to_delete = Some(i); }
                    });
                }
                if let Some(idx) = silent_to_delete {
                    state.config.silent_words.remove(idx);
                    let _ = state.config.save();
                }
            }

            ui.add_space(4.0);
            ui.separator();

            // Add new entry
            ui.horizontal(|ui| {
                ui.label("表記:");
                ui.add(egui::TextEdit::singleline(&mut state.new_dict_surface).desired_width(90.0));
                ui.label("読み:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.new_dict_pronunciation)
                        .desired_width(90.0)
                        .hint_text("{silent}で無音化"),
                );
                if ui.button("追加").clicked()
                    && !state.new_dict_surface.trim().is_empty()
                    && !state.new_dict_pronunciation.trim().is_empty()
                {
                    let surface = state.new_dict_surface.trim().to_string();
                    let reading = state.new_dict_pronunciation.trim().to_string();
                    if reading == "{silent}" {
                        if !state.config.silent_words.contains(&surface) {
                            state.config.silent_words.push(surface);
                        }
                    } else {
                        state.config.voiceger_dict
                            .entry(lang.clone())
                            .or_default()
                            .insert(surface, reading);
                    }
                    let _ = state.config.save();
                    state.new_dict_surface.clear();
                    state.new_dict_pronunciation.clear();
                }
            });
        });

        }); // Voiceger

        ui.add_space(8.0);

        // ── Audio ─────────────────────────────────────────────────────────────
        ui.collapsing("Audio", |ui| {

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
                let current = state.config.mic_source_name.clone().unwrap_or_default();
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
                            let is_selected =
                                state.config.mic_source_name.as_deref() == Some(name.as_str());
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

            ui.add_space(4.0);
            let rnnoise_available =
                crate::audio::virtual_device::VirtualDevice::is_rnnoise_available();
            let old_ns = state.config.noise_suppression;
            ui.add_enabled(
                rnnoise_available,
                egui::Checkbox::new(
                    &mut state.config.noise_suppression,
                    "ノイズキャンセル (RNNoise)",
                ),
            );
            if !rnnoise_available {
                ui.label(
                    egui::RichText::new(
                        "noise-suppression-for-voice がインストールされていません",
                    )
                    .small()
                    .weak()
                    .color(egui::Color32::from_rgb(255, 160, 0)),
                );
            } else {
                ui.label(
                    egui::RichText::new("AIベースのノイズ除去をマイク入力に適用します")
                        .small()
                        .weak(),
                );
            }
            if state.config.noise_suppression != old_ns && state.mic_passthrough {
                state.pending_reconnect_mic = true;
            }
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
                egui::RichText::new(
                    "名前変更は「削除」→「作成」で反映されます（アプリの再起動は不要）",
                )
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
                if ui.button("参照").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.soundboard_path = path.to_string_lossy().to_string();
                        state.pending_soundboard_scan = true;
                    }
                }
                ui.add(egui::TextEdit::singleline(&mut state.config.soundboard_path)
                    .desired_width(f32::INFINITY));
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("ターゲット音量 (LUFS):");
                ui.add(egui::Slider::new(&mut state.config.target_lufs, -96.0..=-6.0).step_by(0.5));
            });

            ui.horizontal(|ui| {
                ui.label("許容範囲 (±LUFS):");
                ui.add(
                    egui::Slider::new(&mut state.config.loudness_tolerance, 1.0..=6.0).step_by(0.5),
                );
            });

            ui.add_space(4.0);
            if !state.config.soundboard_gains.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "調整済み: {}ファイル",
                            state.config.soundboard_gains.len()
                        ))
                        .size(10.0)
                        .color(state.config.theme.color(state.config.theme.text_muted)),
                    );
                    if ui.button("ゲインリセット").clicked() {
                        state.config.soundboard_gains.clear();
                        let _ = state.config.save();
                        state.pending_loudness_scan = true;
                    }
                });
            }
        });

        }); // Audio

        ui.add_space(12.0);

        // Warn when no presets exist
        let voicevox_presets = state.config.presets.iter().any(|p| p.engine == TtsEngineType::Voicevox);
        let voiceger_presets = state.config.presets.iter().any(|p| p.engine == TtsEngineType::Voiceger);
        if !voicevox_presets || !voiceger_presets {
            let warn_color = state.config.theme.color(state.config.theme.status_warn);
            if !voicevox_presets {
                ui.colored_label(warn_color, "⚠ VOICEVOXプリセットがありません。設定で追加してください。");
            }
            if !voiceger_presets {
                ui.colored_label(warn_color, "⚠ Voicegerプリセットがありません。設定で追加してください。");
            }
        }

        if ui.button("設定を保存").clicked() {
            match state.config.save() {
                Ok(()) => state.last_error = None,
                Err(e) => state.last_error = Some(format!("保存失敗: {}", e)),
            }
        }

        ui.add_space(24.0);
        ui.separator();
        ui.add_space(8.0);

        // Credits
        ui.collapsing("Credits", |ui| {
            let muted = state.config.theme.color(state.config.theme.text_muted);
            let secondary = state.config.theme.color(state.config.theme.text_secondary);

            ui.label(egui::RichText::new("ZunduxTTS").size(13.0).color(secondary));
            ui.label(
                egui::RichText::new("A Linux desktop TTS virtual microphone for VRChat.")
                    .small()
                    .color(muted),
            );
            ui.add_space(8.0);

            let credits = [
                ("VOICEVOX", "High-quality Japanese TTS engine", "https://voicevox.hiroshiba.jp/"),
                ("Voiceger / GPT-SoVITS", "Multilingual voice synthesis", "https://github.com/zunzun999/voiceger_v2"),
                ("egui / eframe", "Immediate-mode GUI framework", "https://github.com/emilk/egui"),
                ("ZUNDAMON character", "© AHS Co., Ltd.", "https://zunko.jp/"),
            ];

            for (name, desc, _url) in &credits {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(*name).small().color(secondary));
                    ui.label(egui::RichText::new(format!("— {}", desc)).small().color(muted));
                });
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Built with Rust  •  Licensed under MIT")
                    .small()
                    .color(muted),
            );
        });

        ui.add_space(8.0);
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
                egui::Slider::new(&mut opacity, 0.0..=100.0).suffix("%"),
            )
            .changed()
        {
            let alpha = (opacity / 100.0 * 255.0).round() as u8;
            accessor(&mut state.config.theme)[3] = alpha;
            state.needs_theme_update = true;
        }
    });
    if *accessor(&mut state.config.theme) == [255, 255, 255, current[3]]
        && current[0] == 255
        && current[1] == 255
        && current[2] == 255
    {
        ui.label(
            egui::RichText::new("  白(#FFFFFF)はpremultiplied alphaのため透明度を変更できません")
                .small()
                .weak(),
        );
    }
}
