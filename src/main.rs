//! A demo using raw win32 API for window creation and event handling.
//!
//! Also features communication between the webpage and the host.

use once_cell::unsync::OnceCell;
use std::{convert::TryInto, mem};
use std::ptr;
use std::rc::Rc;
use webview2::Controller;
use winapi::{
    shared::minwindef::*, shared::windef::*, um::libloaderapi::*, um::winbase::MulDiv,
    um::wingdi::*, um::winuser::*,
};

fn set_dpi_aware() {
    unsafe {
        // Windows 10.
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr() as *const i8);
        let set_thread_dpi_awareness_context = GetProcAddress(
            user32,
            b"SetThreadDpiAwarenessContext\0".as_ptr() as *const i8,
        );
        if !set_thread_dpi_awareness_context.is_null() {
            let set_thread_dpi_awareness_context: extern "system" fn(
                DPI_AWARENESS_CONTEXT,
            )
                -> DPI_AWARENESS_CONTEXT = mem::transmute(set_thread_dpi_awareness_context);
            set_thread_dpi_awareness_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            return;
        }
        // Windows 7.
        SetProcessDPIAware();
    }
}

fn main() {
    let width = 600;
    let height = 400;

    set_dpi_aware();

    let controller = Rc::new(OnceCell::<Controller>::new());
    let controller_clone = controller.clone();

    // Window procedure.
    let wnd_proc = move |hwnd, msg, w_param, l_param| match msg {
        WM_SIZE => {
            if let Some(c) = controller.get() {
                let mut r = unsafe { mem::zeroed() };
                unsafe {
                    GetClientRect(hwnd, &mut r);
                }
                c.put_bounds(r).unwrap();
            }
            0
        }
        WM_MOVE => {
            if let Some(c) = controller.get() {
                let _ = c.notify_parent_window_position_changed();
            }
            0
        }
        // Optimization: don't render the webview when the window is minimized.
        WM_SYSCOMMAND if w_param == SC_MINIMIZE => {
            if let Some(c) = controller.get() {
                c.put_is_visible(false).unwrap();
            }
            unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) }
        }
        WM_SYSCOMMAND if w_param == SC_RESTORE => {
            if let Some(c) = controller.get() {
                c.put_is_visible(true).unwrap();
            }
            unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) }
        }
        // High DPI support.
        WM_DPICHANGED => unsafe {
            let rect = *(l_param as *const RECT);
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            0
        },
        _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
    };

    // Child Window procedure.
    let child_wnd_proc = move |hwnd, msg, w_param, l_param| match msg {
        // WM_NCHITTEST => {
        //     unsafe {
        //         let res = SendMessageA(GetParent(hwnd), msg, w_param, l_param);
        //         return HTTRANSPARENT
        //     }
        // }

        WM_LBUTTONDOWN=>  {
            unsafe {
                SendMessageA(GetParent(hwnd), WM_NCLBUTTONDOWN, HTCAPTION.try_into().unwrap(), 0);
            }
            return 0;
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
    };

    // Register window class. (Standard windows GUI boilerplate).
    let class_name = utf_16_null_terminiated("WebView2 Win32 Class");
    let h_instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
        lpfnWndProc: Some(unsafe { wnd_proc_helper::as_global_wnd_proc(wnd_proc) }),
        lpszClassName: class_name.as_ptr(),
        hInstance: h_instance,
        hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
        ..unsafe { mem::zeroed() }
    };
    unsafe {
        if RegisterClassW(&class) == 0 {
            message_box(
                ptr::null_mut(),
                &format!("RegisterClassW failed: {}", std::io::Error::last_os_error()),
                "Error",
                MB_ICONERROR | MB_OK,
            );
            return;
        }
    }

    //Register caption window class. (Standard windows GUI boilerplate).
    let child_class_name = utf_16_null_terminiated("Caption Win32 Class");
    unsafe {
        let child_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
            lpfnWndProc: Some(unsafe { wnd_proc_helper::as_global_child_wnd_proc(child_wnd_proc) }),
            lpszClassName: child_class_name.as_ptr(),
            hInstance: h_instance,
            //hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            hbrBackground: GetStockObject(NULL_BRUSH.try_into().unwrap()) as HBRUSH,
            ..unsafe { mem::zeroed() }
        };
        if RegisterClassW(&child_class) == 0 {
            message_box(
                ptr::null_mut(),
                &format!("RegisterClassW failed: {}", std::io::Error::last_os_error()),
                "Error",
                MB_ICONERROR | MB_OK,
            );
            return;
        }
    }

    // Create window. (Standard windows GUI boilerplate).
    let window_title = utf_16_null_terminiated("WebView2 - Win 32");
    let hdc = unsafe { GetDC(ptr::null_mut()) };
    let dpi = unsafe { GetDeviceCaps(hdc, LOGPIXELSX) };
    unsafe { ReleaseDC(ptr::null_mut(), hdc) };
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            MulDiv(width, dpi, USER_DEFAULT_SCREEN_DPI),
            MulDiv(height, dpi, USER_DEFAULT_SCREEN_DPI),
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        message_box(
            ptr::null_mut(),
            &format!(
                "CreateWindowExW failed: {}",
                std::io::Error::last_os_error()
            ),
            "Error",
            MB_ICONERROR | MB_OK,
        );
        return;
    }
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    // Create the webview.
    let r = webview2::Environment::builder().build(move |env| {
        env.unwrap().create_controller(hwnd, move |c| {
            let c = c.unwrap();

            

            let mut r = unsafe { mem::zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut r);
            }
            c.put_bounds(r).unwrap();

            let w = c.get_webview().unwrap();
            w.add_script_to_execute_on_document_created(r"document.addEventListener('mousedown', evt => {
                const { target } = evt;
                const appRegion = getComputedStyle(target)['-webkit-app-region'];
        
                //if (appRegion === 'drag') {
                    //chrome.webview.hostObjects.sync.eventForwarder.MouseDownDrag();
                    window.chrome.webview.postMessage('drag');
                    evt.preventDefault();
                    evt.stopPropagation();
                //}
            });
            
            
            // document.addEventListener('mousedown', function (event)
            // {
            //     let jsonObject =
            //     {
            //         Key: 'mousedown',
            //         Value:
            //         {
            //             X: event.screenX,
            //             Y: event.screenY
            //         }
            //     };
            //    window.chrome.webview.postMessage(JSON.stringify(jsonObject));
            //}
            ", |a|Ok(())).unwrap();
            //w.navigate("https://caesar2go.caseris.de/web/timio").unwrap();


            
            // Communication.
            w.navigate_to_string(r##"
<!doctype html>
<html>
<head>
<title>Demo</title>
<style>
    h2 {
        --webkit-app-region: drag;
    }
</style>
</head>
<body>
    <h1> Die Überschrift</h1>
    <h2> Hier Fenster ziehen</h2>
    <p>Das ist der Inhalt</p>

    <script>
    console.log('Affe')
    document.body.addEventListener('mousedown', evt => {
        const { target } = evt;
        const appRegion = getComputedStyle(target)['-webkit-app-region'];

        if (appRegion === 'drag') {
            //chrome.webview.hostObjects.sync.eventForwarder.MouseDownDrag();
            window.chrome.webview.postMessage('drag');
            evt.preventDefault();
            evt.stopPropagation();
        }
    });    
    </script>
</body>
</html>
"##).unwrap();
//             // Receive message from webpage.
            let affe = hwnd.clone();
            w.add_web_message_received(move |w, msg| {
                // let msg = msg.try_get_web_message_as_string()?;
                // // Send it back.
                // let h = msg.to_string();
                unsafe {
                    PostMessageW(affe, WM_NCLBUTTONDOWN, 2, 0);                
                }
                Ok(())
                //w.post_web_message_as_string(&msg)
            }).unwrap();
            controller_clone.set(c).unwrap();
            Ok(())
        })
    });
    if let Err(e) = r {
        message_box(
            ptr::null_mut(),
            &format!("Creating WebView2 Environment failed: {}\n", e),
            "Error",
            MB_ICONERROR | MB_OK,
        );
        return;
    }

    let child_hwnd = unsafe {
        CreateWindowExW(
            0,
            child_class_name.as_ptr(),
            window_title.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            200,
            50,
            hwnd,
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        )
    };


    // Message loop. (Standard windows GUI boilerplate).
    let mut msg: MSG = unsafe { mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn utf_16_null_terminiated(x: &str) -> Vec<u16> {
    x.encode_utf16().chain(std::iter::once(0)).collect()
}

fn message_box(hwnd: HWND, text: &str, caption: &str, _type: u32) -> i32 {
    let text = utf_16_null_terminiated(text);
    let caption = utf_16_null_terminiated(caption);

    unsafe { MessageBoxW(hwnd, text.as_ptr(), caption.as_ptr(), _type) }
}

mod wnd_proc_helper {
    use super::*;
    use std::cell::UnsafeCell;

    struct UnsafeSyncCell<T> {
        inner: UnsafeCell<T>,
    }

    impl<T> UnsafeSyncCell<T> {
        const fn new(t: T) -> UnsafeSyncCell<T> {
            UnsafeSyncCell {
                inner: UnsafeCell::new(t),
            }
        }
    }

    impl<T: Copy> UnsafeSyncCell<T> {
        unsafe fn get(&self) -> T {
            self.inner.get().read()
        }

        unsafe fn set(&self, v: T) {
            self.inner.get().write(v)
        }
    }

    unsafe impl<T: Copy> Sync for UnsafeSyncCell<T> {}

    static GLOBAL_F: UnsafeSyncCell<usize> = UnsafeSyncCell::new(0);
    static GLOBAL_F_CHILD: UnsafeSyncCell<usize> = UnsafeSyncCell::new(0);

    /// Use a closure as window procedure.
    ///
    /// The closure will be boxed and stored in a global variable. It will be
    /// released upon WM_DESTROY. (It doesn't get to handle WM_DESTROY.)
    pub unsafe fn as_global_wnd_proc<F: Fn(HWND, UINT, WPARAM, LPARAM) -> isize + 'static>(
        f: F,
    ) -> unsafe extern "system" fn(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> isize
    {
        let f_ptr = Box::into_raw(Box::new(f));
        GLOBAL_F.set(f_ptr as usize);

        unsafe extern "system" fn wnd_proc<F: Fn(HWND, UINT, WPARAM, LPARAM) -> isize + 'static>(
            hwnd: HWND,
            msg: UINT,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> isize {
            let f_ptr = GLOBAL_F.get() as *mut F;

            if msg == WM_DESTROY {
                Box::from_raw(f_ptr);
                GLOBAL_F.set(0);
                PostQuitMessage(0);
                return 0;
            }

            if !f_ptr.is_null() {
                let f = &*f_ptr;

                f(hwnd, msg, w_param, l_param)
            } else {
                DefWindowProcW(hwnd, msg, w_param, l_param)
            }
        }

        wnd_proc::<F>
    }

    pub unsafe fn as_global_child_wnd_proc<F: Fn(HWND, UINT, WPARAM, LPARAM) -> isize + 'static>(
        f: F,
    ) -> unsafe extern "system" fn(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> isize
    {
        let f_ptr = Box::into_raw(Box::new(f));
        GLOBAL_F_CHILD.set(f_ptr as usize);

        unsafe extern "system" fn wnd_proc<F: Fn(HWND, UINT, WPARAM, LPARAM) -> isize + 'static>(
            hwnd: HWND,
            msg: UINT,
            w_param: WPARAM,
            l_param: LPARAM,
        ) -> isize {
            let f_ptr = GLOBAL_F.get() as *mut F;

            if msg == WM_DESTROY {
                Box::from_raw(f_ptr);
                GLOBAL_F.set(0);
                PostQuitMessage(0);
                return 0;
            }

            if !f_ptr.is_null() {
                let f = &*f_ptr;

                f(hwnd, msg, w_param, l_param)
            } else {
                DefWindowProcW(hwnd, msg, w_param, l_param)
            }
        }

        wnd_proc::<F>
    }}