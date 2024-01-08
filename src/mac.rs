//! 针对 MacOS 的服务安装和卸载
//!

use std::process::Command;
use std::env;

use cocoa::foundation::NSString;
use druid::{ ExtEventSink };

use crate::{ app_state::AppState, DynResult };

pub fn check_sudoer(username: &str) -> bool {
    let output = Command::new("cat").arg("/etc/sudoers").output().expect("执行cat命令错误");

    let sudoers = String::from_utf8(output.stdout).expect("转换字符串错误");
    sudoers.contains(username)
}

pub fn get_current_user() -> String {
    env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

pub fn install_hiper(ctx: ExtEventSink) -> DynResult {
    let user = get_current_user();
    let install_script = format!(
        "\
    export user={}
    if ! grep -q $user /etc/sudoers; then
        echo \"User $user already exists in sudoers file\"
    else
        sudo echo \"$user ALL=(ALL) NOPASSWD: ALL\" >> /etc/sudoers
        echo \"User $user added to sudoers file\"
        sudo chmod a+r /etc/sudoers
    fi
    ",
        &user
    );
    // #[cfg(target_arch = "x86_64")]
    // let daemon_url = "https://gitcode.net/to/hiper/-/raw/master/darwin-amd64/hiper-daemon";
    // #[cfg(target_arch = "aarch64")]
    // let daemon_url = "https://gitcode.net/to/hiper/-/raw/master/darwin-arm64/hiper-daemon";

    // ctx.add_idle_callback(|data: &mut AppState| {
    //     data.init_message = "正在下载 HiPer Daemon".into();
    //     data.running_script = true;
    // });

    // let daemon = tinyget::get(daemon_url).send().context("下载 HiPer Daemon 失败")?;
    // crate::utils
    //     ::write_file_safe("/tmp/hiper-daemon", daemon.as_bytes())
    //     .context("写入 HiPer Daemon 到临时目录失败")?;

    ctx.add_idle_callback(|data: &mut AppState| {
        data.init_message = "正在运行初始化脚本".into();
    });

    println!("Running Script");
    let result = crate::mac::do_admin_shell_in_apple_script(&install_script);
    println!("Finished Running Script");

    if let Ok(result) = result {
        println!("Result:\n{}", result);
    }

    if !check_sudoer(&user) {
        anyhow::bail!("初始化 HiPer 失败");
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
            anyhow::bail!("执行初始化脚本失败")
        } else {
            let string_value: id = msg_send![result, stringValue];

            let string_value = NSString::UTF8String(string_value);
            let string_value = std::ffi::CStr::from_ptr(string_value);
            let string_value = string_value.to_str()?.to_string();
            Ok(string_value)
        }
    }
}
