use tinyjson::*;

pub fn try_get_log_line(line: &str) -> Option<(String, String, String)> {
    if let Ok(JsonValue::Object(log_data)) = line.parse::<JsonValue>() {
        let level;
        if let Some(JsonValue::String(level_data)) = log_data.get("level") {
            level = level_data.to_owned();
        } else {
            return None;
        }
        let error = if let Some(JsonValue::String(error_data)) = log_data.get("error") {
            error_data.to_owned()
        } else {
            "".into()
        };
        if let Some(JsonValue::String(message_data)) = log_data.get("msg") {
            return Some((level, message_data.to_owned(), error));
        } else {
            return None;
        }
    }
    None
}

pub fn try_get_ipv4(line: &str) -> Option<String> {
    if let Ok(JsonValue::Object(log_data)) = line.parse::<JsonValue>() {
        if let Some(JsonValue::Object(network_data)) = log_data.get("network") {
            if let Some(JsonValue::String(ip_data)) = network_data.get("IP") {
                return Some(ip_data.to_owned());
            }
        }
    }
    None
}

pub fn try_get_valid(line: &str) -> Option<String> {
    if let Ok(JsonValue::Object(log_data)) = line.parse::<JsonValue>() {
        if let Some(JsonValue::String(valid_data)) = log_data.get("valid") {
            return Some(valid_data.to_owned());
        }
    }
    None
}