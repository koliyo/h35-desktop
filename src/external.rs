use std::process::Command;

use url::{Host, Url};

pub fn allow_webview_navigation(url: &str) -> bool {
    if should_open_externally(url) {
        open_system_url(url);
        false
    } else {
        true
    }
}

pub fn should_open_externally(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    match parsed.scheme() {
        "mailto" | "tel" => true,
        "http" | "https" => !host_is_loopback(parsed.host()),
        _ => false,
    }
}

pub fn open_system_url(url: &str) {
    if let Err(error) = spawn_system_open(url) {
        tracing::error!(%error, %url, "failed to open URL in system browser");
    }
}

fn host_is_loopback(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(addr)) => addr.is_loopback(),
        Some(Host::Ipv6(addr)) => addr.is_loopback(),
        None => false,
    }
}

fn spawn_system_open(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_open_externally;

    #[test]
    fn loopback_preview_stays_in_webview() {
        assert!(!should_open_externally("http://127.0.0.1:8000/docs"));
        assert!(!should_open_externally("http://localhost:5173/"));
        assert!(!should_open_externally("https://[::1]/"));
        assert!(!should_open_externally("file:///tmp/index.html"));
        assert!(!should_open_externally("about:blank"));
    }

    #[test]
    fn generic_web_and_mail_links_open_externally() {
        assert!(should_open_externally("https://github.com/koliyo/okmate"));
        assert!(should_open_externally("http://example.com/path"));
        assert!(should_open_externally("mailto:dev@example.com"));
    }
}
