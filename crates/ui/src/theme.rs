//! Colour tokens for light and dark, fonts, icons and the egui style built from them: Remapr's,
//! so the two apps look like one family.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Theme,
    Visuals,
};

/// Every colour a screen may use. Pick the right set with [`Palette::of`].
///
/// Text and status colours meet WCAG AA (4.5:1) on every surface they sit on, body text AAA (7:1),
/// and controls 3:1; the `contrast` test checks each pairing.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    /// Outlines and tracks of controls that must stand out from the surface (a switch that is off).
    pub control: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub info: Color32,
    pub info_soft: Color32,
    /// Behind a modal: dims the window while keeping it visible.
    pub scrim: Color32,
}

impl Palette {
    pub const LIGHT: Self = Self {
        bg: Color32::from_rgb(0xF6, 0xF6, 0xF8),
        surface: Color32::WHITE,
        surface_hover: Color32::from_rgb(0xF1, 0xF1, 0xF4),
        border: Color32::from_rgb(0xE4, 0xE4, 0xE9),
        control: Color32::from_rgb(0x84, 0x84, 0x8E),
        text: Color32::from_rgb(0x18, 0x18, 0x1B),
        muted: Color32::from_rgb(0x6B, 0x6B, 0x76),
        accent: Color32::from_rgb(0x4F, 0x46, 0xE5),
        accent_hover: Color32::from_rgb(0x43, 0x38, 0xCA),
        on_accent: Color32::WHITE,
        accent_soft: Color32::from_rgb(0xEE, 0xED, 0xFD),
        danger: Color32::from_rgb(0xB9, 0x1C, 0x1C),
        danger_soft: Color32::from_rgb(0xFE, 0xF2, 0xF2),
        warning: Color32::from_rgb(0xB4, 0x53, 0x09),
        warning_soft: Color32::from_rgb(0xFF, 0xFB, 0xEB),
        success: Color32::from_rgb(0x15, 0x80, 0x3D),
        success_soft: Color32::from_rgb(0xF0, 0xFD, 0xF4),
        info: Color32::from_rgb(0x25, 0x63, 0xEB),
        info_soft: Color32::from_rgb(0xEF, 0xF6, 0xFF),
        scrim: Color32::from_rgba_unmultiplied_const(0x18, 0x18, 0x1B, 0x66),
    };

    /// Near black rather than black: deep on OLED screens, while surfaces and borders stay apart.
    pub const DARK: Self = Self {
        bg: Color32::from_rgb(0x09, 0x09, 0x0B),
        surface: Color32::from_rgb(0x14, 0x14, 0x17),
        surface_hover: Color32::from_rgb(0x1C, 0x1C, 0x20),
        border: Color32::from_rgb(0x27, 0x27, 0x2D),
        control: Color32::from_rgb(0x6C, 0x6C, 0x76),
        text: Color32::from_rgb(0xF4, 0xF4, 0xF5),
        muted: Color32::from_rgb(0xA1, 0xA1, 0xAA),
        accent: Color32::from_rgb(0x81, 0x8C, 0xF8),
        accent_hover: Color32::from_rgb(0xA5, 0xB4, 0xFC),
        on_accent: Color32::from_rgb(0x09, 0x09, 0x0B),
        accent_soft: Color32::from_rgb(0x1E, 0x1C, 0x3A),
        danger: Color32::from_rgb(0xF8, 0x71, 0x71),
        danger_soft: Color32::from_rgb(0x2A, 0x13, 0x16),
        warning: Color32::from_rgb(0xFB, 0xBF, 0x24),
        warning_soft: Color32::from_rgb(0x27, 0x1D, 0x09),
        success: Color32::from_rgb(0x4A, 0xDE, 0x80),
        success_soft: Color32::from_rgb(0x0E, 0x23, 0x16),
        info: Color32::from_rgb(0x60, 0xA5, 0xFA),
        info_soft: Color32::from_rgb(0x0D, 0x1B, 0x30),
        scrim: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0x99),
    };

    pub fn of(ui: &egui::Ui) -> &'static Self {
        if ui.visuals().dark_mode { &Self::DARK } else { &Self::LIGHT }
    }

    /// The strong and soft colours of a message's `tone`.
    pub fn tone(&self, tone: Tone) -> (Color32, Color32) {
        match tone {
            Tone::Info => (self.info, self.info_soft),
            Tone::Success => (self.success, self.success_soft),
            Tone::Warning => (self.warning, self.warning_soft),
            Tone::Danger => (self.danger, self.danger_soft),
        }
    }
}

/// What a message is about, which sets its colours and icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn icon(self) -> Icon {
        match self {
            Tone::Info => icon::INFO,
            Tone::Success => icon::SUCCESS,
            Tone::Warning => icon::WARNING,
            Tone::Danger => icon::ERROR,
        }
    }
}

/// One of Splicr's icons: a Lucide SVG (ISC licence, `assets/icons`), drawn at any size and tinted
/// to the palette. Pick one from [`icon`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon {
    uri: &'static str,
    svg: &'static str,
}

impl Icon {
    /// The icon as an egui image of `size` points, in `color`.
    pub fn image(self, size: f32, color: Color32) -> egui::Image<'static> {
        egui::Image::from_uri(self.uri).fit_to_exact_size(egui::Vec2::splat(size)).tint(color)
    }
}

macro_rules! icons {
    ($($name:ident = $file:literal,)*) => {
        $(pub const $name: super::Icon = super::Icon {
            uri: concat!("bytes://splicr/icons/", $file, ".svg"),
            svg: include_str!(concat!("../assets/icons/", $file, ".svg")),
        };)*
        pub(crate) const ALL: &[super::Icon] = &[$($name),*];
    };
}

