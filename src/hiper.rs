use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Read, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicU32},
};

use crate::{ui::*, DynResult};
use anyhow::Context;
use druid::{ExtEventSink, Target};
use windows::Win32::System::{
    ProcessStatus::{K32EnumDeviceDrivers, K32GetDeviceDriverBaseNameW},
    SystemInformation::{GetSystemInfo, SYSTEM_INFO},
    Threading::{OpenProcess, TerminateProcess, PROCESS_ACCESS_RIGHTS},
};

static HIPER_PROCESS: AtomicU32 = AtomicU32::new(0);
static HAS_UPDATED: AtomicBool = AtomicBool::new(false);

fn check_tap_installed() -> bool {
    unsafe {
        let mut drivers = Vec::with_capacity(512);
        let mut lpcb_needed = 0;
        K32EnumDeviceDrivers(
            drivers.as_mut_ptr(),
            drivers.capacity() as _,
            &mut lpcb_needed,
        )
        .unwrap();
        if lpcb_needed > drivers.capacity() as _ {
            drivers = Vec::with_capacity(lpcb_needed as _);
            K32EnumDeviceDrivers(
                drivers.as_mut_ptr(),
                drivers.capacity() as _,
                &mut lpcb_needed,
            )
            .unwrap();
        }
        drivers.set_len(lpcb_needed as _);
        let mut filename = vec![0; 256];
        for driver_handle in drivers {
            let strlen = K32GetDeviceDriverBaseNameW(driver_handle, &mut filename) as usize;
            let filename = String::from_utf16_lossy(&filename[..strlen]);
            if filename.ends_with("tap0901.sys") {
                return true;
            }
        }
    }
    false
}

enum Arch {
    X86,
    X64,
    ARM64,
}

fn get_system_arch() -> Arch {
    unsafe {
        let mut info: SYSTEM_INFO = Default::default();
        GetSystemInfo(&mut info);
        match info.Anonymous.Anonymous.wProcessorArchitecture.0 {
            0 => Arch::X86,
            12 => Arch::ARM64,
            9 => Arch::X64,
            _ => unreachable!(),
        }
    }
}

