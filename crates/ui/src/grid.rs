//! A spreadsheet-like view of any number of rows: only the cells on screen are laid out and
//! painted, so a million rows scroll as smoothly as ten. The header row and the row numbers stay
//! put while the cells scroll under them; columns are resized by dragging their header's edge; a
//! cell is picked with the mouse or the arrow keys and copied with the system's shortcut.
//!
//! The ideas are GridCraft's (`crates/ui-egui/src/grid/`): paint only the visible span, keep the
//! frozen parts on top of the scrolled ones, and draw the header and gutter from the viewport rather
//! than from the content. Rows here are all one height, so the span is a division, not GridCraft's
//! prefix-sum search; columns, which can be resized, are found by a binary search over their edges.

use std::borrow::Cow;

use eframe::egui::{
    self, Align2, AsIdSalt, CursorIcon, EventFilter, FontId, Key, Pos2, Rect, Role, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, WidgetInfo, pos2, text::LayoutJob, vec2,
};

use crate::{Palette, semibold};

const ROW_H: f32 = 26.0;
const HEADER_H: f32 = 30.0;
const DEFAULT_COL_W: f32 = 140.0;
const MIN_COL_W: f32 = 48.0;
const MAX_COL_W: f32 = 800.0;
const PAD: f32 = 8.0;

/// What a grid shows. Implement it over any table; the grid asks only for what is on screen.
pub trait GridData {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    /// The header of `col`.
    fn column_title(&self, col: usize) -> Cow<'_, str>;
    /// The text of a cell.
    fn cell(&self, row: usize, col: usize) -> Cow<'_, str>;
    /// Whether the cell is a number or a date, which line up on the right.
    fn numeric(&self, _row: usize, _col: usize) -> bool {
        false
    }
}

/// What the user did to the grid this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridResponse {
    /// The cell picked, as row and column.
    pub selected: Option<(usize, usize)>,
    /// A cell's text was copied to the clipboard.
    pub copied: bool,
}

#[derive(Clone, Debug, Default)]
struct State {
    widths: Vec<f32>,
    selected: Option<(usize, usize)>,
}

