use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{command, Manager, State};

/// Seconds before the device powers off after a shutdown command.
/// Keep in sync with `WithShutdownLayout.vue`.
const SHUTDOWN_DELAY_SECS: u32 = 20;

/// Generation of the pending delayed shutdown on macOS and Linux, whose `shutdown` command only
/// takes whole minutes. The delay runs inside the app, and bumping the generation cancels it.
struct PendingShutdown(Arc<AtomicU64>);

/// Powers the device off right away on macOS and Linux.
fn power_off_now() {
    let result = if cfg!(target_os = "macos") {
        Command::new("osascript")
            .args(["-e", "tell app \"System Events\" to shut down"])
            .output()
    } else {
        Command::new("systemctl").arg("poweroff").output()
    };

    if let Err(err) = result {
        eprintln!("power off failed: {err}");
    }
}

#[command]
fn trigger_shutdown_state(command: &str, pending: State<PendingShutdown>) -> Result<(), String> {
    let output = match command {
        "sleep" => {
            if cfg!(target_os = "windows") {
                Command::new("rundll32.exe")
                    .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                    .output()
            } else if cfg!(target_os = "macos") {
                Command::new("pmset").arg("sleepnow").output()
            } else if cfg!(target_os = "linux") {
                Command::new("systemctl").arg("suspend").output()
            } else {
                return Err("Unsupported OS".into());
            }
        }

        "hibernate" => {
            if cfg!(target_os = "windows") {
                Command::new("rundll32.exe")
                    .args(["powrprof.dll,SetSuspendState", "1,0,0"])
                    .output()
            } else if cfg!(target_os = "linux") {
                Command::new("systemctl").arg("hibernate").output()
            } else {
                return Err("Unsupported OS".into());
            }
        }

        "shutdown" => {
            if cfg!(target_os = "windows") {
                Command::new("shutdown")
                    .args([
                        "/s",
                        "/f",
                        "/t",
                        &SHUTDOWN_DELAY_SECS.to_string(),
                        "/c",
                        &format!("Komputer anda akan mati dalam {SHUTDOWN_DELAY_SECS} detik :)"),
                    ])
                    .output()
            } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
                let generation = pending.0.fetch_add(1, Ordering::SeqCst) + 1;
                let current = Arc::clone(&pending.0);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(u64::from(SHUTDOWN_DELAY_SECS)));
                    if current.load(Ordering::SeqCst) == generation {
                        power_off_now();
                    }
                });
                return Ok(());
            } else {
                return Err("Unsupported OS".into());
            }
        }

        "cancelShutdown" => {
            if cfg!(target_os = "windows") {
                Command::new("shutdown").arg("/a").output()
            } else if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
                pending.0.fetch_add(1, Ordering::SeqCst);
                return Ok(());
            } else {
                return Err("Unsupported OS".into());
            }
        }

        _ => return Err("Unknown command".into()),
    };

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
        Err(err) => Err(err.to_string()),
    }
}

#[command]
fn closing_app(_code: u8) {
    std::process::exit(0);
}

#[tauri::command]
fn validate_input(user_input: String) -> bool {
    user_input == "selm1945c3pu;"
}

/// Reads an on/off flag from `.env`: only an explicit `false` or `0` turns it off.
fn flag_enabled(value: Option<&str>) -> bool {
    !matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("false" | "0")
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PendingShutdown(Arc::new(AtomicU64::new(0))))
        .setup(|app| {
            // tauri.conf.json already sets contentProtected to true, so only an explicit
            // VITE_CONTENT_PROTECTION=false in .env switches it off.
            if !flag_enabled(option_env!("VITE_CONTENT_PROTECTION")) {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_content_protected(false)?;
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            trigger_shutdown_state,
            closing_app,
            validate_input
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
