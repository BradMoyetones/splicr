//! Splicr's widgets. The buttons, switch, card and banner are Remapr's; the navigation, choice
//! cards, badge and drop zone are Splicr's own, in the same style. Each one reports itself to screen
//! readers through `widget_info`.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, InnerResponse, Margin, Rect, Response, Role,
    Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, vec2,
};

use crate::Palette;
use crate::theme::{Icon, SEMIBOLD, Tone};

/// A square, frameless button showing one [`Icon`](crate::icon). `label` is its tooltip and
/// accessible name.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    icon_button_toned(ui, icon, label, false)
}

/// An [`icon_button`] for an action that deletes something: red, and redder on hover.
pub fn danger_icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    icon_button_toned(ui, icon, label, true)
}

fn icon_button_toned(ui: &mut Ui, icon: Icon, label: &str, danger: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(32.0), Sense::click());
    let p = Palette::of(ui);
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let (hover_fill, ring, idle, active) = if danger {
            (p.danger_soft, p.danger, p.danger.gamma_multiply(0.7), p.danger)
        } else {
            (p.surface_hover, p.accent, p.muted, p.text)
        };
        if hovered {
            ui.painter().rect_filled(rect, CornerRadius::same(8), hover_fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, ring), StrokeKind::Inside);
        }
        let color = if hovered { active } else { idle };
        paint_icon(ui, ui.painter(), icon, rect.center(), 16.0, color);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(label)
}

/// The one main action of a screen: filled with the accent colour, with an optional leading icon.
///
/// Disabled, it turns grey with a label that can still be read. egui fades a disabled widget on top
/// of whatever it draws, which took the faded accent to near nothing, so this one paints on a
/// painter of its own.
pub fn primary_button(ui: &mut Ui, icon: Option<Icon>, text: &str) -> Response {
    let p = Palette::of(ui);
    let enabled = ui.is_enabled();
    let (painter, ink) = if enabled {
        (ui.painter().clone(), p.on_accent)
    } else {
        (ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(ui.clip_rect()), p.muted)
    };
    let galley =
        ui.painter().layout_no_wrap(text.to_owned(), FontId::new(14.0, FontFamily::Name(SEMIBOLD.into())), ink);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 28.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = match (enabled, response.hovered() || response.has_focus()) {
            (false, _) => p.border,
            (true, true) => p.accent_hover,
            (true, false) => p.accent,
        };
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(10),
                Stroke::new(1.5, p.accent),
                StrokeKind::Outside,
            );
        }
        let mut x = rect.left() + 14.0;
        if let Some(icon) = icon {
            paint_icon(ui, &painter, icon, egui::pos2(x + 8.0, rect.center().y), 16.0, ink);
            x += icon_w;
        }
        painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, ink);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));
    if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response }
}

/// A secondary action: outlined on the surface, with an optional leading icon. As tall as
/// [`primary_button`], to sit beside it.
pub fn button(ui: &mut Ui, icon: Option<Icon>, text: &str) -> Response {
    let p = Palette::of(ui);
    let enabled = ui.is_enabled();
    let ink = if enabled { p.text } else { p.muted };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(14.0), ink);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 26.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = enabled && (response.hovered() || response.has_focus());
        let fill = if hovered { p.surface_hover } else { p.surface };
        let stroke = if response.has_focus() { Stroke::new(1.5, p.accent) } else { Stroke::new(1.0, p.border) };
        ui.painter().rect(rect, CornerRadius::same(8), fill, stroke, StrokeKind::Inside);
        let mut x = rect.left() + 13.0;
        if let Some(icon) = icon {
            paint_icon(ui, ui.painter(), icon, egui::pos2(x + 8.0, rect.center().y), 16.0, ink);
            x += icon_w;
        }
        ui.painter().galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, ink);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));
    if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response }
}

/// An on/off switch. Returns a response that is `changed()` when flipped.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let size = vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = Palette::of(ui);
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let track = if *on { p.accent } else { p.control };
        let radius = CornerRadius::same(11);
        ui.painter().rect_filled(rect, radius, track);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(13),
                Stroke::new(1.5, p.accent),
                StrokeKind::Outside,
            );
        }
        let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
        let knob = if *on { p.on_accent } else { p.surface };
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), 8.0, knob);
    }
    response.widget_info(|| WidgetInfo::selected(Role::Switch, ui.is_enabled(), *on, label));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A surface for one item or group. `highlight` draws an accent border (the item needs attention).
pub fn card<R>(ui: &mut Ui, highlight: bool, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = Palette::of(ui);
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(if highlight { 1.5 } else { 1.0 }, if highlight { p.accent } else { p.border }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, add_contents)
}

