use std::{
    fmt::Display,
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Mutex,
    },
};

use crate::{ui::*, utils::write_file_safe, DynResult};
use anyhow::Context;
use druid::{ExtEventSink, Target};
use path_absolutize::Absolutize;
#[cfg(windows)]
use windows::Win32::System::{
    ProcessStatus::{K32EnumDeviceDrivers, K32GetDeviceDriverBaseNameW},
    SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO},
    Threading::{
        OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    },
};

static HIPER_PROCESS: AtomicU32 = AtomicU32::new(0);
static HAS_UPDATED: AtomicBool = AtomicBool::new(false);
static SPAWNED_PROCESSES: Mutex<Option<Vec<u32>>> = Mutex::new(None);

#[cfg(windows)]
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

pub fn get_log_file_path() -> DynResult<PathBuf> {
    use path_absolutize::*;
    Ok(get_hiper_dir()?
        .join("latest.log")
        .absolutize()
        .map(|x| x.to_path_buf())?)
}

enum Arch {
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
        #[cfg(unix)]
        match self {
            Arch::X86 => f.write_str("linux-386"),
            Arch::X64 => f.write_str("linux-amd64"),
            Arch::ARM64 => f.write_str("linux-arm64"),
        }
        #[cfg(osx)]
        match self {
            Arch::X86 => f.write_str("darwin-386"),
            Arch::X64 => f.write_str("darwin-amd64"),
            Arch::ARM64 => f.write_str("darwin-arm64"),
        }
    }
}

