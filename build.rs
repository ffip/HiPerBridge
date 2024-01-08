#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn main() {
    use winres::*;
    let mut res = WindowsResource::new();
    res.set_icon("assets/icon-slim.ico");
    res.set_icon_with_id("assets/icon-slim.ico", "ICON");
    #[cfg(not(debug_assertions))]
    res.set_manifest_file("assets/manifest.xml"); // 在这里设置默认管理员权限
    res.compile().unwrap();
}

#[cfg(all(target_os = "windows", not(target_env = "msvc")))]
fn main() {
    update_self_info();
}

#[cfg(not(target_os = "windows"))]
fn main() {}