/// A full-width message in the colours of its `tone`, with the tone's icon.
pub fn banner(ui: &mut Ui, tone: Tone, text: &str) {
    let (strong, soft) = Palette::of(ui).tone(tone);
    egui::Frame::new()
        .fill(soft)
        .stroke(Stroke::new(1.0, strong.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icon_label(ui, tone.icon(), 16.0, strong);
                ui.add(egui::Label::new(egui::RichText::new(text).color(strong)).wrap());
            });
        });
}

/// An [`Icon`] of `size` points, laid out like a label. Decorative: give it a tooltip with
/// `on_hover_text` when it carries meaning on its own.
pub fn icon_label(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        icon.image(size, color).paint_at(ui, rect);
    }
    response
}

/// One destination in the sidebar, as wide as the sidebar: its icon and, unless `compact`, its name,
/// which is the tooltip otherwise (the item is then a square, in a sidebar as wide as it). The current one is tinted
/// with the accent.
pub fn nav_item(ui: &mut Ui, icon: Icon, label: &str, selected: bool, compact: bool) -> Response {
    let p = Palette::of(ui);
    // Compact, a 36-point square in the middle of the row; otherwise the whole row, 36 points tall.
    // Either way the icon's centre is 18 points in, so it doesn't move when the sidebar folds.
    let (rect, response) = if compact {
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
        let square = Rect::from_center_size(row.center(), Vec2::splat(36.0));
        (square, ui.interact(square, ui.make_persistent_id(("nav_item", label)), Sense::click()))
    } else {
        ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click())
    };
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let (fill, ink) = match (selected, hovered) {
            (true, _) => (Some(p.accent_soft), p.accent),
            (false, true) => (Some(p.surface_hover), p.text),
            (false, false) => (None, p.muted),
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, p.accent), StrokeKind::Inside);
        }
        let x = if compact { rect.center().x } else { rect.left() + 18.0 };
        paint_icon(ui, ui.painter(), icon, egui::pos2(x, rect.center().y), 18.0, ink);
        if !compact {
            let font = if selected {
                FontId::new(14.0, FontFamily::Name(SEMIBOLD.into()))
            } else {
                FontId::proportional(14.0)
            };
            let text = if selected { ink } else { p.text };
            ui.painter().text(egui::pos2(rect.left() + 38.0, rect.center().y), Align2::LEFT_CENTER, label, font, text);
        }
    }
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, label));
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    if compact { response.on_hover_text(label) } else { response }
}

/// A row of a list, as wide as the list: an icon, a `title` cut with an ellipsis where it doesn't
/// fit, and a muted `detail` on the right (a count, a size). The current one is tinted with the
/// accent. Rows read as items to pick, not as buttons: no border, a fill only on hover.
pub fn list_item(ui: &mut Ui, icon: Icon, title: &str, detail: &str, selected: bool) -> Response {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let (fill, ink) = match (selected, hovered) {
            (true, _) => (Some(p.accent_soft), p.accent),
            (false, true) => (Some(p.surface_hover), p.text),
            (false, false) => (None, p.text),
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.5, p.accent), StrokeKind::Inside);
        }
        let icon_ink = if selected { p.accent } else { p.muted };
        paint_icon(ui, ui.painter(), icon, egui::pos2(rect.left() + 16.0, rect.center().y), 15.0, icon_ink);
        let detail = ui.painter().layout_no_wrap(detail.to_owned(), FontId::proportional(12.0), p.muted);
        let detail_x = rect.right() - 10.0 - detail.size().x;
        let font =
            if selected { FontId::new(13.5, FontFamily::Name(SEMIBOLD.into())) } else { FontId::proportional(13.5) };
        let mut job = egui::text::LayoutJob::simple_singleline(title.to_owned(), font, ink);
        job.wrap = egui::text::TextWrapping::truncate_at_width((detail_x - 10.0 - (rect.left() + 32.0)).max(0.0));
        let title = ui.painter().layout_job(job);
        ui.painter().galley(egui::pos2(rect.left() + 32.0, rect.center().y - title.size().y / 2.0), title, ink);
        ui.painter().galley(egui::pos2(detail_x, rect.center().y - detail.size().y / 2.0), detail, p.muted);
    }
    response.widget_info(|| WidgetInfo::selected(Role::ListItem, ui.is_enabled(), selected, title));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A small pill of muted text: a file's format, a count.
pub fn badge(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    let galley =
        ui.painter().layout_no_wrap(text.to_owned(), FontId::new(11.0, FontFamily::Name(SEMIBOLD.into())), p.muted);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(12.0, 6.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, CornerRadius::same(6), p.surface_hover, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.muted);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    response
}

