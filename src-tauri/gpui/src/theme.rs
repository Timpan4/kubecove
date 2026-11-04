//! Dark-theme tokens ported from `src/App.css` and the Tailwind palette the
//! Svelte frontend uses. Colors stay in OKLCH here so each value can be checked
//! against the stylesheet it mirrors.

use gpui_kit::component::highlighter::ThemeStyle;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Rgba, px};

/// Converts CSS `oklch(l c h / alpha)` to GPUI color.
pub fn oklch(l: f32, c: f32, h: f32, alpha: f32) -> Hsla {
    let (sin, cos) = h.to_radians().sin_cos();
    let (a, b) = (c * cos, c * sin);
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let gamma = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Rgba {
        r: gamma(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        g: gamma(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_4 * s_),
        b: gamma(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
        a: alpha,
    }
    .into()
}

/// GPUI color to CSS `oklch(l c h / alpha)` components.
fn to_oklch(color: Hsla) -> [f32; 4] {
    let rgb = color.to_rgb();
    let linear = |x: f32| {
        if x <= 0.040_45 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(rgb.r), linear(rgb.g), linear(rgb.b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    [
        lightness,
        a.hypot(b),
        b.atan2(a).to_degrees().rem_euclid(360.),
        rgb.a,
    ]
}

/// CSS `color-mix(in oklch, a weight, b)`: premultiplied alpha, shorter hue
/// arc, and an achromatic side takes the other side's hue.
pub fn mix(a: Hsla, b: Hsla, weight: f32) -> Hsla {
    let ([la, ca, ha, aa], [lb, cb, hb, ab]) = (to_oklch(a), to_oklch(b));
    let alpha = weight * aa + (1. - weight) * ab;
    if alpha <= 0. {
        return oklch(0., 0., 0., 0.);
    }
    let premultiplied = |x: f32, y: f32| (weight * aa * x + (1. - weight) * ab * y) / alpha;
    const ACHROMATIC: f32 = 1e-4;
    let hue = match (ca < ACHROMATIC || aa == 0., cb < ACHROMATIC || ab == 0.) {
        (true, false) => hb,
        (false, true) | (true, true) => ha,
        (false, false) => {
            let delta = (hb - ha + 540.).rem_euclid(360.) - 180.;
            ha + delta * (1. - weight)
        }
    };
    oklch(
        premultiplied(la, lb),
        premultiplied(ca, cb),
        hue.rem_euclid(360.),
        alpha,
    )
}

fn white(alpha: f32) -> Hsla {
    oklch(1.0, 0.0, 0.0, alpha)
}

/// `.dark` tokens from App.css.
pub struct Tokens;

#[allow(dead_code)] // The full token set mirrors App.css; views adopt tokens as screens land.
impl Tokens {
    pub fn background() -> Hsla {
        oklch(0.147, 0.004, 49.25, 1.0)
    }
    pub fn foreground() -> Hsla {
        oklch(0.985, 0.001, 106.423, 1.0)
    }
    pub fn card() -> Hsla {
        oklch(0.216, 0.006, 56.043, 1.0)
    }
    /// `color-mix(in oklch, card 88%, white 12%)`.
    pub fn surface2() -> Hsla {
        oklch(0.310_08, 0.005_28, 56.043, 1.0)
    }
    /// `color-mix(in oklch, card 76%, white 24%)`.
    pub fn surface3() -> Hsla {
        oklch(0.404_16, 0.004_56, 56.043, 1.0)
    }
    pub fn primary() -> Hsla {
        oklch(0.443, 0.11, 240.79, 1.0)
    }
    pub fn primary_foreground() -> Hsla {
        oklch(0.977, 0.013, 236.62, 1.0)
    }
    pub fn secondary() -> Hsla {
        oklch(0.274, 0.006, 286.033, 1.0)
    }
    pub fn muted() -> Hsla {
        oklch(0.268, 0.007, 34.298, 1.0)
    }
    pub fn muted_foreground() -> Hsla {
        oklch(0.709, 0.01, 56.259, 1.0)
    }
    pub fn destructive() -> Hsla {
        oklch(0.704, 0.191, 22.216, 1.0)
    }
    pub fn border() -> Hsla {
        white(0.10)
    }
    pub fn input() -> Hsla {
        white(0.15)
    }
    pub fn ring() -> Hsla {
        oklch(0.553, 0.013, 58.071, 1.0)
    }
    pub fn sidebar_primary() -> Hsla {
        oklch(0.685, 0.169, 237.323, 1.0)
    }
    pub fn workload() -> Hsla {
        oklch(0.72, 0.13, 236.0, 1.0)
    }
    pub fn network() -> Hsla {
        oklch(0.78, 0.12, 190.0, 1.0)
    }
    pub fn config() -> Hsla {
        oklch(0.74, 0.12, 300.0, 1.0)
    }
    pub fn storage() -> Hsla {
        oklch(0.78, 0.13, 78.0, 1.0)
    }
    pub fn secret() -> Hsla {
        oklch(0.72, 0.17, 20.0, 1.0)
    }
    pub fn cluster() -> Hsla {
        oklch(0.74, 0.14, 155.0, 1.0)
    }
    pub fn replicaset() -> Hsla {
        oklch(0.74, 0.12, 292.0, 1.0)
    }
    pub fn pod() -> Hsla {
        oklch(0.78, 0.12, 200.0, 1.0)
    }
}

/// Tailwind v4 palette entries used by status styles.
pub struct Palette;

#[allow(dead_code)]
impl Palette {
    pub fn emerald300() -> Hsla {
        oklch(0.845, 0.143, 164.978, 1.0)
    }
    pub fn emerald500(alpha: f32) -> Hsla {
        oklch(0.696, 0.17, 162.48, alpha)
    }
    pub fn amber300() -> Hsla {
        oklch(0.879, 0.169, 91.605, 1.0)
    }
    pub fn amber400() -> Hsla {
        oklch(0.828, 0.189, 84.429, 1.0)
    }
    pub fn sky400() -> Hsla {
        oklch(0.746, 0.16, 232.661, 1.0)
    }
    pub fn amber500(alpha: f32) -> Hsla {
        oklch(0.769, 0.188, 70.08, alpha)
    }
    pub fn sky300() -> Hsla {
        oklch(0.828, 0.111, 230.318, 1.0)
    }
    pub fn sky500(alpha: f32) -> Hsla {
        oklch(0.685, 0.169, 237.323, alpha)
    }
    pub fn red300() -> Hsla {
        oklch(0.808, 0.114, 19.571, 1.0)
    }
}

pub const SANS: &str = "Inter";
pub const HEADING: &str = "Geist";
pub const MONO: &str = "Geist Mono";

/// Applies the web frontend's dark theme to GPUI Kit components.
pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = SANS.into();
    // Also the window rem: 16px like the browser default, so component sizes
    // ported from Tailwind (`h_8`, `text_sm`) match the web.
    theme.font_size = px(16.);
    theme.mono_font_family = MONO.into();
    theme.mono_font_size = px(12.);
    theme.radius = px(8.);
    theme.radius_lg = px(10.);
    let colors = &mut theme.colors;
    colors.background = Tokens::background();
    colors.foreground = Tokens::foreground();
    colors.border = Tokens::border();
    colors.input = Tokens::input();
    colors.ring = Tokens::ring();
    colors.muted = Tokens::muted();
    colors.muted_foreground = Tokens::muted_foreground();
    colors.accent = Tokens::muted();
    colors.accent_foreground = Tokens::foreground();
    colors.primary = Tokens::primary();
    colors.primary_foreground = Tokens::primary_foreground();
    colors.secondary = Tokens::secondary();
    colors.popover = Tokens::card();
    colors.popover_foreground = Tokens::foreground();
    colors.sidebar = Tokens::card();
    colors.sidebar_border = Tokens::border();
    colors.sidebar_foreground = Tokens::foreground();
    colors.selection = Tokens::primary().opacity(0.5);
    colors.caret = Tokens::foreground();
    apply_yaml_highlight(theme);
}

/// `yamlHighlightStyle` in `src/components/yamlCodeViewerDiagnostics.ts`, on the
/// editor's `bg-card`. `ThemeStyle` fields are private, so styles go through its
/// serde form, the same path gpui-component uses for theme files.
fn apply_yaml_highlight(theme: &mut Theme) {
    let style = |color: Hsla| -> Option<ThemeStyle> {
        Some(
            serde_json::from_value(serde_json::json!({ "color": color }))
                .expect("ThemeStyle accepts a color"),
        )
    };
    let mut highlight = (*theme.highlight_theme).clone();
    let editor = &mut highlight.style;
    editor.editor_background = Some(Tokens::card());
    // `.cm-gutters`: `color-mix(in oklch, var(--card) 88%, black)`.
    editor.editor_gutter_background = Some(mix(Tokens::card(), oklch(0., 0., 0., 1.), 0.88));
    // The read-only viewer has no active-line highlight (`editable ? highlightActiveLine() : []`).
    // GPUI's theme is per app, so the draft editor loses it too.
    editor.editor_active_line = None;
    let syntax = &mut editor.syntax;
    syntax.keyword = style(oklch(0.78, 0.12, 250., 1.));
    syntax.boolean = style(oklch(0.8, 0.12, 305., 1.));
    syntax.constant = style(oklch(0.8, 0.12, 305., 1.));
    syntax.number = style(oklch(0.82, 0.12, 78., 1.));
    syntax.string = style(oklch(0.82, 0.08, 145., 1.));
    // YAML keys: CodeMirror tags them `definition(propertyName)`.
    syntax.property = style(oklch(0.84, 0.1, 225., 1.));
    syntax.comment = style(Tokens::muted_foreground());
    syntax.punctuation = style(oklch(0.78, 0.01, 90., 1.));
    syntax.punctuation_delimiter = style(oklch(0.78, 0.01, 90., 1.));
    syntax.punctuation_bracket = style(oklch(0.78, 0.01, 90., 1.));
    syntax.operator = style(oklch(0.78, 0.08, 20., 1.));
    theme.highlight_theme = std::sync::Arc::new(highlight);
}
