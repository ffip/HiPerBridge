use crate::{app_state::AppState, hiper::get_hiper_dir, DynResult};
use std::{collections::HashMap, io::Write, path::PathBuf};
use tinyjson::*;

pub fn get_save_path() -> DynResult<PathBuf> {
    let hiper_path = get_hiper_dir()?;
    Ok(hiper_path.join("hiper-launcher.cfg.bin"))
}

pub fn save_config(app_state: &AppState) {
    if let Ok(save_path) = get_save_path() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(save_path)
        {
            let mut data_hashmap = HashMap::with_capacity(16);

            data_hashmap.insert(
                "token".into(),
                JsonValue::String(app_state.token.to_owned()),
            );
            data_hashmap.insert("use_tun".into(), JsonValue::Boolean(app_state.use_tun));
            data_hashmap.insert(
                "auto_restart".into(),
                JsonValue::Boolean(app_state.auto_restart),
            );
            data_hashmap.insert(
                "debug_mode".into(),
                JsonValue::Boolean(app_state.debug_mode),
            );
            data_hashmap.insert(
                "kill_hiper_when_start".into(),
                JsonValue::Boolean(app_state.kill_hiper_when_start),
            );

            let data = JsonValue::Object(data_hashmap);

            if let Ok(data) = data.stringify() {
                let _ = file.write_all(data.as_bytes());
                let _ = file.sync_all();
            }
        }
    }
}

pub fn load_config(app_state: &mut AppState) {
    if let Ok(save_path) = get_save_path() {
        if save_path.exists() {
            if let Ok(file) = std::fs::read_to_string(save_path) {
                if let Ok(JsonValue::Object(data)) = file.parse::<JsonValue>() {
                    if let Some(Some(token)) = data.get("token").map(|x| x.get::<String>()) {
                        if !token.is_empty() {
                            app_state.token = token.to_owned();
                        }
                    }
                    if let Some(use_tun) = data
                        .get("use_tun")
                        .map(|x| x.get::<bool>().copied().unwrap_or(false))
                    {
                        app_state.use_tun = use_tun;
                    }
                    if let Some(auto_restart) = data
                        .get("auto_restart")
                        .map(|x| x.get::<bool>().copied().unwrap_or(false))
                    {
                        app_state.auto_restart = auto_restart;
                    }
                    if let Some(debug_mode) = data
                        .get("debug_mode")
                        .map(|x| x.get::<bool>().copied().unwrap_or(false))
                    {
                        app_state.debug_mode = debug_mode;
                    }
                    if let Some(kill_hiper_when_start) = data
                        .get("kill_hiper_when_start")
                        .map(|x| x.get::<bool>().copied().unwrap_or(true))
                    {
                        app_state.kill_hiper_when_start = kill_hiper_when_start;
                    }
                }
            }
        }
    }
}
