use serde::*;
use std::path::PathBuf;

use crate::{app_state::AppState, hiper::get_hiper_dir, DynResult};

#[derive(Clone, Deserialize, Serialize)]
struct Config {
    pub auto_restart: bool,
    pub use_tun: bool,
    pub token: String,
}

pub fn get_save_path() -> DynResult<PathBuf> {
    let hiper_path = get_hiper_dir()?;
    Ok(hiper_path.join("hiper-launcher.cfg.bin"))
}

pub fn save_config(app_state: &AppState) {
    if let Ok(save_path) = get_save_path() {
        let config = Config {
            auto_restart: app_state.auto_restart,
            use_tun: app_state.use_tun,
            token: app_state.inner_token.to_owned(),
        };
        if let Ok(file) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(save_path)
        {
            let _ = serde_json::to_writer(file, &config);
        }
    }
}

pub fn load_config(app_state: &mut AppState) {
    if let Ok(save_path) = get_save_path() {
        if save_path.exists() {
            if let Ok(file) = std::fs::read(save_path) {
                if let Ok(data) = serde_json::from_slice::<Config>(&file) {
                    if !data.token.is_empty() {
                        app_state.inner_token = data.token;
                        app_state.token = "••••••••".into();
                    }
                    app_state.use_tun = data.use_tun;
                    app_state.auto_restart = data.auto_restart;
                }
            }
        }
    }
}
