use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Mutex,
    },
};

use crate::{ui::*, DynResult};
use anyhow::Context;
use druid::{ExtEventSink, Target};
use windows::Win32::System::{
    ProcessStatus::{K32EnumDeviceDrivers, K32GetDeviceDriverBaseNameW},
    SystemInformation::{GetSystemInfo, SYSTEM_INFO},
    Threading::{OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_ACCESS_RIGHTS},
};

static HIPER_PROCESS: AtomicU32 = AtomicU32::new(0);
static HAS_UPDATED: AtomicBool = AtomicBool::new(false);
static SPAWNED_PROCESSES: Mutex<Option<Vec<u32>>> = Mutex::new(None);

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

pub fn run_hiper_in_thread(ctx: ExtEventSink, token: String, use_tun: bool) {
    std::thread::spawn(move || {
        let _ = ctx.submit_command(SET_DISABLED, true, Target::Auto);
        match run_hiper(ctx.to_owned(), token, use_tun) {
            Ok(_) => {
                println!("Launched!");
            }
            Err(e) => {
                println!("Failed to launch! {:?}", e);
                let _ = ctx.submit_command(
                    SET_WARNING,
                    format!("启动时发生错误：{:?}", e),
                    Target::Auto,
                );
                let _ = ctx.submit_command(SET_START_TEXT, "启动", Target::Auto);
            }
        }
        let _ = ctx.submit_command(SET_DISABLED, false, Target::Auto);
    });
}

pub fn get_hiper_dir() -> DynResult<PathBuf> {
    let appdata = PathBuf::from_str(std::env!("APPDATA")).context("无法获取 APPDATA 环境变量")?;
    let hiper_dir_path = appdata.join("hiper");
    Ok(hiper_dir_path)
}

