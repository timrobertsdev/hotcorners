fn main() {
    windows::build! {
        Windows::Win32::Foundation::RECT,
        Windows::Win32::Graphics::Gdi::PtInRect,
        Windows::Win32::UI::KeyboardAndMouseInput::{
            GetKeyboardState,
            GetKeyState,
            RegisterHotKey,
            SendInput,
            HOT_KEY_MODIFIERS,
            INPUT,
            VIRTUAL_KEY,
        },
        Windows::Win32::UI::WindowsAndMessaging::{
            CallNextHookEx,
            DispatchMessageW,
            GetMessageW,
            GetCursorPos,
            SetWindowsHookExW,
            UnhookWindowsHookEx,
            MSG,
            MSLLHOOKSTRUCT,
            WM_HOTKEY,
            WM_MOUSEMOVE,
        },
    };
}