pub fn run_hiper(ctx: ExtEventSink, token: String, use_tun: bool) -> DynResult {
    println!("Launching hiper using token {}", token);

    let has_token = !token.is_empty();
    let _ = ctx.submit_command(SET_START_TEXT, "正在检查所需文件", Target::Auto);
    let _ = ctx.submit_command(SET_WARNING, "".to_string(), Target::Auto);

    let appdata = PathBuf::from_str(std::env!("APPDATA")).context("Can't get appdata path!")?;
    let hiper_dir_path = appdata.join("hiper");

    let tap_path = hiper_dir_path.join("tap-windows.exe");
    let wintun_path = hiper_dir_path.join("wintun.dll");
    let wintun_disabled_path = hiper_dir_path.join("wintun.dll.disabled");
    let hiper_plus_path = hiper_dir_path.join("hpr.exe");
    let hiper_env_path = hiper_dir_path.join("hpr_env.exe");

    std::fs::create_dir_all(&hiper_dir_path).context("Can't create hiper path!")?;

    if !use_tun && wintun_path.exists() {
        std::fs::rename(&wintun_path, &wintun_disabled_path)
            .context("Can't rename hiper to disabled!")?;
    } else if use_tun && wintun_disabled_path.exists() {
        std::fs::rename(&wintun_disabled_path, &wintun_path)
            .context("Can't rename hiper from disabled!")?;
    }

    if use_tun {
        if !wintun_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在下载安装 WinTUN", Target::Auto);
            let res = tinyget::get(
                "https://gitcode.net/to/hiper/-/raw/plus/windows/wintun/amd64/wintun.dll",
            )
            .send()
            .context("Can't send tap download request!")?;
            std::fs::write(&wintun_path, res.as_bytes()).context("Can't write tap into file!")?;
        }
    } else if !check_tap_installed() {
        if !tap_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在下载 TAP 虚拟网卡", Target::Auto);
            let res = tinyget::get(
                "https://gitcode.net/chearlai/f/-/raw/master/d/tap-windows-9.21.2.exe",
            )
            .send()
            .context("Can't send tap download request!")?;
            std::fs::write(&tap_path, res.as_bytes()).context("Can't write tap into file!")?;
        }
        let _ = ctx.submit_command(SET_START_TEXT, "正在安装 TAP 虚拟网卡", Target::Auto);

        let c = Command::new(tap_path)
            .arg("/S")
            .status()
            .context("Failed to run tap installer!")?;
        c.code().context("Failed to install tap!")?;
    }

    let _update_available = false;

    if !HAS_UPDATED.load(std::sync::atomic::Ordering::SeqCst) {
        if hiper_plus_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在检查 HiPer 并更新", Target::Auto);
        } else {
            let _ = ctx.submit_command(SET_START_TEXT, "正在安装 HiPer", Target::Auto);
        }
        let res = tinyget::get("https://gitcode.net/to/hiper/-/raw/plus/windows/64bit/hpr.exe")
            .send()
            .context("Can't send tap download hiper request!")?;
        println!("HPR downloaded, size {}", res.as_bytes().len());
        std::fs::write(&hiper_plus_path, res.as_bytes()).context("Can't write hpr into file!")?;

        let res = tinyget::get("https://gitcode.net/to/hiper/-/raw/plus/windows/64bit/hpr_env.exe")
            .send()
            .context("Can't send tap download hiper environment utils request!")?;
        println!("HPR Env downloaded, size {}", res.as_bytes().len());
        std::fs::write(&hiper_env_path, res.as_bytes())
            .context("Can't write hpr env into file!")?;

        HAS_UPDATED.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    let _ = ctx.submit_command(SET_START_TEXT, "正在启动 HiPer", Target::Auto);

    let mut child = Command::new(hiper_plus_path);

    if has_token {
        child.arg("-t");
        child.arg(token);
    }

    println!("{:?}", child);

    let mut child = child
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .context("Failed to start hiper!")?;

    HIPER_PROCESS.store(child.id(), std::sync::atomic::Ordering::SeqCst);

    let stdout = child
        .stdout
        .take()
        .context("Can't get stdout from hiper!")?;
    let mut stdout = BufReader::new(stdout);
    let mut buf = String::with_capacity(256);

    let (sender, reciver) = oneshot::channel::<String>();

    std::thread::spawn(move || -> DynResult {
        // Start Logging
        let mut logger_file = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open("latest.log")
            .context("Can't open logger file (latest.log)!");
        let mut sender = Some(sender);
        while let Ok(len) = stdout.read_line(&mut buf) {
            if len == 0 {
                if let Some(sender) = sender.take() {
                    sender.send("".into()).map_err(|x| {
                        anyhow::anyhow!("Can't send ip to parent thread! {}", x.as_inner())
                    })?;
                }
                return Ok(());
            }
            if let Ok(Some(_)) = child.try_wait() {
                if let Some(sender) = sender.take() {
                    sender.send("".into()).map_err(|x| {
                        anyhow::anyhow!("Can't send ip to parent thread! {}", x.as_inner())
                    })?;
                    return Ok(());
                }
            }
            let line = buf[..len].trim();
            println!("[HPR] {}", line);
            if let Ok(logger_file) = &mut logger_file {
                let _ = logger_file.write(line.as_bytes());
                let _ = logger_file.write(b"\n");
            }
            if let Some(ipv4) = crate::log_parser::try_get_ipv4(line) {
                if ipv4.is_unspecified() {
                    if let Some(sender) = sender.take() {
                        sender.send("".into()).map_err(|x| {
                            anyhow::anyhow!("Can't send ip to parent thread! {}", x.as_inner())
                        })?;
                    }
                } else if let Some(sender) = sender.take() {
                    sender.send(ipv4.to_string()).map_err(|x| {
                        anyhow::anyhow!("Can't send ip to parent thread! {}", x.as_inner())
                    })?;
                }
            }
            buf.clear();
        }
        Ok(())
    });

    let ip = reciver
        .recv()
        .context("Can't receive ip from logger thread!")?;

    if ip.is_empty() {
        let _ = ctx.submit_command(SET_START_TEXT, "启动", Target::Auto);
        stop_hiper_directly();
        if has_token {
            let _ = ctx.submit_command(
                SET_WARNING,
                "错误：HiPer 启动失败！请检查 latest.log 日志文件确认问题！".to_string(),
                Target::Auto,
            );
        } else {
            let _ = ctx.submit_command(
                SET_WARNING,
                "错误：HiPer 入网失败！请检查访问密钥是否填写正确！".to_string(),
                Target::Auto,
            );
        }
    } else {
        if !has_token {
            let _ = ctx.submit_command(
                SET_WARNING,
                "警告：没有提供凭证，HiPer 将使用临时网络连接并将会在半小时后断连！".to_string(),
                Target::Auto,
            );
        }
        let _ = ctx.submit_command(SET_IP, ip, Target::Auto);
        let _ = ctx.submit_command(SET_START_TEXT, "关闭", Target::Auto);
    }

    Ok(())
}

pub fn stop_hiper_directly() {
    let pid = HIPER_PROCESS.swap(0, std::sync::atomic::Ordering::SeqCst);
    if pid != 0 {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_ACCESS_RIGHTS(0x0001), false, pid) {
                TerminateProcess(handle, 0);
            }
        }
    }
}

pub fn stop_hiper(ctx: ExtEventSink) {
    let _ = ctx.submit_command(SET_START_TEXT, "正在关闭 HiPer", Target::Auto);
    let _ = ctx.submit_command(SET_WARNING, "".to_string(), Target::Auto);
    let _ = ctx.submit_command(SET_IP, "".to_string(), Target::Auto);

    stop_hiper_directly();

    let _ = ctx.submit_command(SET_START_TEXT, "启动", Target::Auto);
}
