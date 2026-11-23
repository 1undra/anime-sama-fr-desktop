#![cfg(windows)]

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::CoTaskMemAlloc;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

const AUMID: &str = "com.anime-sama.desktop";

pub fn set_process_app_user_model_id() {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(AUMID));
    }
}

pub fn set_window_app_user_model_id(hwnd: HWND) {
    unsafe {
        let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) else {
            return;
        };
        let wide: Vec<u16> = AUMID.encode_utf16().chain(Some(0)).collect();
        let buffer = CoTaskMemAlloc(wide.len() * 2) as *mut u16;
        if buffer.is_null() {
            return;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), buffer, wide.len());
        let mut value = PROPVARIANT::default();
        let inner = &mut *value.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = PWSTR(buffer);
        let _ = store.SetValue(&PKEY_AppUserModel_ID, &value);
        let _ = PropVariantClear(&mut value);
    }
}
