use egui::{Color32, CornerRadius, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub window_background: [u8; 4],
    pub window_rounding: f32,
    pub titlebar_background: [u8; 4],
    pub titlebar_text: [u8; 4],
    pub panel_background: [u8; 4],
    pub text_primary: [u8; 4],
    pub text_secondary: [u8; 4],
    pub text_muted: [u8; 4],
    pub accent: [u8; 4],
    pub accent_hover: [u8; 4],
    pub status_ok: [u8; 4],
    pub status_warn: [u8; 4],
    pub status_error: [u8; 4],
    pub button_background: [u8; 4],
    pub button_rounding: f32,
    pub input_background: [u8; 4],
    pub input_rounding: f32,
    pub chip_background: [u8; 4],
    pub chip_rounding: f32,
    pub tab_active_background: [u8; 4],
    pub tab_rounding: f32,
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            window_background: [15, 15, 20, 200],
            window_rounding: 12.0,
            titlebar_background: [20, 20, 28, 240],
            titlebar_text: [180, 180, 180, 255],
            panel_background: [255, 255, 255, 15],
            text_primary: [224, 224, 224, 255],
            text_secondary: [160, 160, 160, 255],
            text_muted: [100, 100, 100, 255],
            accent: [120, 200, 120, 255],
            accent_hover: [140, 220, 140, 255],
            status_ok: [112, 192, 112, 255],
            status_warn: [200, 200, 100, 255],
            status_error: [200, 100, 100, 255],
            button_background: [255, 255, 255, 15],
            button_rounding: 6.0,
            input_background: [30, 35, 50, 180],
            input_rounding: 8.0,
            chip_background: [30, 35, 50, 180],
            chip_rounding: 16.0,
            tab_active_background: [8, 8, 12, 240],
            tab_rounding: 6.0,
            spacing_small: 4.0,
            spacing_medium: 8.0,
            spacing_large: 16.0,
        }
    }
}

impl Theme {
    pub fn color(&self, rgba: [u8; 4]) -> Color32 {
        Color32::from_rgba_premultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
    }

    pub fn validated(mut self) -> Self {
        let defaults = Self::default();
        let checks = [
            (
                &mut self.window_rounding,
                defaults.window_rounding,
                0.0,
                50.0,
            ),
            (
                &mut self.button_rounding,
                defaults.button_rounding,
                0.0,
                50.0,
            ),
            (&mut self.input_rounding, defaults.input_rounding, 0.0, 50.0),
            (&mut self.chip_rounding, defaults.chip_rounding, 0.0, 50.0),
            (&mut self.tab_rounding, defaults.tab_rounding, 0.0, 50.0),
            (&mut self.spacing_small, defaults.spacing_small, 0.0, 100.0),
            (
                &mut self.spacing_medium,
                defaults.spacing_medium,
                0.0,
                100.0,
            ),
            (&mut self.spacing_large, defaults.spacing_large, 0.0, 100.0),
        ];
        for (value, default, min, max) in checks {
            if !value.is_finite() || *value < min || *value > max {
                tracing::warn!(
                    "Theme value {} out of range, using default {}",
                    value,
                    default
                );
                *value = default;
            }
        }
        self
    }

    /// Parse a hex color string like "#RRGGBB" or "#RRGGBBAA" into [u8; 4].
    pub fn parse_hex(hex: &str) -> Option<[u8; 4]> {
        // Collect ASCII hex chars only, skipping '#' and any non-ASCII (e.g. full-width)
        let chars: Vec<u8> = hex
            .trim()
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .map(|c| c as u8)
            .collect();
        let s = std::str::from_utf8(&chars).ok()?;
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some([r, g, b, 255])
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some([r, g, b, a])
            }
            _ => None,
        }
    }

    /// Format [u8; 4] as "#RRGGBBAA".
    pub fn to_hex(rgba: [u8; 4]) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2], rgba[3])
    }

    pub fn to_visuals(&self) -> Visuals {
        let mut visuals = Visuals::dark();
        // Set default text color via widget strokes, not override_text_color
        // (override_text_color ignores per-widget RichText::color calls)
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.color(self.text_primary));
        visuals.panel_fill = self.color(self.panel_background);
        visuals.window_fill = self.color(self.window_background);
        visuals.window_corner_radius = CornerRadius::same(self.window_rounding as u8);
        visuals.widgets.inactive.bg_fill = self.color(self.button_background);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(self.button_rounding as u8);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.color(self.text_secondary));
        visuals.widgets.hovered.bg_fill = self.color(self.accent_hover);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(self.button_rounding as u8);
        visuals.widgets.active.bg_fill = self.color(self.accent);
        visuals.widgets.active.corner_radius = CornerRadius::same(self.button_rounding as u8);
        visuals.selection.bg_fill = self.color(self.accent);
        visuals.extreme_bg_color = Color32::TRANSPARENT;
        visuals
    }

    pub fn to_style(&self) -> Style {
        let mut style = Style::default();
        style.spacing.item_spacing = egui::vec2(self.spacing_medium, self.spacing_medium);
        style.spacing.button_padding = egui::vec2(self.spacing_medium, self.spacing_small);
        style
    }
}
