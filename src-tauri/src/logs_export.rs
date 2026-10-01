//! Settings -> Help -> Send logs (PLAN.md "Contact and send logs"): zips
//! the last week of logs plus a `system.txt` into Downloads for the user to
//! send by hand. Nothing is uploaded. The Windows profile path and account
//! name are scrubbed from the zipped copies; the files on disk are untouched.

use std::io::Write;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::player::PlayerState;

/// Daily log files included, newest first.
const MAX_LOG_FILES: usize = 7;

/// Replaces the profile path (as written in logs: `\`, `/`, and
/// JSON-escaped `\\`) with `%USERPROFILE%` and the bare account name with
/// `<user>`.
fn scrub(text: &str, profile: Option<&str>, user: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(profile) = profile.filter(|p| !p.is_empty()) {
        let variants = [profile.to_string(), profile.replace('\\', "/"), profile.replace('\\', "\\\\")];
        for variant in variants {
            out = replace_ignore_case(&out, &variant, "%USERPROFILE%");
        }
    }
    if let Some(user) = user.filter(|u| u.len() >= 2) {
        out = replace_ignore_case(&out, user, "<user>");
    }
    out
}

/// ASCII-case-insensitive replace (Windows paths aren't case-sensitive).
fn replace_ignore_case(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let lower_hay = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0;
    for (index, _) in lower_hay.match_indices(&lower_needle) {
        if index < last {
            continue;
        }
        out.push_str(&haystack[last..index]);
        out.push_str(replacement);
        last = index + needle.len();
    }
    out.push_str(&haystack[last..]);
    out
}

fn windows_version() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// No console window flashing up from the GUI app.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(output) = std::process::Command::new("cmd").args(["/C", "ver"]).creation_flags(CREATE_NO_WINDOW).output() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    std::env::consts::OS.to_string()
}

/// Zips the logs + `system.txt` into Downloads and returns the zip's path.
#[tauri::command]
pub async fn export_logs(app: AppHandle, player: State<'_, PlayerState>) -> Result<String, String> {
    tracing::info!("export_logs invoked");
    let mpv_available = tokio::task::spawn_blocking(mpv_player::is_available).await.unwrap_or(false);
    let mpv_version = crate::player::mpv_version(&player).await;
    let system = format!(
        "app version: {}\nos: {}\narch: {}\nmpv available: {mpv_available}\nlibmpv version: {}\n",
        app.package_info().version,
        windows_version(),
        std::env::consts::ARCH,
        mpv_version.as_deref().unwrap_or("unknown (player not started)"),
    );

    let result = tokio::task::spawn_blocking(move || write_zip(&system)).await.map_err(|err| err.to_string())?;
    match result {
        Ok((path, bytes)) => {
            tracing::info!(path = %path.display(), bytes, "logs exported");
            Ok(path.to_string_lossy().into_owned())
        }
        Err(err) => {
            tracing::error!(%err, "export_logs failed");
            Err(format!("Couldn't export the logs: {err}"))
        }
    }
}

fn write_zip(system: &str) -> anyhow::Result<(PathBuf, u64)> {
    let profile = dirs::home_dir().map(|p| p.to_string_lossy().into_owned());
    let user = std::env::var("USERNAME").ok();
    let scrub = |text: &str| scrub(text, profile.as_deref(), user.as_deref());

    let mut logs: Vec<PathBuf> = std::fs::read_dir(crate::log_dir())?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    // Daily files carry their date in the name, so name order is age order.
    logs.sort();
    logs.reverse();
    logs.truncate(MAX_LOG_FILES);
    tracing::debug!(files = logs.len(), "export_logs collecting log files");

    let downloads = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let date = chrono_date();
    let path = downloads.join(format!("nyaa-stream-logs-{date}.zip"));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("system.txt", options)?;
    zip.write_all(scrub(system).as_bytes())?;
    for log in &logs {
        let name = log.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let text = String::from_utf8_lossy(&std::fs::read(log)?).into_owned();
        zip.start_file(scrub(&name), options)?;
        zip.write_all(scrub(&text).as_bytes())?;
    }
    zip.finish()?;
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok((path, bytes))
}

/// `YYYY-MM-DD` (UTC) without a date crate.
fn chrono_date() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubs_profile_and_user() {
        let text = r#"dir=C:\Users\Alice\AppData x=c:/users/alice/y json="C:\\Users\\Alice\\z" by Alice"#;
        let out = scrub(text, Some(r"C:\Users\Alice"), Some("Alice"));
        assert_eq!(out, r#"dir=%USERPROFILE%\AppData x=%USERPROFILE%/y json="%USERPROFILE%\\z" by <user>"#);
    }
}
