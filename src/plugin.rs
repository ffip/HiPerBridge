use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::Child,
};

use anyhow::Context;
use druid::{ExtEventSink, Target};
use path_absolutize::Absolutize;
use tinyjson::*;

use crate::{
    hiper::get_hiper_dir,
    ui::{SET_START_TEXT, SET_WARNING},
    utils::write_file_safe,
    DynResult,
};

trait TinyJsonGet {
    fn try_get(&self, key: &str) -> Option<&JsonValue>;
    fn try_get_into<T: tinyjson::InnerAsRef>(&self, key: &str) -> Option<&T>;
}

impl TinyJsonGet for JsonValue {
    fn try_get(&self, key: &str) -> Option<&JsonValue> {
        if let JsonValue::Object(obj) = self {
            obj.get(key)
        } else {
            None
        }
    }
    fn try_get_into<T: tinyjson::InnerAsRef>(&self, key: &str) -> Option<&T> {
        if let Some(obj) = self.try_get(key) {
            obj.get()
        } else {
            None
        }
    }
}

pub fn dispatch_event(event_name: &str) -> Vec<Child> {
    load_plugins()
        .into_iter()
        .flat_map(|x| x.dispatch_event(event_name))
        .collect()
}

pub fn dispatch_event_and_wait(event_name: &str) {
    for mut child in dispatch_event(event_name) {
        match child.wait() {
            Ok(status) => {
                if !status.success() {
                    println!(
                        "[WARN] 有插件触发 {} 事件执行失败，返回值：{}",
                        event_name,
                        status.code().unwrap_or_default()
                    );
                }
            }
            Err(err) => {
                println!("[WARN] 有插件触发 {} 事件执行出错：{}", event_name, err);
            }
        }
    }
}

/// 读取当前已有的所有插件
pub fn load_plugins() -> Vec<Plugin> {
    if let Ok(hiper_dir) = get_hiper_dir() {
        if let Ok(mut read_dir) = std::fs::read_dir(hiper_dir.join("plugins")) {
            let mut plugins = Vec::with_capacity(16);
            while let Some(Ok(entry)) = read_dir.next() {
                let plugin_json_path = entry.path().join("plugin.json");
                if plugin_json_path.is_file() {
                    match Plugin::from_path(plugin_json_path) {
                        Ok(plugin_json) => {
                            plugins.push(plugin_json);
                        }
                        Err(err) => {
                            println!(
                                "[WARN] 无法加载插件 {} ：{}",
                                entry.path().to_string_lossy(),
                                err
                            );
                        }
                    }
                }
            }
            return plugins;
        }
    }
    vec![]
}