pub fn run_hiper(ctx: ExtEventSink, token: String, use_tun: bool) -> DynResult {
    println!("Launching hiper using token {}", token);

    let has_token = !token.is_empty();
    let _ = ctx.submit_command(SET_START_TEXT, "正在检查所需文件", Target::Auto);
    let _ = ctx.submit_command(SET_WARNING, "".to_string(), Target::Auto);

    let hiper_dir_path = get_hiper_dir()?;

    let tap_path = hiper_dir_path.join("tap-windows.exe");
    let wintun_path = hiper_dir_path.join("wintun.dll");
    let wintun_disabled_path = hiper_dir_path.join("wintun.dll.disabled");
    let hiper_plus_path = hiper_dir_path.join("hpr.exe");
    let hiper_env_path = hiper_dir_path.join("hpr_env.exe");

    std::fs::create_dir_all(&hiper_dir_path).context("无法创建 HiPer 安装目录")?;

    if !use_tun && wintun_path.exists() {
        std::fs::rename(&wintun_path, &wintun_disabled_path).context("无法禁用 WinTUN")?;
    } else if use_tun && wintun_disabled_path.exists() {
        std::fs::rename(&wintun_disabled_path, &wintun_path).context("无法启用 WinTUN")?;
    }

    if use_tun {
        if !wintun_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在下载安装 WinTUN", Target::Auto);
            let res = tinyget::get(
                "https://gitcode.net/to/hiper/-/raw/plus/windows/wintun/amd64/wintun.dll",
            )
            .send()
            .context("无法下载 WinTUN")?;
            std::fs::write(&wintun_path, res.as_bytes()).context("无法安装 WinTUN")?;
        }
    } else if !check_tap_installed() {
        if !tap_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在下载 WinTAP", Target::Auto);
            let res = tinyget::get(
                "https://gitcode.net/to/hiper/-/raw/plus/windows/tap-windows-9.21.2.exe",
            )
            .send()
            .context("无法下载 WinTAP 安装程序")?;
            std::fs::write(&tap_path, res.as_bytes()).context("无法写入 WinTAP 安装程序！")?;
        }
        let _ = ctx.submit_command(SET_START_TEXT, "正在安装 WinTAP", Target::Auto);

        let c = Command::new(tap_path)
            .arg("/S")
            .status()
            .context("无法运行 WinTAP 安装程序")?;
        c.code().context("无法安装 WinTAP")?;
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
            .context("无法下载 HiPer Plus 程序")?;
        println!("HPR downloaded, size {}", res.as_bytes().len());

        loop {
            std::fs::write(&hiper_plus_path, res.as_bytes()).context("无法安装 HiPer Plus 程序")?;
            let meta = hiper_plus_path
                .metadata()
                .context("无法校验 HiPer 文件正确性")?;
            if meta.len() == res.as_bytes().len() as u64 {
                break;
            }
        }

        if hiper_plus_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在检查 HiPer Env 并更新", Target::Auto);
        } else {
            let _ = ctx.submit_command(SET_START_TEXT, "正在安装 HiPer Env", Target::Auto);
        }

        let res = tinyget::get("https://gitcode.net/to/hiper/-/raw/plus/windows/64bit/hpr_env.exe")
            .send()
            .context("无法下载 HiPer Plus Env 程序")?;
        println!("HPR Env downloaded, size {}", res.as_bytes().len());

        loop {
            std::fs::write(&hiper_env_path, res.as_bytes())
                .context("无法安装 HiPer Plus Env 程序")?;

            let meta = hiper_env_path
                .metadata()
                .context("无法校验 HiPer Plus Env 文件正确性")?;
            if meta.len() == res.as_bytes().len() as u64 {
                break;
            }
        }

        HAS_UPDATED.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    let _ = ctx.submit_command(SET_START_TEXT, "正在启动 HiPer", Target::Auto);

    let mut child = Command::new(hiper_plus_path);

    if has_token {
        child.arg("-t");
        child.arg(token);
    }

    let (sender, reciver) = oneshot::channel::<String>();

    let ctx_c = ctx.to_owned();
    std::thread::spawn(move || -> DynResult {
        let mut child = child
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .context("无法启动 HiPer Plus")?;

        let stdout = child.stdout.take().context("无法获取 HiPer 输出流")?;
        let mut stdout = BufReader::new(stdout);
        let mut buf = String::with_capacity(256);

        stop_hiper_directly();
        if let Ok(mut p) = SPAWNED_PROCESSES.lock() {
            if p.is_none() {
                *p = Some(Vec::with_capacity(16));
            }
            if let Some(p) = p.as_mut() {
                p.push(child.id())
            }
        }
        HIPER_PROCESS.store(child.id(), std::sync::atomic::Ordering::SeqCst);

        println!("{:?}", child);

        // Start Logging
        let mut logger_file = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open("latest.log")
            .context("无法打开日志文件 (latest.log)!");
        let mut sender = Some(sender);
        let mut sent = false;
        let mut no_more_logs = false;

        loop {
            match stdout.read_line(&mut buf) {
                Ok(len) => {
                    no_more_logs |= len == 0;
                    let line = buf[..len].trim();
                    if len != 0 {
                        println!("[HPR] {}", line);
                        if let Ok(logger_file) = &mut logger_file {
                            let _ = logger_file.write(line.as_bytes());
                            let _ = logger_file.write(b"\n");
                        }
                    }
                    if let Some(ipv4) = crate::log_parser::try_get_ipv4(line) {
                        if ipv4.is_unspecified() {
                            if let Some(sender) = sender.take() {
                                sender.send("".into()).map_err(|x| {
                                    anyhow::anyhow!("无法发送 IP 地址到父线程：{}", x.as_inner())
                                })?;
                            }
                        } else if let Some(sender) = sender.take() {
                            sender.send(ipv4.to_string()).map_err(|x| {
                                anyhow::anyhow!("无法发送 IP 地址到父线程：{}", x.as_inner())
                            })?;
                            sent = true;
                        }
                    } else if let Ok(log_line) =
                        crate::log_parser::parse_log_line(line).map(|x| x.1)
                    {
                        if log_line.trim() == "xxx user token has been expired xxx" {
                            let _ = ctx_c.submit_command(
                                SET_WARNING,
                                "警告：凭证已过期！请使用新的凭证密钥重试！".to_string(),
                                Target::Auto,
                            );
                            sent = false;
                        }
                    }
                    if no_more_logs {
                        if let Ok(Some(_)) = child.try_wait() {
                            if let Some(sender) = sender.take() {
                                sender.send("".into()).map_err(|x| {
                                    anyhow::anyhow!("无法发送 IP 地址到父线程：{}", x.as_inner())
                                })?;
                            }
                            break;
                        }
                    }
                    buf.clear();
                }
                Err(err) => {
                    println!("警告：解析日志发生错误：{:?}", err);
                }
            }
        }
        if sent {
            let _ = ctx_c.submit_command(REQUEST_RESTART, (), Target::Auto);
        }
        Ok(())
    });

    let ip = reciver.recv().context("未能从 HiPer 输出中获取 IP 地址")?;

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
                "错误：HiPer 入网失败！请检查凭证密钥是否填写正确！".to_string(),
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

fn stop_process(pid: u32) {
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_ACCESS_RIGHTS(0x0001), false, pid) {
            TerminateProcess(handle, 0);
            WaitForSingleObject(handle, 0);
        }
    }
}

pub fn stop_hiper_directly() {
    let pid = HIPER_PROCESS.swap(0, std::sync::atomic::Ordering::SeqCst);
    if pid != 0 {
        stop_process(pid)
    }
    if let Ok(mut p) = SPAWNED_PROCESSES.lock() {
        if let Some(p) = p.as_mut() {
            for pid in p.drain(..) {
                stop_process(pid);
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
