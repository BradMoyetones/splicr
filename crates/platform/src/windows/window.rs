//! Native window icons and title bar. From Remapr (`crates/platform/src/windows/window.rs`), less
//! what only a tray app needs.

use std::ptr::null;
use std::sync::atomic::{AtomicU8, AtomicU16, Ordering::Relaxed};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::UI::Controls::SetWindowTheme;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetForegroundWindow, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, PostMessageW,
    SM_CXICON, SM_CXSMICON, SendMessageW, WM_APP, WM_DPICHANGED, WM_NCACTIVATE, WM_SETICON, WM_SETTINGCHANGE,
    WM_SHOWWINDOW,
};

const SUBCLASS_ID: usize = 1;
const WM_REPAINT_TITLE_BAR: u32 = WM_APP + 0x52;
/// The title bar theme the app chose: 0 follows Windows, 1 is light, 2 is dark.
static TITLE_BAR_THEME: AtomicU8 = AtomicU8::new(0);
/// The icon resource `use_exe_icon` set, loaded again at each DPI change.
static ICON_ID: AtomicU16 = AtomicU16::new(0);

/// Repaints the title bar of `hwnd` each time the window is shown.
///
/// egui gives the window its light or dark theme before it first appears, but Windows paints that
/// first title bar in the system theme and only corrects it on the next activation change: a light
/// app on a dark system showed a dark title bar until the user switched away and back.
pub fn repaint_title_bar_when_shown(hwnd: isize) {
    // SAFETY: subclassing a window this process owns; installing again only updates it.
    unsafe { SetWindowSubclass(hwnd as _, Some(window_subclass), SUBCLASS_ID, 0) };
}

/// The theme the app forces on its title bar (`Some(true)` is dark), or `None` to follow Windows.
///
/// egui applies it, but winit puts the system theme back on every settings change Windows
/// broadcasts (wallpaper, accent colour…); the window subclass applies the app's again right after.
pub fn keep_title_bar_theme(dark: Option<bool>) {
    TITLE_BAR_THEME.store(dark.map_or(0, |dark| if dark { 2 } else { 1 }), Relaxed);
}

/// Paints the title bar of `hwnd` in the app's colours: its background, title and border, as RGB.
/// The native frame stays (Snap Layouts, shadow, resizing); with the caption in the app's
/// background, the title bar and the app read as one surface. Windows 11 only: Windows 10 ignores
/// these attributes and keeps its light or dark title bar.
pub fn tint_title_bar(hwnd: isize, caption: [u8; 3], text: [u8; 3], border: [u8; 3]) {
    // COLORREF: 0x00BBGGRR.
    let colorref = |[r, g, b]: [u8; 3]| u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16;
    for (attribute, color) in [(DWMWA_CAPTION_COLOR, caption), (DWMWA_TEXT_COLOR, text), (DWMWA_BORDER_COLOR, border)] {
        let value = colorref(color);
        // SAFETY: a COLORREF-sized value for an attribute that takes one, on our own window. A
        // failure (Windows 10) leaves the frame as it was.
        unsafe {
            DwmSetWindowAttribute(hwnd as HWND, attribute as u32, (&raw const value).cast(), size_of::<u32>() as u32)
        };
    }
}

unsafe extern "system" fn window_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: plain messages to our own window; everything else is forwarded untouched.
    unsafe {
        if msg == WM_REPAINT_TITLE_BAR {
            // A round trip through the other activation state and back repaints the frame. Straight
            // to DefWindowProc: winit reads WM_NCACTIVATE as a focus change, and a fake one left
            // egui believing the window had lost the focus.
            let active = usize::from(GetForegroundWindow() == hwnd);
            DefWindowProcW(hwnd, WM_NCACTIVATE, active ^ 1, 0);
            DefWindowProcW(hwnd, WM_NCACTIVATE, active, 0);
            return 0;
        }
        if msg == WM_SHOWWINDOW && wparam != 0 {
            // Posted, because the window is only on screen once WM_SHOWWINDOW returns.
            PostMessageW(hwnd, WM_REPAINT_TITLE_BAR, 0, 0);
        }
        let result = DefSubclassProc(hwnd, msg, wparam, lparam);
        let forced = TITLE_BAR_THEME.load(Relaxed);
        if msg == WM_SETTINGCHANGE && forced != 0 {
            use_dark_mode_colors(hwnd, forced == 2);
        }
        if msg == WM_DPICHANGED && ICON_ID.load(Relaxed) != 0 {
            set_icons(hwnd, ICON_ID.load(Relaxed));
        }
        result
    }
}

/// Light or dark title bar colours, set exactly the way winit sets them: the window's visual style,
/// then the undocumented `SetWindowCompositionAttribute(WCA_USEDARKMODECOLORS)`. Once winit has
/// used that, the documented `DWMWA_USE_IMMERSIVE_DARK_MODE` no longer wins over it, so only the same
/// pair undoes it. The style change also makes Windows redraw the frame.
fn use_dark_mode_colors(hwnd: HWND, dark: bool) {
    #[repr(C)]
    struct Data {
        attribute: u32,
        value: *mut std::ffi::c_void,
        size: usize,
    }
    const WCA_USEDARKMODECOLORS: u32 = 26;
    type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut Data) -> i32;
    // SAFETY: user32 is loaded in every GUI process; the function, if present, has this signature
    // (the one winit declares), and `value` outlives the call.
    unsafe {
        let user32 = GetModuleHandleW(super::wide("user32.dll").as_ptr());
        let Some(set) = GetProcAddress(user32, c"SetWindowCompositionAttribute".as_ptr().cast()) else { return };
        let set: SetWindowCompositionAttribute = std::mem::transmute(set);
        let theme = super::wide(if dark { "DarkMode_Explorer" } else { "" });
        SetWindowTheme(hwnd, theme.as_ptr(), null());
        let mut value = i32::from(dark);
        let mut data =
            Data { attribute: WCA_USEDARKMODECOLORS, value: (&raw mut value).cast(), size: size_of::<i32>() };
        set(hwnd, &mut data);
    }
}

/// Gives the window `hwnd` the executable's icon resource `id`, at the exact sizes its DPI asks for,
/// again whenever it moves to a monitor with another DPI. (eframe can only set one bitmap, which
/// Windows rescales blurrily.)
pub fn use_exe_icon(hwnd: isize, id: u16) {
    ICON_ID.store(id, Relaxed);
    // SAFETY: as in `repaint_title_bar_when_shown`; all share one subclass.
    unsafe { SetWindowSubclass(hwnd as _, Some(window_subclass), SUBCLASS_ID, 0) };
    set_icons(hwnd as _, id);
}

fn set_icons(hwnd: HWND, id: u16) {
    // SAFETY: plain Win32 calls on a window owned by this process; a failed load is skipped.
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let module = GetModuleHandleW(null());
        for (which, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
            let size = GetSystemMetricsForDpi(metric, dpi);
            // MAKEINTRESOURCE: a numeric resource id travels as the pointer value.
            let icon = LoadImageW(module, usize::from(id) as _, IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, which as usize, icon as isize);
            }
        }
    }
}
