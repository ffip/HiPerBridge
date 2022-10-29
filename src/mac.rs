//! 针对 MacOS 的服务安装和卸载
//!

use std::{
    path::{Path, PathBuf},
    result,
    time::Duration,
};

use anyhow::Context;
use cocoa::foundation::NSString;
use druid::{ExtEventSink, Target};
use scl_gui_widgets::widgets::{ENABLE_BACK_PAGE, QUERY_POP_PAGE};

use crate::{app_state::AppState, DynResult};

pub struct Launchctl {
    domain_target: String,
    service_path: PathBuf,
}

impl Launchctl {
    pub fn new() -> Launchctl {
        let uid = 0; //nix::unistd::getuid().as_raw();
        let (domain_target, service_path) = if uid == 0 {
            ("system".into(), "/Library/LaunchDaemons".into())
        } else {
            let mut path = dirs::home_dir().unwrap();
            path.push("Library/LaunchAgents");
            (format!("gui/{}", uid), path)
        };

        Launchctl {
            domain_target,
            service_path,
        }
    }

    pub fn install_plist<P: AsRef<Path>>(&self, path: P) -> DynResult {
        if let Some(name) = path.as_ref().file_name() {
            let mut install_path = self.service_path.clone();

            // Create `Launch..` dir if it doesn't already exist.
            if !install_path.exists() {
                std::fs::create_dir(dbg!(&install_path))?;
            }

            install_path.push(name);

            if !install_path.exists() {
                std::fs::copy(&path, dbg!(&self.service_path))
                    .context("Could not install plist")?;
            }

            Ok(())
        } else {
            anyhow::bail!("Plist path does not contain filename")
        }
    }

    pub fn uninstall_plist(&self, name: &str) -> DynResult {
        let mut path = self.service_path.clone();
        path.push(name);
        path.set_extension("plist");
        if path.exists() {
            std::fs::remove_file(&path).context("Could not uninstall plist")?;
        }

        Ok(())
    }

    pub fn enable(&self, name: &str) -> DynResult {
        let status = std::process::Command::new("/bin/launchctl")
            .arg("enable")
            .arg(format!("{}/{}", self.domain_target, name))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("Failed to enable service {}", name);
        }
    }

    pub fn disable(&self, name: &str) -> DynResult {
        let status = std::process::Command::new("/bin/launchctl")
            .arg("disable")
            .arg(format!("{}/{}", self.domain_target, name))
            .status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("Failed to enable service {}", name);
        }
    }
}

pub fn install_plist() -> DynResult {
    let launchctl = Launchctl::new();
    let tmp_dir = PathBuf::from("/tmp");
    let temp_plist_path = tmp_dir.join("com.matrix.hiper.plist");
    std::fs::write(
        &temp_plist_path,
        include_str!("../assets/com.matrix.hiper.plist"),
    )?;
    launchctl.install_plist(dbg!(temp_plist_path))?;
    Ok(())
}

pub fn uninstall_plist() -> DynResult {
    let launchctl = Launchctl::new();
    launchctl.uninstall_plist("com.matrix.hiper")?;
    Ok(())
}

pub fn enable_plist() -> DynResult {
    let launchctl = Launchctl::new();
    launchctl.enable("hiper")?;
    Ok(())
}

pub fn disable_plist() -> DynResult {
    let launchctl = Launchctl::new();
    launchctl.disable("hiper")?;
    Ok(())
}

pub fn is_hiper_installed() -> bool {
    use std::path::*;
    let hiper_path = Path::new("/usr/local/bin/hiper-daemon");
    let hiper_plist_path = Path::new("/Library/LaunchDaemons/HPNS.plist");
    let hiper_config_path = Path::new("/etc/hiper/config.yml");
    hiper_path.is_file() && hiper_plist_path.is_file() && hiper_config_path.is_file()
}

pub fn install_hiper(ctx: ExtEventSink) -> DynResult {
    let install_script = "\
    mv /tmp/hiper-daemon /usr/local/bin/hiper-daemon && \
    chmod +x /usr/local/bin/hiper-daemon && \
    /usr/local/bin/hiper-daemon -service install && \
    launchctl load /Library/LaunchDaemons/HPNS.plist && \
    touch /etc/hiper/config.yml && \
    chmod 777 /etc/hiper/config.yml
    ";
    #[cfg(target_arch = "x86_64")]
    let daemon_url = "https://gitcode.net/to/hiper/-/raw/master/darwin-amd64/hiper-daemon";
    #[cfg(target_arch = "aarch64")]
    let daemon_url = "https://gitcode.net/to/hiper/-/raw/master/darwin-arm64/hiper-daemon";

    ctx.add_idle_callback(|data: &mut AppState| {
        data.init_message = "正在下载 HiPer Daemon".into();
        data.running_script = true;
    });

    let daemon = tinyget::get(daemon_url)
        .send()
        .context("下载 HiPer Daemon 失败")?;
    crate::utils::write_file_safe("/tmp/hiper-daemon", daemon.as_bytes())
        .context("写入 HiPer Daemon 到临时目录失败")?;

    ctx.add_idle_callback(|data: &mut AppState| {
        data.init_message = "正在运行安装脚本".into();
    });

    println!("Running Script");
    let result = crate::mac::do_admin_shell_in_apple_script(install_script);
    println!("Finished Running Script");

    if let Ok(result) = result {
        println!("Result:\n{}", result);
    }

    use std::path::*;
    let hiper_path = Path::new("/usr/local/bin/hiper-daemon");
    let hiper_plist_path = Path::new("/Library/LaunchDaemons/HPNS.plist");
    let hiper_config_path = Path::new("/etc/hiper/config.yml");
    if !hiper_path.is_file() || !hiper_plist_path.is_file() {
        anyhow::bail!("安装 HiPer 失败")
    }

    Ok(())
}

/// 以管理员身份执行脚本
///
/// 将会强制要求用户输入密码以进行提权
///
/// 执行成功后会返回脚本输出内容，否则会报错
pub fn do_admin_shell_in_apple_script(shell_script: &str) -> DynResult<String> {
    unsafe {
        use cocoa::base::*;
        use objc::*;
        let nsdictonary_cls = class!(NSDictionary);
        let error: id = msg_send![nsdictonary_cls, new];
        let script_body = format!(
            "return do shell script \"{}\" with administrator privileges",
            shell_script.replace('\"', "\\\"")
        );
        let script = NSString::alloc(nil).init_str(&script_body);

        let ns_script_cls = class!(NSAppleScript);
        let apple_script: id = msg_send![ns_script_cls, alloc];
        let apple_script: id = msg_send![apple_script, initWithSource: script];

        let result: id = msg_send![apple_script, executeAndReturnError: error];

        if result.is_null() {
            anyhow::bail!("执行管理员脚本失败")
        } else {
            let string_value: id = msg_send![result, stringValue];

            let string_value = NSString::UTF8String(string_value);
            let string_value = std::ffi::CStr::from_ptr(string_value);
            let string_value = string_value.to_str()?.to_string();
            Ok(string_value)
        }
    }
}