/// One of several options, as a card with an icon, a title and a line about it. The chosen one
/// has the accent's border and tint. Lay them out side by side with [`Ui::columns`].
pub fn choice_card(ui: &mut Ui, selected: bool, icon: Icon, title: &str, detail: &str) -> Response {
    let p = Palette::of(ui);
    let width = ui.available_width();
    let title_galley = ui.painter().layout(
        title.to_owned(),
        FontId::new(14.0, FontFamily::Name(SEMIBOLD.into())),
        p.text,
        width - 28.0,
    );
    let detail_galley = ui.painter().layout(detail.to_owned(), FontId::proportional(12.5), p.muted, width - 28.0);
    let height = 14.0 + 20.0 + 8.0 + title_galley.size().y + 2.0 + detail_galley.size().y + 14.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let (fill, stroke) = match (selected, hovered) {
            (true, _) => (p.accent_soft, Stroke::new(1.5, p.accent)),
            (false, true) => (p.surface_hover, Stroke::new(1.0, p.control)),
            (false, false) => (p.surface, Stroke::new(1.0, p.border)),
        };
        ui.painter().rect(rect, CornerRadius::same(10), fill, stroke, StrokeKind::Inside);
        let ink = if selected { p.accent } else { p.muted };
        let mut y = rect.top() + 14.0;
        paint_icon(ui, ui.painter(), icon, egui::pos2(rect.left() + 24.0, y + 10.0), 20.0, ink);
        if selected {
            let dot = egui::pos2(rect.right() - 20.0, y + 10.0);
            ui.painter().circle_filled(dot, 9.0, p.accent);
            paint_icon(ui, ui.painter(), crate::icon::CHECK, dot, 12.0, p.on_accent);
        }
        y += 28.0;
        let title_h = title_galley.size().y;
        ui.painter().galley(egui::pos2(rect.left() + 14.0, y), title_galley, p.text);
        y += title_h + 2.0;
        ui.painter().galley(egui::pos2(rect.left() + 14.0, y), detail_galley, p.muted);
    }
    response.widget_info(|| WidgetInfo::selected(Role::RadioButton, ui.is_enabled(), selected, title));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A dashed area that takes files: dropped from the system (`hovered` while they are dragged over
/// the window) or picked with a click, which the caller answers with a file dialog.
pub fn drop_zone(ui: &mut Ui, hovered: bool, icon: Icon, title: &str, detail: &str, height: f32) -> Response {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if ui.is_rect_visible(rect) {
        let active = hovered || response.hovered() || response.has_focus();
        let (fill, line) = if active { (p.accent_soft, p.accent) } else { (p.surface, p.control) };
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
        dashed_rect(ui, rect.shrink(1.0), Stroke::new(1.5, line));
        // The texts wrap, centred, to the zone's width: on a narrow window they take more lines.
        let wrap = (rect.width() - 48.0).max(80.0);
        let centred = |text: &str, font: FontId, color: Color32| {
            let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, wrap);
            job.halign = egui::Align::Center;
            ui.painter().layout_job(job)
        };
        let title = centred(title, FontId::new(15.0, FontFamily::Name(SEMIBOLD.into())), p.text);
        let detail = centred(detail, FontId::proportional(13.0), p.muted);
        let block = 32.0 + 10.0 + title.size().y + 4.0 + detail.size().y;
        let (x, mut y) = (rect.center().x, rect.center().y - block / 2.0);
        let ink = if active { p.accent } else { p.muted };
        paint_icon(ui, ui.painter(), icon, egui::pos2(x, y + 16.0), 32.0, ink);
        y += 42.0;
        let title_h = title.size().y;
        ui.painter().galley(egui::pos2(x, y), title, p.text);
        ui.painter().galley(egui::pos2(x, y + title_h + 4.0), detail, p.muted);
    }
    let name = title.to_owned();
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), &name));
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A rectangle's outline in dashes, with square corners cut: egui draws no rounded dashed paths.
fn dashed_rect(ui: &Ui, rect: Rect, stroke: Stroke) {
    let inset = 12.0;
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let sides = [
        [egui::pos2(l + inset, t), egui::pos2(r - inset, t)],
        [egui::pos2(r, t + inset), egui::pos2(r, b - inset)],
        [egui::pos2(r - inset, b), egui::pos2(l + inset, b)],
        [egui::pos2(l, b - inset), egui::pos2(l, t + inset)],
    ];
    for side in sides {
        ui.painter().extend(egui::Shape::dashed_line(&side, stroke, 6.0, 5.0));
    }
}

pub(crate) fn paint_icon(ui: &Ui, painter: &egui::Painter, icon: Icon, center: egui::Pos2, size: f32, color: Color32) {
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    if let Ok(egui::load::TexturePoll::Ready { texture }) = icon.image(size, color).load_for_size(ui.ctx(), rect.size())
    {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture.id, rect, uv, color);
    }
}
