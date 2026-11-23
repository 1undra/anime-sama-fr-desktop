#![cfg(windows)]

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::time::Duration;
use windows::core::{HSTRING, Interface};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    eRender, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2,
    IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

pub fn spawn_audio_session_fixer(exe_path: String) {
    std::thread::spawn(move || loop {
        rename_sessions(&exe_path);
        std::thread::sleep(Duration::from_millis(100));
    });
}

fn rename_sessions(exe_path: &str) {
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = (|| -> windows::core::Result<()> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let devices: IMMDeviceCollection =
                enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            for i in 0..devices.GetCount()? {
                let device = devices.Item(i)?;
                let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
                let sessions: IAudioSessionEnumerator = manager.GetSessionEnumerator()?;
                for j in 0..sessions.GetCount()? {
                    let control = sessions.GetSession(j)?;
                    let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                        continue;
                    };
                    let Ok(pid) = control2.GetProcessId() else {
                        continue;
                    };
                    if !is_our_webview2(pid) {
                        continue;
                    }
                    let _ = control2.SetDisplayName(&HSTRING::from("Anime-Sama"), std::ptr::null());
                    let icon = HSTRING::from(format!("{},0", exe_path));
                    let _ = control2.SetIconPath(&icon, std::ptr::null());
                }
            }
            Ok(())
        })();
        let _ = result;
        if com {
            CoUninitialize();
        }
    }
}

fn is_our_webview2(pid: u32) -> bool {
    let processes = process_tree();
    let Some((_, name)) = processes.get(&pid) else {
        return false;
    };
    if !name.starts_with("msedgewebview2") {
        return false;
    }
    let host = std::process::id();
    let mut current = pid;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let Some((parent, _)) = processes.get(&current) else {
            return false;
        };
        if *parent == host {
            return true;
        }
        current = *parent;
    }
    false
}

fn process_tree() -> HashMap<u32, (u32, String)> {
    let mut processes = HashMap::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return processes;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = OsString::from_wide(&entry.szExeFile)
                    .to_string_lossy()
                    .to_ascii_lowercase();
                processes.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    processes
}