pub fn update_plugins(ctx: ExtEventSink) {
    let _ = ctx.submit_command(SET_START_TEXT, "正在检查插件更新", Target::Auto);
    let _ = ctx.submit_command(SET_WARNING, "".to_string(), Target::Auto);

    for plugin in load_plugins() {
        if plugin.update_url.is_empty() {
            continue;
        }
        if let Ok(res) = tinyget::get(&plugin.update_url).send() {
            if res.status_code != 200 {
                continue;
            }
            if let Ok(Ok(update_meta)) = res.as_str().map(PluginUpdateMeta::from_str) {
                if update_meta.version == plugin.version {
                    continue;
                }
                if let Some(target_download) =
                update_meta.downloads.iter().find(|x| x.is_downloadable())
            {
                let _ = ctx.submit_command(
                    SET_START_TEXT,
                    "正在更新插件",
                    Target::Auto,
                );
                let mut buf = Vec::with_capacity(4096);
                if let Ok(res) = tinyget::get(&target_download.url).send() {
                    if res.status_code != 200 {
                        continue;
                    }
                    let r = Cursor::new(res.as_bytes());
                    if let Ok(mut z) = zip::ZipArchive::new(r) {
                        for i in 0..z.len() {
                            if let Ok(mut e) = z.by_index(i) {
                                if let Ok(final_path) = plugin
                                    .path
                                    .join(e.name())
                                    .absolutize()
                                    .map(PathBuf::from)
                                {
                                    // 确保不会恶意写入到外部
                                    if !final_path.starts_with(&plugin.path) {
                                        continue;
                                    }
                                    if e.is_file() {
                                        if let Some(parent_dir) =
                                            final_path.parent()
                                        {
                                            let _ = std::fs::create_dir_all(
                                                parent_dir,
                                            );
                                            if let Ok(l) =
                                                e.read_to_end(&mut buf)
                                            {
                                                let _ = write_file_safe(
                                                    final_path,
                                                    &buf[0..l],
                                                );
                                                buf.clear();
                                            }
                                        }
                                    } else if e.is_dir() {
                                        let _ = std::fs::create_dir_all(
                                            final_path,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            }
        }
    }
}

pub struct Plugin {
    path: PathBuf,
    id: String,
    name: String,
    version: String,
    update_url: String,
    scripts: Vec<PluginScript>,
}

pub struct PluginScript {
    on: String,
    system: String,
    arch: String,
    debug: bool,
    commands: Vec<String>,
}

pub struct PluginUpdateMeta {
    version: String,
    downloads: Vec<PluginDownloadItem>,
}

pub struct PluginDownloadItem {
    system: String,
    arch: String,
    url: String,
}

impl Plugin {
    pub const PLUGIN_MAXIMUM_VERSION: u32 = 1;
    pub const PLUGIN_MINUMUM_VERSION: u32 = 1;

    pub fn from_path(path: impl AsRef<Path>) -> DynResult<Self> {
        let data = std::fs::read_to_string(path.as_ref())?;
        let mut result = Self::from_str(&data)?;
        result.path = PathBuf::from(
            (path
                .as_ref()
                .parent()
                .ok_or_else(|| anyhow::anyhow!("元数据所在路径父文件夹有误"))?
                .to_owned())
            .absolutize()
            .context("无法获取元数据所在路径父文件夹的绝对路径")?,
        );
        Ok(result)
    }

    pub fn from_str(data: &str) -> DynResult<Self> {
        let value = data
            .parse::<JsonValue>()
            .context("无法解析插件元数据 JSON 文件")?;
        Self::from_json(&value)
    }

    pub fn from_json(value: &JsonValue) -> DynResult<Self> {
        if !value.is_object() {
            anyhow::bail!("元数据不是一个合法对象")
        }
        let version = value
            .try_get_into::<f64>("_version")
            .copied()
            .ok_or_else(|| anyhow::anyhow!("元数据没有合法的插件元数据版本"))?
            as u32;
        if version < Self::PLUGIN_MINUMUM_VERSION {
            anyhow::bail!(
                "插件版本过低，此版本的 HiPer Bridge 最低支持 {}，插件元数据版本为 {}",
                Self::PLUGIN_MINUMUM_VERSION,
                version
            );
        }
        if version > Self::PLUGIN_MAXIMUM_VERSION {
            anyhow::bail!(
                "插件版本过高，此版本的 HiPer Bridge 最高支持 {}，插件元数据版本为 {}",
                Self::PLUGIN_MAXIMUM_VERSION,
                version
            );
        }
        let id = value
            .try_get_into::<String>("id")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("元数据没有合法的插件 ID 标识"))?;
        let name = value
            .try_get_into::<String>("name")
            .cloned()
            .unwrap_or_else(|| id.to_owned());
        let plugin_version = value
            .try_get_into::<String>("plugin_version")
            .cloned()
            .unwrap_or_default();
        let update_url = value
            .try_get_into::<String>("update_url")
            .cloned()
            .unwrap_or_default();

        let scripts = if let JsonValue::Object(obj) = value {
            if let Some(JsonValue::Array(arr)) = obj.get("scripts") {
                arr.iter().map(PluginScript::from_json).collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let mut loaded_scripts = Vec::with_capacity(scripts.len());

        for script in scripts {
            loaded_scripts.push(script?);
        }

        Ok(Self {
            id,
            name,
            version: plugin_version,
            update_url,
            scripts: loaded_scripts,
            path: PathBuf::new(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn dispatch_event(&self, event_name: &str) -> Vec<Child> {
        self.scripts
            .iter()
            .filter(|x| x.on == event_name && x.should_run())
            .map(|x| x.run_script(Some(&self.path)))
            .filter_map(|x| x.ok())
            .collect()
    }
}

impl PluginScript {
    pub fn from_json(value: &JsonValue) -> DynResult<Self> {
        let on = value
            .try_get_into::<String>("on")
            .cloned()
            .context("该脚本不存在触发事件")?;
        let system = value
            .try_get_into::<String>("system")
            .cloned()
            .unwrap_or_default();
        let arch = value
            .try_get_into::<String>("arch")
            .cloned()
            .unwrap_or_default();
        let debug = value
            .try_get_into::<bool>("debug")
            .cloned()
            .unwrap_or(false);
        if let JsonValue::Object(obj) = value {
            if let Some(JsonValue::Array(arr)) = obj.get("commands") {
                let commands = arr
                    .iter()
                    .map(|x| x.get::<String>().cloned().unwrap_or_default())
                    .collect();
                return Ok(Self {
                    on,
                    system,
                    arch,
                    commands,
                    debug,
                });
            }
        }
        Ok(Self {
            on,
            system,
            arch,
            debug,
            commands: vec![],
        })
    }

    pub fn should_run(&self) -> bool {
        let system = self.system.is_empty();
        #[cfg(target_os = "windows")]
        let system = system || self.system == "windows";
        #[cfg(target_os = "linux")]
        let system = system || self.system == "linux";
        #[cfg(target_os = "macos")]
        let system = system || self.system == "macos";

        let arch = self.arch.is_empty();
        let arch = arch
            || self.arch
                == match crate::utils::get_system_arch() {
                    crate::utils::Arch::X86 => "x86",
                    crate::utils::Arch::X64 => "x86_64",
                    crate::utils::Arch::ARM64 => "aarch64",
                };

        system && arch
    }

    pub fn run_script(&self, cwd: Option<&Path>) -> DynResult<Child> {
        let mut p = std::process::Command::new({
            #[cfg(target_os = "windows")]
            {
                "cmd.exe"
            }
            #[cfg(target_os = "linux")]
            {
                "bash"
            }
            #[cfg(target_os = "macos")]
            {
                "zsh"
            }
        });
        p.stdin(std::process::Stdio::piped());
        if let Some(cwd) = cwd {
            if cwd.is_dir() {
                p.current_dir(cwd);
            }
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            p.arg("/c").arg(self.commands.join("\n"));
            if !self.debug {
                p.creation_flags(0x08000000);
            }
        }
        #[cfg(target_os = "windows")]
        let p = p.spawn()?;
        #[cfg(not(target_os = "windows"))]
        let mut p = p.spawn()?;
        #[cfg(not(target_os = "windows"))]
        if let Some(stdin) = &mut p.stdin {
            use std::io::Write;
            for line in &self.commands {
                let _ = stdin.write(line.as_bytes());
                let _ = stdin.write(b"\n");
            }
            let _ = stdin.write(b"exit\n");
        }
        Ok(p)
    }
}

impl PluginUpdateMeta {
    pub fn from_str(data: &str) -> DynResult<Self> {
        let value = data
            .parse::<JsonValue>()
            .context("无法解析插件更新元数据 JSON 文件")?;
        Self::from_json(&value)
    }

    pub fn from_json(value: &JsonValue) -> DynResult<Self> {
        let version = value
            .try_get_into::<String>("version")
            .cloned()
            .context("更新元数据版本号不合法")?;
        if let Some(JsonValue::Array(downloads)) = value.try_get("downloads") {
            let mut result = Vec::with_capacity(downloads.len());
            for download in downloads {
                result.push(PluginDownloadItem::from_json(download)?)
            }
            Ok(Self {
                version,
                downloads: result,
            })
        } else {
            Ok(Self {
                version,
                downloads: vec![],
            })
        }
    }
}

impl PluginDownloadItem {
    pub fn from_json(value: &JsonValue) -> DynResult<Self> {
        let url = value
            .try_get_into::<String>("url")
            .cloned()
            .context("下载项不含下载直链")?;
        let system = value
            .try_get_into::<String>("system")
            .cloned()
            .unwrap_or_default();
        let arch = value
            .try_get_into::<String>("arch")
            .cloned()
            .unwrap_or_default();
        Ok(Self { url, system, arch })
    }

    pub fn is_downloadable(&self) -> bool {
        if self.url.is_empty() {
            return false;
        }

        let system = self.system.is_empty();
        #[cfg(target_os = "windows")]
        let system = system || self.system == "windows";
        #[cfg(target_os = "linux")]
        let system = system || self.system == "linux";
        #[cfg(target_os = "macos")]
        let system = system || self.system == "macos";

        let arch = self.arch.is_empty();
        let arch = arch
            || self.arch
                == match crate::utils::get_system_arch() {
                    crate::utils::Arch::X86 => "x86",
                    crate::utils::Arch::X64 => "x86_64",
                    crate::utils::Arch::ARM64 => "aarch64",
                };

        system && arch
    }
}
