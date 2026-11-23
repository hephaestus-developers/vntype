//! Cơ chế đồng bộ liên tiến trình (IPC) nguyên tử 64-bit giữa Windows TSF và trình nền Hook (`vntype-win-ipc`).
//!
//! Trên Windows, TSF Text Service (`vntype_tsf.dll`) được nạp trực tiếp vào không gian địa chỉ
//! của từng ứng dụng (Notepad, Chrome, Word), trong khi một số cửa sổ như Windows Terminal hoặc
//! trò chơi không nạp TSF chưa ký WHQL và cần tiến trình `vntype.exe` (Hook `WH_KEYBOARD_LL`) xử lý.
//!
//! Để khắc phục lỗi đọc lệch 64-bit (torn read trên x86) của thiết kế cũ, cấu trúc [`SharedRoutingBlock`]
//! chỉ sử dụng các nguyên thuỷ nguyên tử [`AtomicU32`] và [`AtomicU64`] với thứ tự bộ nhớ `AcqRel`.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Tên đối tượng vùng nhớ dùng chung trong không gian phiên người dùng Windows.
pub const SHARED_MEMORY_NAME: &str = "Local\\VNTypeTsfRoutingV2";

/// Mã nhận diện hợp lệ của khối vùng nhớ dùng chung (`'VNTP'`).
pub const ROUTING_MAGIC: u32 = 0x564E_5450;

/// Phiên bản giao thức định tuyến hiện hành.
pub const ROUTING_VERSION: u32 = 2;

/// Khối dữ liệu nguyên tử nằm trong vùng nhớ dùng chung giữa TSF DLL và Hook Daemon.
#[repr(C, align(8))]
pub struct SharedRoutingBlock {
    /// Mã nhận diện [`ROUTING_MAGIC`].
    pub magic: AtomicU32,
    /// Phiên bản giao thức [`ROUTING_VERSION`].
    pub version: AtomicU32,
    /// Định danh tiến trình (PID) đang được TSF kiểm soát hoạt động (`0` nếu không có).
    pub active_pid: AtomicU32,
    /// Cờ trạng thái: `1` nếu TSF đang sở hữu ngữ cảnh nhập liệu, `0` nếu nhường cho Hook.
    pub tsf_active: AtomicU32,
    /// Tay cầm cửa sổ (`HWND`) 64-bit đang được TSF gắn kết, truy cập qua [`AtomicU64`] chống đọc lệch.
    pub active_hwnd: AtomicU64,
    /// Số thứ tự thế hệ cấu hình (tăng lên mỗi khi người dùng lưu cấu hình từ giao diện Settings).
    pub config_generation: AtomicU64,
}

impl Default for SharedRoutingBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedRoutingBlock {
    /// Khởi tạo một khối trạng thái định tuyến mới ở trạng thái nghỉ.
    pub const fn new() -> Self {
        SharedRoutingBlock {
            magic: AtomicU32::new(ROUTING_MAGIC),
            version: AtomicU32::new(ROUTING_VERSION),
            active_pid: AtomicU32::new(0),
            tsf_active: AtomicU32::new(0),
            active_hwnd: AtomicU64::new(0),
            config_generation: AtomicU64::new(1),
        }
    }

    /// Kiểm tra khối vùng nhớ có hợp lệ và đúng phiên bản hay không.
    pub fn is_valid(&self) -> bool {
        self.magic.load(Ordering::Acquire) == ROUTING_MAGIC
            && self.version.load(Ordering::Acquire) == ROUTING_VERSION
    }

    /// Được gọi bởi `vntype-tsf` khi một ngữ cảnh TSF nhận tiêu điểm trong tiến trình `pid` và cửa sổ `hwnd`.
    pub fn publish_tsf_active(&self, pid: u32, hwnd: u64) {
        self.active_hwnd.store(hwnd, Ordering::Release);
        self.active_pid.store(pid, Ordering::Release);
        self.tsf_active.store(1, Ordering::Release);
    }

    /// Được gọi bởi `vntype-tsf` khi mất tiêu điểm.
    ///
    /// Chỉ xoá trạng thái nếu `active_pid` hiện tại vẫn trùng với `caller_pid`, ngăn chặn lỗi
    /// tiến trình cũ mất tiêu điểm trễ ghi đè lên tiến trình mới vừa nhận tiêu điểm.
    pub fn clear_tsf_active(&self, caller_pid: u32) {
        if self.active_pid.load(Ordering::Acquire) == caller_pid {
            self.tsf_active.store(0, Ordering::Release);
            self.active_pid.store(0, Ordering::Release);
            self.active_hwnd.store(0, Ordering::Release);
        }
    }

