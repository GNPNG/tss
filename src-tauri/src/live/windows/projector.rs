//! Audience window belongs to the render thread. Its surface drops before HWND.
use super::*;
use tss_live_wheel::{ProjectorOutput, ProjectorTiming};
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
unsafe extern "system" fn display_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 1);
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_KEYDOWN if wp == 27 => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            ValidateRect(hwnd, std::ptr::null());
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
struct Displays {
    editor: HMONITOR,
    external: Option<RECT>,
}
unsafe extern "system" fn monitor(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let out = &mut *(data as *mut Displays);
    if m != out.editor && out.external.is_none() {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(m, &mut info) != 0 {
            out.external = Some(info.rcMonitor);
        }
    }
    1
}
pub(super) struct Projector {
    output: ProjectorOutput,
    surface: wgpu::Surface<'static>,
    window: Child,
}
impl Projector {
    pub(super) fn open(
        parent: HWND,
        instance: &wgpu::Instance,
        r: &LiveWheel,
        delay: u32,
    ) -> Result<Self, String> {
        unsafe {
            let name = wide("TssNativeProgramProjection");
            let title = wide("The Sacred Swirls — Projector");
            let module = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.hInstance = module;
            wc.lpfnWndProc = Some(display_proc);
            wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
            wc.lpszClassName = name.as_ptr();
            if RegisterClassW(&wc) == 0
                && std::io::Error::last_os_error().raw_os_error() != Some(1410)
            {
                return Err(format!(
                    "Register projector window: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let mut monitors = Displays {
                editor: MonitorFromWindow(parent, MONITOR_DEFAULTTONEAREST),
                external: None,
            };
            if EnumDisplayMonitors(
                null_mut(),
                std::ptr::null(),
                Some(monitor),
                &mut monitors as *mut _ as LPARAM,
            ) == 0
            {
                return Err(format!(
                    "Enumerate projector displays: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let (style, x, y, w, h) = if let Some(rect) = monitors.external {
                (
                    WS_POPUP,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                )
            } else {
                (WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 960, 580)
            };
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                null_mut(),
                null_mut(),
                module,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return Err(format!(
                    "Create projector window: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let window = Child(hwnd);
            let raw = Win32WindowHandle::new(
                NonZeroIsize::new(hwnd as isize).ok_or("Null projector HWND")?,
            );
            let surface = instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    raw_window_handle: RawWindowHandle::Win32(raw),
                })
                .map_err(|e| e.to_string())?;
            let output = ProjectorOutput::new(r, &surface, client(hwnd), delay)?;
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            Ok(Self {
                output,
                surface,
                window,
            })
        }
    }
    pub(super) fn delay(&mut self, r: &LiveWheel, ms: u32) -> Result<(), String> {
        self.output.set_delay(r, ms)
    }
    pub(super) fn reset(&mut self) {
        self.output.reset();
    }
    pub(super) fn hwnd(&self) -> HWND {
        self.window.0
    }
    pub(super) fn visible(&self) -> bool {
        unsafe {
            IsIconic(self.window.0) == 0
                && IsWindowVisible(self.window.0) != 0
                && !client(self.window.0).contains(&0)
        }
    }
    pub(super) fn closed(&self) -> bool {
        unsafe {
            GetWindowLongPtrW(self.window.0, GWLP_USERDATA) != 0 || IsWindow(self.window.0) == 0
        }
    }
    pub(super) fn tick(
        &mut self,
        r: &LiveWheel,
        now: f64,
        song: f64,
        capture: bool,
    ) -> Result<Option<ProjectorTiming>, String> {
        self.output
            .present(r, &self.surface, client(self.window.0), now, song, capture)
    }
}
fn client(hwnd: HWND) -> [u32; 2] {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        [
            (rect.right - rect.left).max(0) as u32,
            (rect.bottom - rect.top).max(0) as u32,
        ]
    }
}
impl Host {
    pub async fn projector(&self, enabled: bool, delay: u32) -> Result<(), String> {
        if delay > 1000 {
            return Err("Projector delay must be 0–1000 ms".into());
        }
        if !enabled && self.tx.lock().map_err(|e| e.to_string())?.is_none() {
            return Ok(());
        }
        self.request(|reply| Command::Projector(enabled, delay, reply))
            .await
    }
}
