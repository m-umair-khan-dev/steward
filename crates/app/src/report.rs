//! "Report a Bug" (P6-T13): opens the issue tracker in the browser with the
//! Steward version and Ubuntu release filled in. Nothing is sent until the
//! person submits the report themselves.

/// The new-issue address, with a title and body to fill in.
pub fn issue_url() -> String {
    let release = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    let body = format!(
        "**Steward:** {}\n**System:** {release}\n\n**What I did:**\n\n**What I expected:**\n\n**What happened:**\n",
        env!("CARGO_PKG_VERSION")
    );
    format!(
        "{}/issues/new?body={}",
        env!("CARGO_PKG_REPOSITORY"),
        escape(&body)
    )
}

pub fn open() -> String {
    let url = issue_url();
    if let Err(error) = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>) {
        glib::g_warning!("steward", "could not open the issue tracker: {error}");
    }
    url
}

/// Percent-encodes everything but unreserved characters (RFC 3986).
fn escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
