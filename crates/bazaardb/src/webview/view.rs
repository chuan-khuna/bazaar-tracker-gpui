//! The web view, in a borderless popup the caller's window owns.
//!
//! A UI that draws with DirectComposition (as gpui does) draws above any child window, so the view
//! can not be a child: it sits in an owned popup, which stays above its owner, hides when the owner
//! is minimised, and has no taskbar button. The caller moves it over its panel with [`WebView::place`].

use std::num::NonZeroIsize;
use std::path::PathBuf;
use std::sync::Once;

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SW_HIDE, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetWindowPos, ShowWindow, WNDCLASSW, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use wry::{WebContext, WebViewBuilder};

use super::{Capture, decode, script};

/// A place in the owner's client area, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The popup's window class, as a wide string.
const CLASS: &[u16] = &[b'B' as u16, b'a' as u16, b'z' as u16, b'a' as u16, b'a' as u16, b'r' as u16, b'D' as u16, b'b' as u16, b'W' as u16, b'e' as u16, b'b' as u16, 0];

/// A window handle wry can build in.
struct Popup(HWND);

impl HasWindowHandle for Popup {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.0 as isize).ok_or(HandleError::Unavailable)?;
        // SAFETY: the popup lives as long as the `WebView` that holds it.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(hwnd))) })
    }
}

/// A Bazaar DB page in a web view.
pub struct WebView {
    owner: HWND,
    popup: Popup,
    /// Taken out first when dropped, so the view goes before its window.
    view: Option<wry::WebView>,
    _context: WebContext,
}

impl WebView {
    /// Open `url` in a hidden popup owned by `owner`. Each reply the page gets to a watched API path
    /// goes to `on_capture`. Browser data (cookies, Cloudflare's pass) is kept in `data_dir`.
    pub fn open(owner: &impl HasWindowHandle, data_dir: PathBuf, url: &str, on_capture: impl Fn(Capture) + 'static) -> Result<Self, String> {
        let owner = match owner.window_handle().map_err(|error| error.to_string())?.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get() as HWND,
            _ => return Err("not a Windows window".into()),
        };
        let popup = Popup(create_popup(owner)?);
        let mut context = WebContext::new(Some(data_dir));
        let view = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(url)
            .with_initialization_script_for_main_only(script(), true)
            .with_ipc_handler(move |request| {
                if let Some(capture) = decode(request.body()) {
                    on_capture(capture);
                }
            })
            .with_devtools(true)
            .build(&popup)
            .map_err(|error| {
                // SAFETY: the popup was made above and nothing else holds it.
                unsafe { DestroyWindow(popup.0) };
                error.to_string()
            })?;
        Ok(Self { owner, popup, view: Some(view), _context: context })
    }

    /// Show the view over `rect` of the owner's client area.
    pub fn place(&self, rect: Rect) {
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: both windows are alive while this is.
        unsafe {
            ClientToScreen(self.owner, &mut origin);
            SetWindowPos(self.popup.0, std::ptr::null_mut(), origin.x + rect.x, origin.y + rect.y, rect.width, rect.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
    }

    pub fn hide(&self) {
        // SAFETY: the popup is alive while this is.
        unsafe { ShowWindow(self.popup.0, SW_HIDE) };
    }

    pub fn reload(&self) -> Result<(), String> {
        self.view.as_ref().map_or(Ok(()), |view| view.reload().map_err(|error| error.to_string()))
    }

    pub fn load_url(&self, url: &str) -> Result<(), String> {
        self.view.as_ref().map_or(Ok(()), |view| view.load_url(url).map_err(|error| error.to_string()))
    }

    pub fn open_devtools(&self) {
        if let Some(view) = &self.view {
            view.open_devtools();
        }
    }
}

impl Drop for WebView {
    fn drop(&mut self) {
        self.view.take();
        // SAFETY: the view is gone, and nothing else holds the popup.
        unsafe { DestroyWindow(self.popup.0) };
    }
}

/// A hidden borderless popup owned by `owner`.
fn create_popup(owner: HWND) -> Result<HWND, String> {
    static REGISTER: Once = Once::new();
    // SAFETY: plain Win32 calls; the class name is a static wide string.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        REGISTER.call_once(|| {
            let class = WNDCLASSW { lpfnWndProc: Some(DefWindowProcW), hInstance: instance, lpszClassName: CLASS.as_ptr(), ..std::mem::zeroed() };
            RegisterClassW(&class);
        });
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, CLASS.as_ptr(), std::ptr::null(), WS_POPUP | WS_CLIPCHILDREN, 0, 0, 1, 1, owner, std::ptr::null_mut(), instance, std::ptr::null());
        if hwnd.is_null() { Err("could not make the web view's window".into()) } else { Ok(hwnd) }
    }
}