/// Shows `data` filling the space left. `id_salt` keeps each table's column widths and selection
/// apart: give a different one per table.
pub fn data_grid(ui: &mut Ui, id_salt: impl AsIdSalt, data: &dyn GridData) -> GridResponse {
    let id = ui.make_persistent_id(id_salt);
    let mut state: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let (rows, cols) = (data.rows(), data.cols());
    state.widths.resize(cols, DEFAULT_COL_W);
    state.selected = state.selected.filter(|&(r, c)| r < rows && c < cols);
    let p = *Palette::of(ui);
    let digits = rows.max(1).to_string().len() as f32;
    let gutter = (digits * 8.0 + 2.0 * PAD).max(44.0);
    let mut edges = Vec::with_capacity(cols + 1);
    edges.push(0.0);
    for w in &state.widths {
        edges.push(edges.last().copied().unwrap_or(0.0) + w);
    }
    let content = vec2(gutter + edges[cols], HEADER_H + rows as f32 * ROW_H);
    let mut out = GridResponse::default();

    let frame = egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(10).inner_margin(1);
    frame.show(ui, |ui| {
        ScrollArea::both().auto_shrink(false).show_viewport(ui, |ui, viewport| {
            let origin = ui.min_rect().min;
            let (_, response) = ui.allocate_exact_size(content, Sense::click());
            let painter = ui.painter().clone();
            // On screen: where the scrolled view starts, and the frozen header and gutter.
            let top = origin.y + viewport.min.y;
            let left = origin.x + viewport.min.x;
            let cell_x = |c: usize| origin.x + gutter + edges[c];
            let cell_y = |r: usize| origin.y + HEADER_H + r as f32 * ROW_H;

            let first_row = ((viewport.min.y / ROW_H).floor() as usize).min(rows);
            let last_row = (((viewport.max.y - HEADER_H) / ROW_H).ceil().max(0.0) as usize + 1).min(rows);
            let col_at = |x: f32| edges.partition_point(|&e| e <= x).saturating_sub(1).min(cols.saturating_sub(1));
            let first_col = col_at((viewport.min.x - gutter).max(0.0));
            let last_col = if cols == 0 { 0 } else { col_at(viewport.max.x - gutter) + 1 };

            // Cells, with their grid lines.
            let font = FontId::proportional(13.0);
            for r in first_row..last_row {
                let y = cell_y(r);
                for c in first_col..last_col {
                    let rect = Rect::from_min_size(pos2(cell_x(c), y), vec2(state.widths[c], ROW_H));
                    if state.selected == Some((r, c)) {
                        painter.rect_filled(rect, 0, p.accent_soft);
                    }
                    let text = data.cell(r, c);
                    if !text.is_empty() {
                        let right = data.numeric(r, c);
                        text_in(&painter, rect, &text, font.clone(), p.text, right);
                    }
                    painter.vline(rect.right(), rect.y_range(), Stroke::new(1.0, p.border));
                }
                painter.hline(cell_x(first_col)..=cell_x(last_col), y + ROW_H, Stroke::new(1.0, p.border));
            }
            if let Some((r, c)) = state.selected {
                let rect = Rect::from_min_size(pos2(cell_x(c), cell_y(r)), vec2(state.widths[c], ROW_H));
                painter.rect_stroke(rect, 0, Stroke::new(2.0, p.accent), StrokeKind::Inside);
            }

            // The frozen header: column titles, with a handle on each right edge to resize.
            let header = Rect::from_min_size(pos2(left, top), vec2(viewport.width(), HEADER_H));
            painter.rect_filled(header, 0, p.surface_hover);
            let header_font = semibold(13.0);
            for c in first_col..last_col {
                let rect = Rect::from_min_size(pos2(cell_x(c), top), vec2(state.widths[c], HEADER_H));
                let picked = state.selected.is_some_and(|(_, sc)| sc == c);
                let ink = if picked { p.accent } else { p.text };
                text_in(&painter, rect, &data.column_title(c), header_font.clone(), ink, false);
                painter.vline(rect.right(), rect.y_range(), Stroke::new(1.0, p.border));
                let handle = Rect::from_center_size(pos2(rect.right(), rect.center().y), vec2(8.0, HEADER_H));
                let drag = ui
                    .interact(handle, id.with(("resize", c)), Sense::drag())
                    .on_hover_cursor(CursorIcon::ResizeColumn);
                if drag.dragged() {
                    state.widths[c] = (state.widths[c] + drag.drag_delta().x).clamp(MIN_COL_W, MAX_COL_W);
                }
            }
            painter.hline(header.x_range(), header.bottom(), Stroke::new(1.0, p.control));

            // The frozen gutter: row numbers.
            let gutter_rect =
                Rect::from_min_max(pos2(left, top + HEADER_H), pos2(left + gutter, top + viewport.height()));
            painter.rect_filled(gutter_rect, 0, p.surface_hover);
            let small = FontId::proportional(12.0);
            for r in first_row..last_row {
                let y = cell_y(r) + ROW_H / 2.0;
                if y < top + HEADER_H {
                    continue;
                }
                let picked = state.selected.is_some_and(|(sr, _)| sr == r);
                let ink = if picked { p.accent } else { p.muted };
                let at = pos2(left + gutter - PAD, y);
                painter.with_clip_rect(gutter_rect).text(
                    at,
                    Align2::RIGHT_CENTER,
                    (r + 1).to_string(),
                    small.clone(),
                    ink,
                );
            }
            painter.vline(left + gutter, gutter_rect.y_range(), Stroke::new(1.0, p.control));
            let corner = Rect::from_min_size(pos2(left, top), vec2(gutter, HEADER_H));
            painter.rect_filled(corner, 0, p.surface_hover);
            painter.rect_stroke(corner, 0, Stroke::new(1.0, p.control), StrokeKind::Inside);

            // Picking a cell, by mouse or keyboard.
            if response.clicked() {
                response.request_focus();
                if let Some(at) = response.interact_pointer_pos() {
                    let in_cells = at.x > left + gutter && at.y > top + HEADER_H;
                    if in_cells && rows > 0 && cols > 0 {
                        let row = (((at.y - origin.y - HEADER_H) / ROW_H) as usize).min(rows - 1);
                        let col = col_at(at.x - origin.x - gutter);
                        state.selected = Some((row, col));
                    }
                }
            }
            if response.has_focus() {
                let filter = EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() };
                ui.memory_mut(|m| m.set_focus_lock_filter(response.id, filter));
                if let Some(moved) = arrow_move(ui, state.selected, rows, cols) {
                    state.selected = Some(moved);
                    let (r, c) = moved;
                    let rect = Rect::from_min_size(pos2(cell_x(c), cell_y(r)), vec2(state.widths[c], ROW_H));
                    // Clear of the frozen header and gutter.
                    ui.scroll_to_rect(rect.expand2(vec2(gutter, HEADER_H)), None);
                }
                let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
                if let (true, Some((r, c))) = (copy, state.selected) {
                    ui.ctx().copy_text(data.cell(r, c).into_owned());
                    out.copied = true;
                }
            }
            response.widget_info(|| WidgetInfo::labeled(Role::Grid, ui.is_enabled(), "grid"));
        });
    });

    out.selected = state.selected;
    ui.data_mut(|d| d.insert_temp(id, state));
    out
}

/// The selection after this frame's arrow keys, if one was pressed.
fn arrow_move(ui: &Ui, selected: Option<(usize, usize)>, rows: usize, cols: usize) -> Option<(usize, usize)> {
    if rows == 0 || cols == 0 {
        return None;
    }
    let pressed = |key| ui.input(|i| i.key_pressed(key));
    let (r, c) = selected.unwrap_or((0, 0));
    let moved = if pressed(Key::ArrowDown) {
        (r + 1).min(rows - 1)
    } else if pressed(Key::ArrowUp) {
        r.saturating_sub(1)
    } else {
        r
    };
    let moved_col = if pressed(Key::ArrowRight) {
        (c + 1).min(cols - 1)
    } else if pressed(Key::ArrowLeft) {
        c.saturating_sub(1)
    } else {
        c
    };
    let any = [Key::ArrowDown, Key::ArrowUp, Key::ArrowLeft, Key::ArrowRight].into_iter().any(pressed);
    any.then_some((moved, moved_col))
}

/// `text` in `rect`, on one line, cut with an ellipsis where it doesn't fit.
fn text_in(painter: &egui::Painter, rect: Rect, text: &str, font: FontId, color: egui::Color32, right: bool) {
    let width = rect.width() - 2.0 * PAD;
    if width <= 0.0 {
        return;
    }
    // One line: a cell's line breaks read as spaces, as spreadsheets show them in the grid.
    let line: Cow<'_, str> =
        if text.contains(['\n', '\r']) { Cow::Owned(text.replace(['\r', '\n'], " ")) } else { Cow::Borrowed(text) };
    let mut job = LayoutJob::simple_singleline(line.into_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    let galley = painter.layout_job(job);
    let y = rect.center().y - galley.size().y / 2.0;
    let x = if right { rect.right() - PAD - galley.size().x } else { rect.left() + PAD };
    painter.with_clip_rect(rect.intersect(painter.clip_rect())).galley(Pos2::new(x, y), galley, color);
}
