use std::ffi::c_void;

use windows::{
    runtime::*, Win32::Foundation::*, Win32::Graphics::Gdi::*, Win32::System::Threading::*,
    Win32::UI::KeyboardAndMouseInput::*, Win32::UI::WindowsAndMessaging::*,
};

/// How long the cursor must stay within the hot corner to activate, in milliseconds
const HOT_DELAY: u32 = 100;
/// Base key for exiting
const EXIT_HOTKEY: VIRTUAL_KEY = VK_C;
/// Modifier key(s) for exiting
const EXIT_HOTKEY_MODIFIERS: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);

/// Rectangle to define our hot corner
const HOT_CORNER: RECT = RECT {
    left: -20,
    top: -20,
    right: 20,
    bottom: 20,
};

/// Input sequence to send when the hot corner is activated
const HOT_CORNER_INPUT: [INPUT; 4] = [
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    },
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_TAB,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    },
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_TAB,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    },
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_LWIN,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    },
];

// Global handle to the activation thread
static mut HOT_CORNER_THREAD: HANDLE = HANDLE(0);

fn main() -> Result<()> {
    unsafe {
        let mut msg: MSG = MSG::default();
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_callback), HINSTANCE(0), 0);

        if mouse_hook.is_invalid() {
            return Err(windows::runtime::Error::fast_error(HRESULT(1)));
        }

        RegisterHotKey(
            HWND::default(),
            1,
            EXIT_HOTKEY_MODIFIERS,
            EXIT_HOTKEY.0.into(),
        );

        while GetMessageW(&mut msg as *mut _, HWND(0), 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                break;
            }

            DispatchMessageW(&mut msg as *mut _);
        }

        UnhookWindowsHookEx(mouse_hook);
    }

    Ok(())
}

/// This thread runs when the cursor enters the hot corner, and waits to see if it stays there.
/// Runs in a separate thread when the cursor enters the hot corner, and waits to see if it stays there.
/// Will send `HOT_CORNER_INPUT` if the cursor stays within the rectangle defined by `HOT_CORNER` for 
/// `HOT_DELAY` milliseconds.
/// 
/// Note: we've already checked that no modifier keys or mouse buttons are currently pressed in
/// `mouse_hook_callback`.
extern "system" fn hot_corner_fn(_lp_parameter: *mut c_void) -> u32 {
    println!("In hot_corner_fn");
    // let mut keystate = [0u8; 256];
    let mut point: POINT = Default::default();

    unsafe {
        Sleep(HOT_DELAY);

        // Grab cursor position
        if !GetCursorPos(&mut point as *mut POINT).as_bool() {
            return 1;
        }
        
        // Check if cursor is still in the hot corner and then send the input sequence
        if PtInRect(&HOT_CORNER as *const _, &point).as_bool()
            && SendInput(
                HOT_CORNER_INPUT.len() as u32,
                &HOT_CORNER_INPUT as *const _,
                std::mem::size_of::<INPUT>() as i32,
            ) != HOT_CORNER_INPUT.len() as u32
        {
            return 1;
        }
        println!("sent input");

        0
    }
}

/// Callback that is registered with Windows in order to start the hot corner activation
#[allow(unused_assignments)] // Clippy doesn't like that we sometimes don't read `hot_corner_thread`'s value
extern "system" fn mouse_hook_callback(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
        let evt: *mut MSLLHOOKSTRUCT = std::mem::transmute(l_param);

        // If the mouse hasn't moved, we're done
        if w_param.0 as u32 != WM_MOUSEMOVE {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if the cursor is hot or cold
        if !PtInRect(&HOT_CORNER as *const _, (*evt).pt).as_bool() {
            // The corner is cold, and was cold before
            if HOT_CORNER_THREAD.is_invalid() {
                return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
            }

            // The corner is cold, but was previously hot
            TerminateThread(HOT_CORNER_THREAD, 0);

            CloseHandle(HOT_CORNER_THREAD);

            // Reset state
            HOT_CORNER_THREAD = HANDLE::default();

            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, check if it was already hot
        if !HOT_CORNER_THREAD.is_invalid() {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a mouse button is pressed
        if (GetKeyState(VK_LBUTTON.0 as i32) < 0) || (GetKeyState(VK_RBUTTON.0 as i32) < 0) {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a modifier key is pressed
        let mut keystate = [0u8; 256];
        if GetKeyboardState(keystate.as_mut_ptr()).as_bool()
            && (keydown(keystate[VK_SHIFT.0 as usize])
                || keydown(keystate[VK_CONTROL.0 as usize])
                || keydown(keystate[VK_MENU.0 as usize])
                || keydown(keystate[VK_LWIN.0 as usize])
                || keydown(keystate[VK_RWIN.0 as usize]))
        {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, and was previously cold. Start a new thread to monitor
        HOT_CORNER_THREAD = CreateThread(
            std::ptr::null(),
            0,
            Some(hot_corner_fn),
            std::ptr::null(),
            THREAD_CREATION_FLAGS(0),
            std::ptr::null_mut(),
        );

        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }
}

/// Convenience function for checking if a key is currently pressed down
#[doc(hidden)]
#[inline(always)]
fn keydown(key: u8) -> bool {
    (key & 0x80) != 0
}