fn get_system_arch() -> Arch {
    #[cfg(windows)]
    unsafe {
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

pub fn run_hiper_in_thread(ctx: ExtEventSink, token: String, use_tun: bool, debug_mode: bool) {
    std::thread::spawn(move || {
        let _ = ctx.submit_command(SET_DISABLED, true, Target::Auto);
        match run_hiper(ctx.to_owned(), token, use_tun, debug_mode) {
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
    #[cfg(windows)]
    {
        let appdata =
            PathBuf::from_str(&std::env::var("APPDATA").context("无法获取 APPDATA 环境变量")?)
                .context("无法将 APPDATA 环境变量转换成路径")?;
        let hiper_dir_path = appdata.join("hiper");
        Ok(hiper_dir_path)
    }
    #[cfg(unix)]
    {
        Ok(PathBuf::from_str("/etc/hiper").context("无法将路径字符串转换成路径")?)
    }
}

pub fn run_hiper(ctx: ExtEventSink, token: String, use_tun: bool, debug_mode: bool) -> DynResult {
    println!("Launching hiper using token {}", token);

    let has_token = !token.is_empty();
    let _ = ctx.submit_command(SET_START_TEXT, "正在检查所需文件", Target::Auto);
    let _ = ctx.submit_command(SET_WARNING, "".to_string(), Target::Auto);

    let hiper_dir_path = get_hiper_dir()?;
    let certs_dir_path = hiper_dir_path.join("certs");

    let tap_path = hiper_dir_path.join("tap-windows.exe");
    let wintun_path = hiper_dir_path.join("wintun.dll");
    let wintun_disabled_path = hiper_dir_path.join("wintun.dll.disabled");
    let hiper_path = hiper_dir_path.join("hiper.exe");

    std::fs::create_dir_all(&hiper_dir_path).context("无法创建 HiPer 安装目录")?;
    std::fs::create_dir_all(&certs_dir_path).context("无法创建 HiPer 凭证证书目录")?;

    let cert_path = certs_dir_path.join(format!("{}.yml", token));
    let cert_path = cert_path
        .absolutize()
        .context("无法获取凭证证书所在绝对目录")?;

    let logger_json_data = "\nlogging:\n  format: json";

    if cert_path.is_file() {
        // 确认配置是否设定了日志格式
        let mut cert_data = std::fs::read_to_string(&cert_path).context("无法读取检查凭证证书")?;
        if !cert_data.contains(logger_json_data) {
            cert_data.push_str(logger_json_data);
            write_file_safe(&cert_path, cert_data.as_bytes()).context("无法保存凭证证书")?;
        }
    } else {
        let _ = ctx.submit_command(SET_START_TEXT, "正在获取凭证证书", Target::Auto);
        let res = tinyget::get(format!("https://cert.mcer.cn/{}.yml", token))
            .send()
            .context("无法获取凭证证书，这有可能是因为下载超时或者是你的凭证无效")?;
        if res.status_code != 200 {
            anyhow::bail!("无法获取凭证证书，这有可能是因为下载超时或者是你的凭证无效");
        }
        let mut cert_data = res
            .as_str()
            .context("无法正确解码凭证证书数据，这有可能是下载出错了")?
            .to_owned();
        cert_data.push_str(logger_json_data);
        write_file_safe(&cert_path, cert_data.as_bytes()).context("无法保存凭证证书")?;
    }

    if !use_tun && wintun_path.exists() {
        std::fs::rename(&wintun_path, &wintun_disabled_path).context("无法禁用 WinTUN")?;
    } else if use_tun && wintun_disabled_path.exists() {
        std::fs::rename(&wintun_disabled_path, &wintun_path).context("无法启用 WinTUN")?;
    }

    if use_tun {
        #[cfg(windows)]
        if !wintun_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在下载安装 WinTUN", Target::Auto);
            let res = tinyget::get(&format!(
                "https://gitcode.net/to/hiper/-/raw/master/{}/wintun.dll",
                get_system_arch()
            ))
            .send()
            .context("无法下载 WinTUN")?;
            write_file_safe(&wintun_path, res.as_bytes()).context("无法安装 WinTUN")?;
        }
    } else {
        #[cfg(windows)]
        if !check_tap_installed() {
            if !tap_path.exists() {
                let _ = ctx.submit_command(SET_START_TEXT, "正在下载 WinTAP", Target::Auto);
                let res = tinyget::get(
                    "https://gitcode.net/to/hiper/-/raw/master/tap-windows-9.21.2.exe",
                )
                .send()
                .context("无法下载 WinTAP 安装程序")?;
                write_file_safe(&tap_path, res.as_bytes()).context("无法写入 WinTAP 安装程序！")?;
            }
            let _ = ctx.submit_command(SET_START_TEXT, "正在安装 WinTAP", Target::Auto);

            let c = Command::new(tap_path)
                .arg("/S")
                .status()
                .context("无法运行 WinTAP 安装程序")?;
            c.code().context("无法安装 WinTAP")?;
        }
    }

    let _update_available = false;

    if !HAS_UPDATED.load(std::sync::atomic::Ordering::SeqCst) {
        let arch = get_system_arch().to_string();
        #[cfg(windows)]
        let download_url = format!(
            "https://gitcode.net/to/hiper/-/raw/master/{}/hiper.exe",
            arch
        );
        #[cfg(not(windows))]
        let download_url = format!("https://gitcode.net/to/hiper/-/raw/master/{}/hiper", arch);

        if hiper_path.exists() {
            let _ = ctx.submit_command(SET_START_TEXT, "正在检查 HiPer 更新", Target::Auto);

            // 计算现有的 SHA1
            let mut s = sha1_smol::Sha1::default();
            s.update(&std::fs::read(&hiper_path).context("无法读取 HiPer 程序以计算摘要")?);
            let current_hash = s.hexdigest();

            let res = tinyget::get("https://gitcode.net/to/hiper/-/raw/master/packages.sha1")
                .send()
                .context("无法获取 HiPer 下载索引")?
                .as_str()
                .context("无法解析 HiPer 下载索引")?
                .to_owned();

            for line in res.split('\n') {
                if let Some((hash, path)) = line.split_once("  ") {
                    #[cfg(windows)]
                    let found = path.starts_with(&arch) && path.ends_with("hiper.exe");
                    #[cfg(not(windows))]
                    let found = path.starts_with(&arch) && path.ends_with("hiper");
                    if found {
                        println!("Comparing {} {} {} {}", arch, path, hash, current_hash);
                        if hash != current_hash {
                            let _ =
                                ctx.submit_command(SET_START_TEXT, "正在更新 HiPer", Target::Auto);

                            let res = tinyget::get(download_url.as_str())
                                .send()
                                .context("无法下载 HiPer 程序")?;
                            println!("HPR downloaded, size {}", res.as_bytes().len());

                            write_file_safe(&hiper_path, res.as_bytes())
                                .context("无法更新 HiPer 程序")?;
                        }
                        break;
                    }
                }
            }
        } else {
            let _ = ctx.submit_command(SET_START_TEXT, "正在安装 HiPer", Target::Auto);

            let res = tinyget::get(download_url.as_str())
                .send()
                .context("无法下载 HiPer 程序")?;
            println!("HPR downloaded, size {}", res.as_bytes().len());

            write_file_safe(&hiper_path, res.as_bytes()).context("无法安装 HiPer 程序")?;
        }
    }

    let _ = ctx.submit_command(SET_START_TEXT, "正在启动 HiPer", Target::Auto);

    let mut child = Command::new(hiper_path);

    if has_token {
        child.arg("-config");
        child.arg(cert_path.to_path_buf());
    }

    let (sender, reciver) = oneshot::channel::<String>();

    let ctx_c = ctx.to_owned();
    std::thread::spawn(move || -> DynResult {
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        #[cfg(windows)]
        let mut child = child
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .context("无法启动 HiPer")?;
        #[cfg(not(windows))]
        let mut child = child
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("无法启动 HiPer")?;

        #[cfg(all(windows, not(debug_assertions)))]
        if debug_mode {
            unsafe {
                windows::Win32::System::Console::AllocConsole();
                // 设置控制台关闭指令
                // 阻止调试控制台的直接关闭进程
                use windows::Win32::System::Console::*;
                unsafe extern "system" fn console_ctrl_handler(
                    event: u32,
                ) -> windows::Win32::Foundation::BOOL {
                    match event {
                        CTRL_CLOSE_EVENT | CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_LOGOFF_EVENT
                        | CTRL_SHUTDOWN_EVENT => {
                            println!("[WARN] 请不要直接停止控制台窗口！请点击主窗口的关闭按钮关闭 HiPer Bridge！");
                            stop_hiper_directly();
                        }
                        _ => {}
                    }
                    true.into()
                }
                SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
                println!(
                    "[WARN] 请不要直接关闭控制台窗口！请点击主窗口的关闭按钮关闭 HiPer Bridge！"
                );
            }
        }

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

        // Start Logging
        let mut logger_file = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(get_log_file_path()?)
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
                        if let Ok(ipv4) = ipv4.parse::<std::net::Ipv4Addr>() {
                            if ipv4.is_unspecified() {
                                if let Some(sender) = sender.take() {
                                    sender.send("".into()).map_err(|x| {
                                        anyhow::anyhow!(
                                            "无法发送 IP 地址到父线程：{}",
                                            x.as_inner()
                                        )
                                    })?;
                                }
                            } else if let Some(sender) = sender.take() {
                                sender.send(ipv4.to_string()).map_err(|x| {
                                    anyhow::anyhow!("无法发送 IP 地址到父线程：{}", x.as_inner())
                                })?;
                                crate::tray::set_icon(true);
                                crate::tray::notify(
                                    "HiPer Bridge 正在运行！",
                                    &format!("现在可以使用地址 {} 来访问 HiPer 网络了", ipv4),
                                );
                                sent = true;
                            }
                        }
                    } else if let Some((level, _msg, error)) =
                        crate::log_parser::try_get_log_line(line)
                    {
                        if &level == "error" {
                            match error.as_str() {
                                "hiper certificate for this host is expired" => {
                                    let _ = ctx_c.submit_command(
                                        SET_WARNING,
                                        "警告：凭证已过期！请使用新的凭证密钥重试！".to_string(),
                                        Target::Auto,
                                    );
                                    sent = false;
                                }
                                "Failed to open udp listener" => {
                                    let _ = ctx_c.submit_command(
                                        SET_WARNING,
                                        "错误：HiPer无法监听服务端口，请确认端口占用情况"
                                            .to_string(),
                                        Target::Auto,
                                    );
                                    sent = false;
                                }
                                "Failed to get a tun/tap device" => {
                                    let _ = ctx_c.submit_command(
                                        SET_WARNING,
                                        "错误：无法获取 TUN/TAP 设备！这应该是你多开了 HiPer 导致设备被占用了".to_string(),
                                        Target::Auto,
                                    );
                                    sent = false;
                                }
                                _ => {
                                    // let _ = ctx_c.submit_command(
                                    //     SET_WARNING,
                                    //     "错误：HiPer 启动失败！请检查 latest.log 日志文件确认问题！".to_string(),
                                    //     Target::Auto,
                                    // );
                                    // sent = false;
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            let _ = ctx_c.submit_command(SET_WARNING, "".to_string(), Target::Auto);
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
        #[cfg(all(windows, not(debug_assertions)))]
        if debug_mode {
            unsafe {
                windows::Win32::System::Console::FreeConsole();
            }
        }
        println!("[WARN] HiPer 已退出！");
        if sent && !child.wait().map(|x| x.success()).unwrap_or(false) {
            let _ = ctx_c.submit_command(
                SET_WARNING,
                "警告：HiPer Bridge 意外退出！5 秒后将会自动重启！\n　　如需阻止自动重启，请点击关闭按钮！".to_string(),
                Target::Auto,
            );
            std::thread::sleep(std::time::Duration::from_secs(5));
            let _ = ctx_c.submit_command(REQUEST_RESTART, (), Target::Auto);
        }
        crate::tray::set_icon(false);
        Ok(())
    });

    let ip = reciver.recv().context("未能从 HiPer 输出中获取 IP 地址")?;

    if ip.is_empty() {
        let _ = ctx.submit_command(SET_START_TEXT, "启动", Target::Auto);
        stop_hiper_directly();
        if !has_token {
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
    #[cfg(windows)]
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, false, pid) {
            TerminateProcess(handle, 0);
            let _r = WaitForSingleObject(handle, 0);
        }
    }
}

pub fn is_running() -> bool {
    HIPER_PROCESS.load(std::sync::atomic::Ordering::SeqCst) != 0
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