    /// Được gọi bởi `vntype-hook` để kiểm tra xem tiến trình `foreground_pid` có đang được TSF xử lý không.
    pub fn should_hook_bypass(&self, foreground_pid: u32) -> bool {
        if !self.is_valid() {
            return false;
        }
        self.tsf_active.load(Ordering::Acquire) == 1
            && self.active_pid.load(Ordering::Acquire) == foreground_pid
    }

    /// Tăng bộ đếm thế hệ cấu hình để thông báo cho mọi tiến trình TSF và Hook nạp lại cấu hình.
    pub fn notify_config_reloaded(&self) -> u64 {
        self.config_generation.fetch_add(1, Ordering::AcqRel) + 1
    }
}

#[cfg(windows)]
pub mod win_shm {
    use core::ffi::c_void;
    use core::ops::Deref;
    use core::ptr::NonNull;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READWRITE,
        UnmapViewOfFile,
    };

    use super::{SHARED_MEMORY_NAME, SharedRoutingBlock};

    /// Ánh xạ vùng nhớ dùng chung Win32 (`CreateFileMappingW` + `MapViewOfFile`) cho [`SharedRoutingBlock`].
    pub struct SharedRoutingMapping {
        handle: HANDLE,
        view: NonNull<SharedRoutingBlock>,
    }

    unsafe impl Send for SharedRoutingMapping {}
    unsafe impl Sync for SharedRoutingMapping {}

    impl SharedRoutingMapping {
        /// Mở hoặc tạo mới vùng nhớ dùng chung [`SHARED_MEMORY_NAME`].
        pub fn open_or_create() -> Result<Self, u32> {
            let mut wide_name: Vec<u16> = SHARED_MEMORY_NAME.encode_utf16().collect();
            wide_name.push(0);

            let size = core::mem::size_of::<SharedRoutingBlock>() as u32;
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    core::ptr::null(),
                    PAGE_READWRITE,
                    0,
                    size,
                    wide_name.as_ptr(),
                )
            };
            if handle.is_null() {
                return Err(unsafe { GetLastError() });
            }

            let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            let mapped = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size as usize) };
            let ptr = mapped.Value as *mut SharedRoutingBlock;
            let Some(view) = NonNull::new(ptr) else {
                let err = unsafe { GetLastError() };
                unsafe { CloseHandle(handle) };
                return Err(err);
            };

            let block = unsafe { view.as_ref() };
            if !already_exists || !block.is_valid() {
                unsafe {
                    ptr.write(SharedRoutingBlock::new());
                }
            }

            Ok(Self { handle, view })
        }
    }

    impl Deref for SharedRoutingMapping {
        type Target = SharedRoutingBlock;

        fn deref(&self) -> &Self::Target {
            unsafe { self.view.as_ref() }
        }
    }

    impl Drop for SharedRoutingMapping {
        fn drop(&mut self) {
            unsafe {
                let addr = MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view.as_ptr() as *mut c_void };
                let _ = UnmapViewOfFile(addr);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_routing_handoff_prevents_stale_clear() {
        let block = SharedRoutingBlock::new();
        assert!(!block.should_hook_bypass(100));

        // Tiến trình 100 (Notepad) kích hoạt TSF
        block.publish_tsf_active(100, 0xABCD_1234);
        assert!(block.should_hook_bypass(100));
        assert!(!block.should_hook_bypass(200));

        // Người dùng Alt+Tab sang tiến trình 200 (Chrome), Chrome kích hoạt trước khi Notepad kịp huỷ
        block.publish_tsf_active(200, 0xEEEE_9999);
        // Notepad bây giờ mới gọi clear_tsf_active(100) -> không được làm mất trạng thái của Chrome (200)
        block.clear_tsf_active(100);
        assert!(block.should_hook_bypass(200));

        // Chrome mất tiêu điểm
        block.clear_tsf_active(200);
        assert!(!block.should_hook_bypass(200));
        assert_eq!(block.notify_config_reloaded(), 2);
    }
}
