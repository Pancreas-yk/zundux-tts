mod app;
mod audio;
mod config;
mod media;
mod osc;
mod tts;
mod ui;
mod validation;

use config::AppConfig;
use tts::voicevox::VoicevoxEngine;
use tts::TtsManager;

fn load_icon() -> Option<egui::IconData> {
    let png_bytes = include_bytes!("../assets/design-1.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

fn setup_japanese_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Try to load Noto Sans CJK JP from system
    let font_paths = [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk-fonts/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    ];

    for path in &font_paths {
        if let Ok(font_data) = std::fs::read(path) {
            fonts.font_data.insert(
                "noto_sans_cjk".to_owned(),
                egui::FontData::from_owned(font_data).into(),
            );
            // Add as highest priority for proportional text
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "noto_sans_cjk".to_owned());
            // Also add for monospace as fallback
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("noto_sans_cjk".to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);
}

fn setup_ime_env() {
    // Force X11 backend for IME support (Wayland IME support in winit/egui is incomplete)
    if std::env::var("WINIT_UNIX_BACKEND").is_err() {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

    // Auto-detect and set XMODIFIERS for IME if not already configured
    if std::env::var("XMODIFIERS").is_err() {
        let has_fcitx = std::process::Command::new("pgrep")
            .args(["-x", "fcitx5"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            || std::process::Command::new("pgrep")
                .args(["-x", "fcitx"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

        let has_ibus = std::process::Command::new("pgrep")
            .args(["-x", "ibus-daemon"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if has_fcitx {
            std::env::set_var("XMODIFIERS", "@im=fcitx");
        } else if has_ibus {
            std::env::set_var("XMODIFIERS", "@im=ibus");
        }
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    setup_ime_env();

    // Register cleanup handler for SIGTERM/SIGINT
    let cleanup_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag_clone = cleanup_flag.clone();
    ctrlc::set_handler(move || {
        flag_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        // Cleanup Docker container
        let _ = std::process::Command::new("docker")
            .args(["stop", "zundux-voicevox"])
            .output();
    })
    .expect("Failed to set SIGTERM handler");

    let config = AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config, using defaults: {}", e);
        AppConfig::default()
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    let engine = VoicevoxEngine::new(&config.voicevox_url);
    let tts_manager = TtsManager::new(Box::new(engine));

    let handle = rt.handle().clone();

    let icon = load_icon();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 700.0])
        .with_min_inner_size([400.0, 500.0])
        .with_transparent(true)
        .with_decorations(false);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "zundux_tts",
        options,
        Box::new(move |cc| {
            setup_japanese_fonts(&cc.egui_ctx);
            Ok(Box::new(app::ZunduxApp::new(config, tts_manager, handle)))
        }),
    )
}
