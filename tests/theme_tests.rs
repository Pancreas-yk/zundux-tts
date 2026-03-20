use zundux_tts::ui::theme::Theme;

#[test]
fn default_theme_is_valid() {
    let theme = Theme::default();
    let validated = theme.validated();
    // validated() should not change defaults
    assert_eq!(validated.window_rounding, 12.0);
}

#[test]
fn theme_from_partial_toml() {
    let toml_str = r#"
[theme]
window_rounding = 8.0
"#;
    let config: toml::Value = toml::from_str(toml_str).unwrap();
    let theme: Theme = config
        .get("theme")
        .map(|v| v.clone().try_into().unwrap())
        .unwrap_or_default();
    assert_eq!(theme.window_rounding, 8.0);
    assert_eq!(theme.spacing_small, 4.0);
}

#[test]
fn theme_rejects_invalid_rounding() {
    let mut theme = Theme::default();
    theme.window_rounding = f32::NAN;
    let validated = theme.validated();
    // NaN should be replaced with default
    assert_eq!(validated.window_rounding, Theme::default().window_rounding);
}

#[test]
fn theme_to_visuals_does_not_panic() {
    let theme = Theme::default();
    let _visuals = theme.to_visuals();
    let _style = theme.to_style();
}
