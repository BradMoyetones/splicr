//! Splicr's design system for egui, Remapr's own: apps call [`install`] once, then build screens
//! from these widgets and read colours from [`Palette::of`], never from hard-coded values.
//!
//! The same look on every system, and the same as Remapr's: Inter for text, Lucide for icons, one
//! palette with checked contrast. [`title_bar`] makes a strip of the app act as the window's title
//! bar where the app draws over it (macOS). [`data_grid`] shows tables of any size.

mod grid;
mod theme;
pub mod title_bar;
mod widgets;

pub use grid::{GridData, GridResponse, data_grid};
pub use theme::{Icon, Palette, Tone, icon, install, semibold};
pub use widgets::{
    badge, banner, button, card, choice_card, danger_icon_button, drop_zone, icon_button, icon_label, list_item,
    nav_item, primary_button, small_icon_button, switch,
};
