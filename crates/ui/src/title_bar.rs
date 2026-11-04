//! A strip of the app's own UI as the window's title bar, for windows whose title bar the app draws
//! over (on macOS, a transparent title bar over a full-size content view): dragging its empty part
//! moves the window.
//!
//! Adapted from ArtCraft's `craft_window::title_bar` (storytold/craft-libs, Apache-2.0): the drag
//! detection is theirs. Left out is their bookkeeping of AppKit's `movable` flag, needed only when
//! controls sit inside the system's own title-bar band; Splicr keeps that band empty.

use eframe::egui::{Rect, Response, Sense, Ui, ViewportCommand};

/// Makes the empty part of `rect` act as a title bar. Call it before drawing the strip's controls,
/// so they sit on top and take their own clicks. In full screen it does nothing.
///
/// What a double-click does is the system's setting, so the caller acts on `double_clicked()` on
/// the returned response (see `splicr_platform::title_bar_double_click`).
///
/// The area senses clicks only: a drag-sensing one would also get drags that start on a click-only
/// control (egui gives clicks and drags to different widgets). So a drag is a press on the empty
/// area that has stopped being a click.
pub fn empty_area(ui: &mut Ui, rect: Rect) -> Response {
    let response = ui.interact(rect, ui.scope_id().with("title_bar_empty"), Sense::CLICK);
    // egui forgets which widget a press landed on once it can no longer be a click, so the press is
    // remembered, by its start time, while it still can.
    let armed = response.id.with("drag_press");
    if ui.input(|i| i.viewport().fullscreen).unwrap_or(false) {
        ui.data_mut(|d| d.remove::<Option<f64>>(armed));
        return response;
    }
    let press = ui.input(|i| i.pointer.press_start_time());
    if response.is_pointer_button_down_on() {
        ui.data_mut(|d| d.insert_temp(armed, press));
    }
    let dragging = ui.input(|i| i.pointer.is_decidedly_dragging());
    if press.is_some() && dragging && ui.data(|d| d.get_temp::<Option<f64>>(armed)) == Some(press) {
        // Once per press: AppKit's drag loop takes the mouse-up, so egui may still see the button
        // down afterwards.
        ui.data_mut(|d| d.remove::<Option<f64>>(armed));
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    response
}

/// Whether the window fills the screen, where a title bar's double-click does nothing.
pub fn is_fullscreen(ui: &Ui) -> bool {
    ui.input(|i| i.viewport().fullscreen).unwrap_or(false)
}

/// Whether the window is maximized (zoomed, on macOS).
pub fn is_maximized(ui: &Ui) -> bool {
    ui.input(|i| i.viewport().maximized).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Context, Event, PointerButton, Pos2, RawInput, pos2, vec2};

    use super::*;

    fn frame(ctx: &Context, events: Vec<Event>) -> Vec<ViewportCommand> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            empty_area(ui, Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 40.0)));
        });
        // No renderer here to take the font atlas.
        output.textures_delta.clear();
        output.viewport_output.into_values().flat_map(|v| v.commands).collect()
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    #[test]
    fn dragging_the_empty_area_moves_the_window_once() {
        let ctx = Context::default();
        frame(&ctx, vec![Event::PointerMoved(pos2(100.0, 20.0))]);
        frame(&ctx, vec![press(pos2(100.0, 20.0), true)]);
        let moved = frame(&ctx, vec![Event::PointerMoved(pos2(160.0, 20.0))]);
        assert!(moved.contains(&ViewportCommand::StartDrag), "{moved:?}");
        let again = frame(&ctx, vec![Event::PointerMoved(pos2(200.0, 20.0))]);
        assert!(!again.contains(&ViewportCommand::StartDrag));
    }

    #[test]
    fn a_drag_starting_elsewhere_doesnt_move_the_window() {
        let ctx = Context::default();
        frame(&ctx, vec![Event::PointerMoved(pos2(100.0, 200.0))]);
        frame(&ctx, vec![press(pos2(100.0, 200.0), true)]);
        let moved = frame(&ctx, vec![Event::PointerMoved(pos2(100.0, 20.0))]);
        assert!(!moved.contains(&ViewportCommand::StartDrag), "{moved:?}");
    }
}
