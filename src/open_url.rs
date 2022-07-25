pub fn open_url(url: &str) -> u32 {
    use windows::core::*;
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::*;
    let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let mut info: SHELLEXECUTEINFOW = SHELLEXECUTEINFOW {
        lpVerb: w!("open").into(),
        lpFile: PCWSTR::from_raw(url.as_ptr()),
        nShow: 5,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        ..Default::default()
    };
    let result = unsafe { ShellExecuteExW(&mut info) };
    if result.as_bool() {
        unsafe { GetProcessId(info.hProcess) }
    } else {
        0
    }
}