/// Every icon the app may use. New ones come from the same Lucide version (see ATTRIBUTION.md).
pub mod icon {
    icons! {
        ADD = "plus",
        ARCHIVE = "file-archive",
        BACK = "chevron-left",
        CHECK = "check",
        CLOSE = "x",
        COLLAPSED = "chevron-right",
        COMBINE = "combine",
        CSV = "file-text",
        DELETE = "trash",
        DOWNLOAD = "download",
        EDIT = "pencil",
        ERROR = "octagon-alert",
        EXPANDED = "chevron-down",
        FILE_ADD = "file-plus",
        FOLDER = "folder-open",
        FORWARD = "arrow-right",
        HOME = "house",
        INFO = "info",
        LAYERS = "layers",
        MERGE = "merge",
        MOON = "moon",
        MORE = "ellipsis",
        REFRESH = "refresh-cw",
        RELATE = "link-2",
        SEARCH = "search",
        SETTINGS = "settings",
        SHEET = "sheet",
        SIDEBAR = "panel-left",
        SPLIT = "split",
        SPREADSHEET = "file-spreadsheet",
        SUCCESS = "circle-check",
        SUN = "sun",
        SYSTEM = "sun-moon",
        TABLE = "table-2",
        WARNING = "triangle-alert",
    }
}

pub(crate) const SEMIBOLD: &str = "semibold";

/// Inter, the logo's typeface (SIL OFL 1.1), cut down to the characters Splicr's languages use (see
/// `assets/fonts/README.md`).
const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

/// Fonts, icons, text sizes, spacing and both themes' visuals. Call once at startup.
pub fn install(ctx: &Context) {
    ctx.set_fonts(fonts());
    install_icons(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Name(SEMIBOLD.into()))),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        // Tooltips name icon-only buttons: show them at once, even while the pointer still moves.
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    ctx.set_visuals_of(Theme::Light, visuals(Visuals::light(), &Palette::LIGHT));
    ctx.set_visuals_of(Theme::Dark, visuals(Visuals::dark(), &Palette::DARK));
}

/// Registers every icon under its URI, drawn white so `Image::tint` gives it any colour. The SVG
/// loader rasterises each at the size and DPI it is shown at, so icons stay sharp.
fn install_icons(ctx: &Context) {
    egui_extras::install_image_loaders(ctx);
    for icon in icon::ALL {
        ctx.include_bytes(icon.uri, icon.svg.replace("currentColor", "white").into_bytes());
    }
}

fn visuals(mut v: Visuals, p: &Palette) -> Visuals {
    let radius = CornerRadius::same(8);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.surface_hover;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    for (w, fill) in [
        (&mut v.widgets.inactive, p.surface),
        (&mut v.widgets.hovered, p.surface_hover),
        (&mut v.widgets.active, p.border),
        (&mut v.widgets.open, p.surface_hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, p.border);
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.corner_radius = radius;
        w.expansion = 0.0;
    }
    v
}

/// Inter on every system, so Splicr looks the same everywhere. egui's bundled fonts stay as the
/// fallback for anything outside the cut (other scripts, emoji).
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), Arc::new(FontData::from_static(INTER_REGULAR)));
    fonts.font_data.insert("inter-semibold".into(), Arc::new(FontData::from_static(INTER_SEMIBOLD)));
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    let semibold = std::iter::once("inter-semibold".to_owned()).chain(fallback).collect();
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    fonts
}

/// Inter SemiBold at `size`, for titles and emphasis.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2 contrast ratio between two opaque colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        fn luminance(c: Color32) -> f32 {
            let channel = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
        }
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn palette_contrast() {
        const AAA: f32 = 7.0;
        const AA: f32 = 4.5;
        const CONTROL: f32 = 3.0;
        for (name, p) in [("light", &Palette::LIGHT), ("dark", &Palette::DARK)] {
            let mut pairs = vec![
                ("text", p.text, "accent_soft", p.accent_soft, AAA),
                ("accent", p.accent, "accent_soft", p.accent_soft, AA),
                ("on_accent", p.on_accent, "accent", p.accent, AA),
                ("on_accent", p.on_accent, "accent_hover", p.accent_hover, AA),
                // A disabled primary button: exempt from WCAG, still meant to be read.
                ("muted", p.muted, "border", p.border, CONTROL),
            ];
            for (s, bg) in [("bg", p.bg), ("surface", p.surface), ("surface_hover", p.surface_hover)] {
                pairs.push(("text", p.text, s, bg, AAA));
                pairs.push(("muted", p.muted, s, bg, AA));
                pairs.push(("accent", p.accent, s, bg, AA));
                pairs.push(("control", p.control, s, bg, CONTROL));
            }
            for tone in [Tone::Info, Tone::Success, Tone::Warning, Tone::Danger] {
                let (strong, soft) = p.tone(tone);
                for (s, bg) in [("its soft fill", soft), ("bg", p.bg), ("surface", p.surface)] {
                    pairs.push(("a tone", strong, s, bg, AA));
                }
            }
            for (what, fg, on, bg, min) in pairs {
                let ratio = contrast(fg, bg);
                assert!(ratio >= min, "{name}: {what} {fg:?} on {on} {bg:?} is {ratio:.2}:1, needs {min}:1");
            }
        }
    }

    #[test]
    fn every_icon_is_a_tintable_svg() {
        for icon in icon::ALL {
            assert!(icon.svg.contains("<svg") && icon.svg.contains("currentColor"), "{}", icon.uri);
        }
    }
}
