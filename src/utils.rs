//! 一些常用的玩意

use std::{fmt::Display, io::Write, path::Path};

/// 安全写入文件数据，写入完成后会等待文件缓冲区完全写入才关闭文件
pub fn write_file_safe(p: impl AsRef<Path>, data: &[u8]) -> Result<(), std::io::Error> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(p)?;
    f.write_all(data)?;
    f.flush()?;
    f.sync_all()?;
    Ok(())
}

pub enum Arch {
    X86,
    X64,
    ARM64,
}

impl Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(windows)]
        match self {
            Arch::X86 => f.write_str("windows-386"),
            Arch::X64 => f.write_str("windows-amd64"),
            Arch::ARM64 => f.write_str("windows-arm64"),
        }
        #[cfg(target_os = "linux")]
        match self {
            Arch::X86 => f.write_str("linux-386"),
            Arch::X64 => f.write_str("linux-amd64"),
            Arch::ARM64 => f.write_str("linux-arm64"),
        }
        #[cfg(target_os = "macos")]
        match self {
            Arch::X86 => f.write_str("darwin-386"),
            Arch::X64 => f.write_str("darwin-amd64"),
            Arch::ARM64 => f.write_str("darwin-arm64"),
        }
    }
}

pub fn get_system_arch() -> Arch {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};
        let mut info: SYSTEM_INFO = Default::default();
        GetNativeSystemInfo(&mut info);
        match info.Anonymous.Anonymous.wProcessorArchitecture.0 {
            0 => Arch::X86,
            12 => Arch::ARM64,
            9 => Arch::X64,
            _ => unreachable!(),
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "x86"))]
    return Arch::X86;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Arch::X64;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Arch::ARM64;
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Arch::X64;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Arch::ARM64;
}
