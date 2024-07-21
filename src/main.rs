//! A simple hot corners implementation for Windows 10/11
#![cfg(windows)]
#![windows_subsystem = "windows"]

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use windows::{
    core::Result,
    Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Win32::Graphics::Gdi::PtInRect,
    Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, GetKeyboardState, RegisterHotKey, SendInput, HOT_KEY_MODIFIERS, INPUT,
        INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MOD_ALT,
        MOD_CONTROL, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RBUTTON,
        VK_RWIN, VK_SHIFT, VK_TAB,
    },
    Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        HHOOK, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_HOTKEY, WM_MOUSEMOVE,
    },
};

/// How long the cursor must stay within the hot corner to activate, in milliseconds
const HOT_DELAY: Duration = Duration::from_millis(100);
/// Base key for exiting
const EXIT_HOTKEY: VIRTUAL_KEY = VK_C;
/// Modifier key(s) for exiting
const EXIT_HOTKEY_MODIFIERS: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);

/// Rectangle to define our hot corner
const HOT_CORNER: RECT = RECT {
    // fixes the activation issue when the mouse tries to go through the top left corner
    left: -200,
    top: -200,
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
static HOT_CORNER_THREAD: OnceLock<JoinHandle<()>> = OnceLock::new();

static HOT_CORNER_THREAD_FLAG: OnceLock<AtomicBool> = OnceLock::new();

fn main() -> Result<()> {
    // init statics
    HOT_CORNER_THREAD_FLAG
        .set(AtomicBool::new(false))
        .unwrap();

    HOT_CORNER_THREAD
        .set(thread::spawn(|| {
            let flag = HOT_CORNER_THREAD_FLAG.get().unwrap();
            loop {
                while !flag.load(Ordering::Acquire) {
                    thread::park();
                }
                hot_corner_fn();
                flag.store(false, Ordering::Release);
            }
        }))
        .unwrap();

    unsafe {
        let mut msg: MSG = MSG::default();
        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_callback), HINSTANCE::default(), 0)?;

        RegisterHotKey(
            HWND::default(),
            1,
            EXIT_HOTKEY_MODIFIERS,
            EXIT_HOTKEY.0.into(),
        )?;

        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                break;
            }

            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(mouse_hook)?;
    }

    Ok(())
}

/// Runs in a separate thread when the cursor enters the hot corner, and waits to see if it stays there.
/// Will send `HOT_CORNER_INPUT` if the cursor stays within the rectangle defined by `HOT_CORNER` for
/// `HOT_DELAY` milliseconds.
///
/// Note: we've already checked that no modifier keys or mouse buttons are currently pressed in
/// `mouse_hook_callback`.
fn hot_corner_fn() {
    thread::sleep(HOT_DELAY);

    unsafe {
        // `size_of::<INPUT>()` will never > i32::MAX
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        if SendInput(&HOT_CORNER_INPUT, std::mem::size_of::<INPUT>() as i32)
                // it would be absurd if the size of `HOT_CORNER_INPUT`` exceeded `u32::MAX`
                != HOT_CORNER_INPUT.len() as u32
        {
            println!("Failed to send input");
        }
    }
}

static mut STILL_HOT: bool = false;

/// Callback that is registered with Windows in order to start the hot corner activation
#[allow(unused_assignments)] // Clippy doesn't like that we sometimes don't read `hot_corner_thread`'s value
extern "system" fn mouse_hook_callback(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
        let evt = l_param.0 as *mut MSLLHOOKSTRUCT;
        let flag = HOT_CORNER_THREAD_FLAG.get().unwrap();

        // If the mouse hasn't moved, we're done
        let wm_evt = u32::try_from(w_param.0).expect("w_param.0 fits in a u32");
        if wm_evt != WM_MOUSEMOVE {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if the cursor is hot or cold
        if !PtInRect(&HOT_CORNER, (*evt).pt).as_bool() {
            STILL_HOT = false;
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, check if it was already hot
        if STILL_HOT {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a mouse button is pressed
        if (GetKeyState(i32::from(VK_LBUTTON.0)) < 0) || (GetKeyState(i32::from(VK_RBUTTON.0)) < 0)
        {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a modifier key is pressed
        let mut keystate = [0u8; 256];
        if GetKeyboardState(&mut keystate).is_ok()
            && (keydown(keystate[VK_SHIFT.0 as usize])
                || keydown(keystate[VK_CONTROL.0 as usize])
                || keydown(keystate[VK_MENU.0 as usize])
                || keydown(keystate[VK_LWIN.0 as usize])
                || keydown(keystate[VK_RWIN.0 as usize]))
        {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, and was previously cold. Notify the worker thread to resume
        flag.store(true, Ordering::Relaxed);
        HOT_CORNER_THREAD.get().unwrap().thread().unpark();

        STILL_HOT = true;
        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }
}

/// Convenience function for checking if a key is currently pressed down
#[doc(hidden)]
fn keydown(key: u8) -> bool {
    (key & 0x80) != 0
}
