use std::fs;

/// `.env` keys that the Rust side reads at build time. A missing key leaves the feature on.
const ENV_KEYS: [&str; 1] = ["VITE_CONTENT_PROTECTION"];

/// Returns the value of `key` from the `.env` file content, with surrounding quotes removed.
fn read_env_value(content: &str, key: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == key)
        .map(|(_, value)| value.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
}

fn main() {
    println!("cargo:rerun-if-changed=../.env");

    if let Ok(content) = fs::read_to_string("../.env") {
        for key in ENV_KEYS {
            if let Some(value) = read_env_value(&content, key) {
                println!("cargo:rustc-env={key}={value}");
            }
        }
    }

    tauri_build::build()
}
