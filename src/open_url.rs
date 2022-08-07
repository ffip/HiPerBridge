// pub fn open_url(url: &str) -> u32 {
//     use windows::core::*;
//     use windows::Win32::System::Threading::GetProcessId;
//     use windows::Win32::Foundation::GetLastError;
//     use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
//     use windows::Win32::UI::Shell::*;
//     let url: Vec<u16> = dbg!(url).encode_utf16().chain(Some(0)).collect();
//     let mut info: SHELLEXECUTEINFOW = SHELLEXECUTEINFOW {
//         lpVerb: w!("open").into(),
//         lpFile: dbg!(PCWSTR::from_raw(url.as_ptr())),
//         nShow: SW_SHOW.0 as i32,
//         fMask: SEE_MASK_NOCLOSEPROCESS,
//         ..Default::default()
//     };
//     let result = dbg!(unsafe { ShellExecuteExW(&mut info) });
//     if result.as_bool() {
//         unsafe { GetProcessId(info.hProcess) }
//     } else {
//         unsafe {
//             dbg!(GetLastError());
//         }
//         0
//     }
// }

#[cfg(target_os = "windows")]
pub fn open_url(url: &str) -> u32 {
    use winapi::{
        shared::minwindef::TRUE,
        um::{
            processthreadsapi::GetProcessId,
            shellapi::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
            winuser::SW_SHOW,
        },
    };
    let open = [111u16, 112, 101, 110, 0]; // "open"
    let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.lpVerb = open.as_ptr();
    info.lpFile = url.as_ptr();
    info.nShow = SW_SHOW;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    let result = unsafe { ShellExecuteExW(&mut info) };
    if result == TRUE {
        unsafe { GetProcessId(info.hProcess) }
    } else {
        0
    }
}

#[cfg(target_os = "linux")]
pub fn open_url(url: &str) -> u32 {
    use std::process::Command;
    open_on_unix_using_browser_env(url)
        .or_else(|_| Command::new("xdg-open").arg(url).spawn())
        .or_else(|r| {
            if let Ok(desktop) = ::std::env::var("XDG_CURRENT_DESKTOP") {
                if desktop == "KDE" {
                    return Command::new("kioclient").arg("exec").arg(url).spawn();
                }
            }
            Err(r) // If either `if` check fails, fall through to the next or_else
        })
        .or_else(|_| Command::new("gvfs-open").arg(url).spawn())
        .or_else(|_| Command::new("gnome-open").arg(url).spawn())
        .or_else(|_| Command::new("open").arg(url).spawn())
        .or_else(|_| Command::new("kioclient").arg("exec").arg(url).spawn())
        .or_else(|_e| Command::new("x-www-browser").arg(url).spawn())
        .map(|c| c.id())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn open_on_unix_using_browser_env(url: &str) -> std::io::Result<std::process::Child> {
    use std::{
        io::{Error, ErrorKind},
        process::Command,
    };
    let browsers = ::std::env::var("BROWSER")
        .map_err(|_| -> Error { Error::new(ErrorKind::NotFound, "BROWSER env not set") })?;
    for browser in browsers.split(':') {
        // $BROWSER can contain ':' delimited options, each representing a potential browser command line
        if !browser.is_empty() {
            // each browser command can have %s to represent URL, while %c needs to be replaced
            // with ':' and %% with '%'
            let cmdline = browser
                .replace("%s", url)
                .replace("%c", ":")
                .replace("%%", "%");
            let cmdarr: Vec<&str> = cmdline.split_whitespace().collect();
            let mut cmd = Command::new(&cmdarr[0]);
            if cmdarr.len() > 1 {
                cmd.args(&cmdarr[1..cmdarr.len()]);
            }
            if !browser.contains("%s") {
                // append the url as an argument only if it was not already set via %s
                cmd.arg(url);
            }
            if let Ok(c) = cmd.spawn() {
                return Ok(c);
            }
        }
    }
    Err(Error::new(
        ErrorKind::NotFound,
        "No valid command in $BROWSER",
    ))
}

#[cfg(target_os = "macos")]
pub fn open_url(url: &str) -> u32 {
    use std::process::Command;
    Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map(|x| x.id())
        .unwrap_or(0)
}
