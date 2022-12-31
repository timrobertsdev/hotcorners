//! A simple hot corners implementation for Windows 10/11
#![cfg(windows)]
#![windows_subsystem = "windows"]
#![warn(rust_2018_idioms)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod config;

use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use lazy_static::lazy_static;
use windows::{
    core::Result, Win32::Foundation::*, Win32::Graphics::Gdi::*,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
};

use crate::config::Config;

/// How long the cursor must stay within the hot corner to activate, in milliseconds
static mut HOT_DELAY: Duration = Duration::from_millis(100);
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
lazy_static! {
    static ref HOT_CORNER_THREAD: thread::JoinHandle<()> = {
        thread::spawn(|| {
            loop {
                while !HOT_CORNER_THREAD_FLAG.load(Ordering::Acquire) {
                    thread::park();
                }
                hot_corner_fn();
                // FIXME: Lots of double activation without this, but there's probably a better way
                thread::sleep(Duration::from_millis(200));
                HOT_CORNER_THREAD_FLAG.store(false, Ordering::Release);
            }
        })
    };
}

lazy_static! {
    static ref HOT_CORNER_THREAD_FLAG: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

fn main() -> Result<()> {
    let config: Config = toml::from_str(&fs::read_to_string("config.toml").unwrap()).unwrap();

    if let Some(delay) = &config.delay {
        unsafe { HOT_DELAY = Duration::from_millis(*delay) }
    };

    unsafe {
        let mut msg: MSG = MSG::default();
        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_callback), HINSTANCE(0), 0)?;

        RegisterHotKey(
            HWND::default(),
            1,
            EXIT_HOTKEY_MODIFIERS,
            EXIT_HOTKEY.0.into(),
        );

        while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                break;
            }

            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(mouse_hook);
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
    let mut point: POINT = Default::default();
    let sleep_delay = unsafe { HOT_DELAY };

    thread::sleep(sleep_delay);

    unsafe {
        // Grab cursor position
        if !GetCursorPos(&mut point).as_bool() {
            return;
        }

        // Check if cursor is still in the hot corner and then send the input sequence
        if PtInRect(&HOT_CORNER, point).as_bool()
            && SendInput(&HOT_CORNER_INPUT, std::mem::size_of::<INPUT>() as i32)
                != HOT_CORNER_INPUT.len() as u32
        {
            println!("Failed to send input");
        }
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
        if !PtInRect(&HOT_CORNER, (*evt).pt).as_bool() {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, check if it was already hot
        if HOT_CORNER_THREAD_FLAG.load(Ordering::Acquire) {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a mouse button is pressed
        if (GetKeyState(VK_LBUTTON.0 as i32) < 0) || (GetKeyState(VK_RBUTTON.0 as i32) < 0) {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Check if a modifier key is pressed
        let mut keystate = [0u8; 256];
        if GetKeyboardState(&mut keystate).as_bool()
            && (keydown(keystate[VK_SHIFT.0 as usize])
                || keydown(keystate[VK_CONTROL.0 as usize])
                || keydown(keystate[VK_MENU.0 as usize])
                || keydown(keystate[VK_LWIN.0 as usize])
                || keydown(keystate[VK_RWIN.0 as usize]))
        {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // The corner is hot, and was previously cold. Notify the worker thread to resume
        HOT_CORNER_THREAD_FLAG.store(true, Ordering::Release);
        HOT_CORNER_THREAD.thread().unpark();

        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }
}

/// Convenience function for checking if a key is currently pressed down
#[doc(hidden)]
#[inline(always)]
fn keydown(key: u8) -> bool {
    (key & 0x80) != 0
}
