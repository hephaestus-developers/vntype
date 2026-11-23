//! Thông báo nổi trên màn hình (On-Screen Display - OSD) khi người dùng chuyển chế độ Việt/Anh.

/// Cấu hình cửa sổ nổi OSD hiển thị ngắn hạn giữa màn hình.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsdBanner {
    /// Ký hiệu ngắn (`"VN"` hoặc `"EN"`).
    pub badge: &'static str,
    /// Tiêu đề trạng thái (`"Tiếng Việt"` hoặc `"Tiếng Anh"`).
    pub title: &'static str,
    /// Thời gian tự động ẩn tính bằng mili-giây.
    pub duration_ms: u32,
}

impl OsdBanner {
    /// Tạo thông báo OSD cho trạng thái bật/tắt tiếng Việt mới.
    pub fn for_vietnamese_state(enabled: bool) -> Self {
        if enabled {
            OsdBanner { badge: "VN", title: "Tiếng Việt", duration_ms: 1200 }
        } else {
            OsdBanner { badge: "EN", title: "Tiếng Anh", duration_ms: 1200 }
        }
    }
}

#[cfg(windows)]
pub mod win_osd {
    use core::ffi::c_void;
    use std::cell::RefCell;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject,
        DrawTextW, EndPaint, FillRect, PAINTSTRUCT, RoundRect, SelectObject, SetBkMode, SetTextColor,
        TRANSPARENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, KillTimer, LWA_ALPHA,
        RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, SetLayeredWindowAttributes,
        SetTimer, ShowWindow, WM_DESTROY, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    use super::OsdBanner;

    const OSD_WIDTH: i32 = 220;
    const OSD_HEIGHT: i32 = 64;
    const OSD_TIMER_ID: usize = 1;

    thread_local! {
        static CURRENT_BANNER: RefCell<Option<OsdBanner>> = const { RefCell::new(None) };
    }

    fn to_wide(s: &str) -> Vec<u16> {
        let mut w: Vec<u16> = s.encode_utf16().collect();
        w.push(0);
        w
    }

    /// Cửa sổ nổi OSD Win32 hiển thị trạng thái VN/EN ngắn hạn.
    pub struct WinOsdWindow {
        hwnd: HWND,
    }

    impl WinOsdWindow {
        /// Tạo cửa sổ OSD ẩn sẵn sàng hiển thị khi chuyển chế độ.
        pub fn new() -> Option<Self> {
            unsafe {
                let hinstance = GetModuleHandleW(core::ptr::null());
                let class_name = to_wide("VNTypeOsdWindow");
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(osd_wnd_proc),
                    hInstance: hinstance,
                    lpszClassName: class_name.as_ptr(),
                    ..core::mem::zeroed()
                };
                RegisterClassW(&wc);

                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let screen_h = GetSystemMetrics(SM_CYSCREEN);
                let x = (screen_w - OSD_WIDTH) / 2;
                let y = screen_h - OSD_HEIGHT - 96;

                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class_name.as_ptr(),
                    to_wide("VNType OSD").as_ptr(),
                    WS_POPUP,
                    x,
                    y,
                    OSD_WIDTH,
                    OSD_HEIGHT,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    hinstance,
                    core::ptr::null(),
                );
                if hwnd.is_null() {
                    return None;
                }
                SetLayeredWindowAttributes(hwnd, 0, 230, LWA_ALPHA);
                Some(Self { hwnd })
            }
        }

        /// Hiển thị thông báo OSD trong `banner.duration_ms` mili-giây.
        pub fn show(&self, banner: OsdBanner) {
            let duration = banner.duration_ms;
            CURRENT_BANNER.with(|cell| {
                *cell.borrow_mut() = Some(banner);
            });
            unsafe {
                KillTimer(self.hwnd, OSD_TIMER_ID);
                SetTimer(self.hwnd, OSD_TIMER_ID, duration, None);
                ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
                windows_sys::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, core::ptr::null(), 1);
            }
        }
    }

    impl Drop for WinOsdWindow {
        fn drop(&mut self) {
            if !self.hwnd.is_null() {
                unsafe {
                    DestroyWindow(self.hwnd);
                }
            }
        }
    }

    unsafe extern "system" fn osd_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TIMER if wparam == OSD_TIMER_ID => {
                unsafe {
                    KillTimer(hwnd, OSD_TIMER_ID);
                    ShowWindow(hwnd, SW_HIDE);
                }
                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = unsafe { core::mem::zeroed() };
                let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
                if !hdc.is_null() {
                    let rect = RECT { left: 0, top: 0, right: OSD_WIDTH, bottom: OSD_HEIGHT };
                    let bg_brush = unsafe { CreateSolidBrush(0x0024_201E) };
                    unsafe {
                        FillRect(hdc, &rect, bg_brush);
                        RoundRect(hdc, 2, 2, OSD_WIDTH - 2, OSD_HEIGHT - 2, 16, 16);
                        SetBkMode(hdc, TRANSPARENT as i32);
                        SetTextColor(hdc, 0x00FF_FFFF);
                    }

                    let text = CURRENT_BANNER.with(|cell| {
                        cell.borrow()
                            .as_ref()
                            .map(|b| format!("{}  •  {}", b.badge, b.title))
                            .unwrap_or_else(|| "VNType".to_string())
                    });
                    let mut wide = to_wide(&text);
                    let font_face = to_wide("Segoe UI");
                    let font = unsafe {
                        CreateFontW(22, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 0, 0, font_face.as_ptr())
                    };
                    let old_font = if !font.is_null() {
                        unsafe { SelectObject(hdc, font as *mut c_void) }
                    } else {
                        core::ptr::null_mut()
                    };
                    let mut draw_rect = rect;
                    unsafe {
                        DrawTextW(
                            hdc,
                            wide.as_mut_ptr(),
                            (wide.len().saturating_sub(1)) as i32,
                            &mut draw_rect,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                        );
                        if !old_font.is_null() {
                            SelectObject(hdc, old_font);
                        }
                        if !font.is_null() {
                            DeleteObject(font as *mut c_void);
                        }
                        DeleteObject(bg_brush as *mut c_void);
                        EndPaint(hwnd, &ps);
                    }
                }
                0
            }
            WM_DESTROY => 0,
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osd_banner_states() {
        assert_eq!(OsdBanner::for_vietnamese_state(true).badge, "VN");
        assert_eq!(OsdBanner::for_vietnamese_state(false).badge, "EN");
    }
}
