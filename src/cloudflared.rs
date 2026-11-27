use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::{BinarySource, BinaryStatus};

#[cfg(windows)]
const BINARY_NAME: &str = "cloudflared.exe";

#[cfg(not(windows))]
const BINARY_NAME: &str = "cloudflared";

const TRYCLOUDFLARE_SUFFIX: &str = ".trycloudflare.com";
const LOOPBACK_PROBE_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOrigin {
    pub dial_url: String,
    pub no_tls_verify: bool,
    pub origin_server_name: Option<&'static str>,
}

pub fn managed_binary_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("quicktunnel").join("bin").join(BINARY_NAME))
}

pub fn detect_binary() -> BinaryStatus {
    if let Some(path) = find_in_path() {
        let version = probe_binary_version(&path);
        return BinaryStatus::Ready {
            path,
            source: BinarySource::SystemPath,
            version,
        };
    }

    if let Some(path) = managed_binary_path()
        && is_executable_file(&path)
    {
        let version = probe_binary_version(&path);
        return BinaryStatus::Ready {
            path,
            source: BinarySource::ManagedLocal,
            version,
        };
    }

    BinaryStatus::Missing
}

pub fn download_url_for_current_platform() -> Result<&'static str, String> {
    download_url_for(env::consts::OS, env::consts::ARCH)
}

fn download_url_for(os: &str, arch: &str) -> Result<&'static str, String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok(
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64",
        ),
        ("linux", "aarch64") => Ok(
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64",
        ),
        ("windows", "x86_64") | ("windows", "aarch64") => Ok(
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe",
        ),
        ("windows", "x86") => Ok(
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-386.exe",
        ),
        _ => Err(format!(
            "Automatic cloudflared download is not supported on {os}/{arch}. Please install cloudflared manually and ensure it is on your PATH."
        )),
    }
}

pub fn install_cloudflared(on_progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    let url = download_url_for_current_platform()?;
    let dest_path = managed_binary_path().ok_or_else(|| {
        "Could not determine a local data directory for storing cloudflared.".to_string()
    })?;
    let parent = dest_path
        .parent()
        .ok_or_else(|| "Invalid managed binary destination path.".to_string())?;

    fs::create_dir_all(parent).map_err(|e| {
        format!(
            "Failed to create binary directory {}: {e}",
            parent.display()
        )
    })?;

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_path = parent.join(format!(
        ".{BINARY_NAME}.download-{}-{unique}.tmp",
        std::process::id()
    ));

    let download_result = (|| -> Result<(), String> {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .build();
        let agent: ureq::Agent = config.into();

        let mut response = agent
            .get(url)
            .header("User-Agent", "QuickTunnel/0.1")
            .call()
            .map_err(|e| format!("Failed to download cloudflared: {e}"))?;

        let total_bytes = response.body().content_length();
        on_progress(0, total_bytes);

        let mut file = File::create(&temp_path)
            .map_err(|e| format!("Failed to create temporary file: {e}"))?;

        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;

        loop {
            let bytes_read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(format!("Failed while downloading cloudflared: {e}"));
                }
            };

            file.write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Failed writing cloudflared to disk: {e}"))?;
            downloaded = downloaded.saturating_add(bytes_read as u64);
            on_progress(downloaded, total_bytes);
        }

        if downloaded == 0 {
            return Err("Downloaded cloudflared binary was empty.".to_string());
        }

        file.flush()
            .map_err(|e| format!("Failed flushing downloaded binary: {e}"))?;
        drop(file);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to mark cloudflared as executable: {e}"))?;
        }

        #[cfg(windows)]
        if dest_path.exists() {
            let _ = fs::remove_file(&dest_path);
        }

        fs::rename(&temp_path, &dest_path)
            .map_err(|e| format!("Failed to move cloudflared into place: {e}"))?;

        Ok(())
    })();

    if let Err(err) = download_result {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }

    Ok(dest_path)
}

pub fn normalize_local_address(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Please enter a local address or port.".to_string());
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err("Local address cannot contain spaces.".to_string());
    }

    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        let port = parse_port(trimmed)?;
        return Ok(format!("http://localhost:{port}"));
    }

    if let Some(port_str) = trimmed.strip_prefix(':') {
        if port_str.starts_with(':') {
            return Err(
                "IPv6 addresses must be wrapped in brackets, e.g. [::1]:8000.".to_string(),
            );
        }
        let port = parse_port(port_str)?;
        return Ok(format!("http://localhost:{port}"));
    }

    if let Some((scheme, rest)) = trimmed.split_once("://") {
        let scheme_lower = scheme.to_ascii_lowercase();
        if scheme_lower != "http" && scheme_lower != "https" {
            return Err(format!(
                "Unsupported scheme '{scheme}://'. Use http:// or https://."
            ));
        }

        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        validate_authority(authority)?;
        return Ok(format!("{scheme_lower}://{rest}"));
    }

    let authority = trimmed.split(['/', '?', '#']).next().unwrap_or("");
    validate_authority(authority)?;
    Ok(format!("http://{trimmed}"))
}

pub(crate) fn resolve_origin_target(target_url: &str) -> ResolvedOrigin {
    resolve_origin_with_probe(target_url, cfg!(windows), tcp_loopback_open)
}

fn tcp_loopback_open(ip: IpAddr, port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::new(ip, port), LOOPBACK_PROBE_TIMEOUT).is_ok()
}

fn resolve_origin_with_probe(
    target_url: &str,
    prefer_ipv4_when_unbound: bool,
    mut is_open: impl FnMut(IpAddr, u16) -> bool,
) -> ResolvedOrigin {
    let Some((scheme, after_scheme)) = target_url.split_once("://") else {
        return ResolvedOrigin {
            dial_url: target_url.to_string(),
            no_tls_verify: false,
            origin_server_name: None,
        };
    };

    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let suffix = &after_scheme[authority.len()..];

    let (userinfo_prefix, host_port) = match authority.rsplit_once('@') {
        Some((user, hp)) => (&authority[..user.len() + 1], hp),
        None => ("", authority),
    };

    let (host, explicit_port) = if host_port.starts_with('[') {
        match host_port.find(']') {
            Some(end_bracket) => {
                let h = &host_port[..=end_bracket];
                let port = host_port[end_bracket + 1..]
                    .strip_prefix(':')
                    .and_then(|p| p.parse::<u16>().ok());
                (h, port)
            }
            None => (host_port, None),
        }
    } else if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>().ok())
    } else {
        (host_port, None)
    };

    let is_localhost = host.eq_ignore_ascii_case("localhost");
    let is_ipv4_loopback = host.parse::<Ipv4Addr>().is_ok_and(|ip| ip.is_loopback());
    let is_ipv6_loopback = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .and_then(|s| s.parse::<Ipv6Addr>().ok())
        .is_some_and(|ip| ip.is_loopback());
    let is_loopback = is_localhost || is_ipv4_loopback || is_ipv6_loopback;

    // On Windows, `localhost` often resolves to [::1] before 127.0.0.1, causing 502s when local servers bind IPv4-only.
    let resolved_host = if is_localhost {
        let port = explicit_port.unwrap_or(if scheme == "https" { 443 } else { 80 });
        if is_open(IpAddr::V4(Ipv4Addr::LOCALHOST), port) {
            "127.0.0.1"
        } else if is_open(IpAddr::V6(Ipv6Addr::LOCALHOST), port) {
            "[::1]"
        } else if prefer_ipv4_when_unbound {
            "127.0.0.1"
        } else {
            host
        }
    } else {
        host
    };

    let dial_url = if resolved_host == host {
        target_url.to_string()
    } else {
        match explicit_port {
            Some(port) => format!("{scheme}://{userinfo_prefix}{resolved_host}:{port}{suffix}"),
            None => format!("{scheme}://{userinfo_prefix}{resolved_host}{suffix}"),
        }
    };

    let no_tls_verify = scheme == "https" && is_loopback;
    let origin_server_name =
        if no_tls_verify && is_localhost && !resolved_host.eq_ignore_ascii_case("localhost") {
            Some("localhost")
        } else {
            None
        };

    ResolvedOrigin {
        dial_url,
        no_tls_verify,
        origin_server_name,
    }
}

pub(crate) fn build_tunnel_args(origin: &ResolvedOrigin, is_windows: bool) -> Vec<String> {
    let mut args = vec![
        "tunnel".to_string(),
        "--url".to_string(),
        origin.dial_url.clone(),
        "--no-autoupdate".to_string(),
        "--config".to_string(),
        String::new(),
        "--metrics".to_string(),
        "127.0.0.1:0".to_string(),
    ];
    // Windows Defender Firewall and many networks block/delay outbound UDP 7844 (QUIC).
    if is_windows {
        args.push("--protocol".to_string());
        args.push("http2".to_string());
    }
    if origin.no_tls_verify {
        args.push("--no-tls-verify".to_string());
    }
    if let Some(sni) = origin.origin_server_name {
        args.push("--origin-server-name".to_string());
        args.push(sni.to_string());
    }
    args
}

pub fn extract_tunnel_url(line: &str) -> Option<String> {
    let cleaned = strip_ansi(line);
    let lower = cleaned.to_ascii_lowercase();
    let mut search_start = 0;

    while let Some(rel_pos) = lower[search_start..].find("https://") {
        let start = search_start + rel_pos;
        let after_scheme = start + "https://".len();
        let rest = &cleaned[after_scheme..];

        let host_len = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'.')
            .count();

        let raw_host = &rest[..host_len];
        let trimmed_host = raw_host.trim_end_matches('.');
        let trailing_after_host = &rest[trimmed_host.len()..];

        if is_valid_trycloudflare_host(trimmed_host) && !has_non_empty_path(trailing_after_host) {
            return Some(format!("https://{trimmed_host}"));
        }

        search_start = after_scheme;
    }

    None
}

pub(crate) fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => break,
            }
        } else {
            out.push(ch);
        }
    }

    out
}

pub(crate) fn probe_binary_version(path: &Path) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_command(&mut cmd);

    let output = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_version_output(&stdout).or_else(|| parse_version_output(&stderr))
}

#[cfg(windows)]
pub(crate) fn configure_background_command(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn configure_background_command(_cmd: &mut Command) {}

fn parse_version_output(raw: &str) -> Option<String> {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let lower = line.to_ascii_lowercase();
    if let Some(idx) = lower.find("version ") {
        let after = line[idx + "version ".len()..].trim();
        if let Some(token) = after.split_whitespace().next()
            && !token.is_empty()
        {
            return Some(token.to_string());
        }
    }

    let short: String = line.chars().take(48).collect();
    if short.is_empty() {
        None
    } else {
        Some(short)
    }
}

fn find_in_path() -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(BINARY_NAME);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        (metadata.permissions().mode() & 0o111) != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn parse_port(raw: &str) -> Result<u16, String> {
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
        return Err("Port must be a number between 1 and 65535.".to_string());
    }
    match raw.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err("Port must be a number between 1 and 65535.".to_string()),
    }
}

fn validate_authority(authority: &str) -> Result<(), String> {
    if authority.is_empty() {
        return Err(
            "Address is missing a host (for example, localhost:3000 or http://localhost:3000)."
                .to_string(),
        );
    }

    let host_port = match authority.rsplit_once('@') {
        Some((_, hp)) => hp,
        None => authority,
    };

    if host_port.is_empty() {
        return Err(
            "Address is missing a host (for example, localhost:3000 or http://localhost:3000)."
                .to_string(),
        );
    }

    if let Some(rest) = host_port.strip_prefix('[') {
        let Some((ipv6_host, after_bracket)) = rest.split_once(']') else {
            return Err("Invalid IPv6 address: missing closing ']'.".to_string());
        };
        if ipv6_host.is_empty() {
            return Err("Invalid IPv6 address: host inside brackets cannot be empty.".to_string());
        }
        if !after_bracket.is_empty() {
            let Some(port_str) = after_bracket.strip_prefix(':') else {
                return Err(format!(
                    "Invalid host syntax near '{after_bracket}'."
                ));
            };
            parse_port(port_str)?;
        }
        return Ok(());
    }

    if host_port.contains('[') || host_port.contains(']') {
        return Err("Invalid brackets in host address.".to_string());
    }

    if let Some((host, port_str)) = host_port.split_once(':') {
        if port_str.contains(':') {
            return Err(
                "IPv6 addresses must be wrapped in brackets, e.g. [::1]:8000.".to_string(),
            );
        }
        if host.is_empty() {
            return Err(
                "Address is missing a host before the port (for example, localhost:3000)."
                    .to_string(),
            );
        }
        parse_port(port_str)?;
        return Ok(());
    }

    Ok(())
}

fn is_valid_trycloudflare_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    let Some(subdomain) = lower.strip_suffix(TRYCLOUDFLARE_SUFFIX) else {
        return false;
    };

    if subdomain.is_empty() || subdomain == "api" {
        return false;
    }

    subdomain.split('.').all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn has_non_empty_path(trailing: &str) -> bool {
    if let Some(after_slash) = trailing.strip_prefix('/') {
        after_slash
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '%'))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_port_only_inputs() {
        assert_eq!(
            normalize_local_address("3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_local_address(":3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_local_address("  :8080  ").unwrap(),
            "http://localhost:8080"
        );
        assert_eq!(
            normalize_local_address("65535").unwrap(),
            "http://localhost:65535"
        );
    }

    #[test]
    fn normalize_host_and_port_without_scheme() {
        assert_eq!(
            normalize_local_address("localhost:8080").unwrap(),
            "http://localhost:8080"
        );
        assert_eq!(
            normalize_local_address("127.0.0.1:5173").unwrap(),
            "http://127.0.0.1:5173"
        );
        assert_eq!(
            normalize_local_address("[::1]:8000").unwrap(),
            "http://[::1]:8000"
        );
        assert_eq!(
            normalize_local_address("localhost:3000/api").unwrap(),
            "http://localhost:3000/api"
        );
    }

    #[test]
    fn normalize_preserves_http_and_https() {
        assert_eq!(
            normalize_local_address("http://localhost:3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_local_address("https://localhost:8443").unwrap(),
            "https://localhost:8443"
        );
        assert_eq!(
            normalize_local_address("HTTPS://127.0.0.1:8443/app").unwrap(),
            "https://127.0.0.1:8443/app"
        );
        assert_eq!(
            normalize_local_address("https://[::1]:8443").unwrap(),
            "https://[::1]:8443"
        );
    }

    #[test]
    fn normalize_rejects_invalid_inputs() {
        assert!(normalize_local_address("").is_err());
        assert!(normalize_local_address("   ").is_err());
        assert!(normalize_local_address("localhost 3000").is_err());
        assert!(normalize_local_address("http://local host:3000").is_err());
        assert!(normalize_local_address("0").is_err());
        assert!(normalize_local_address(":0").is_err());
        assert!(normalize_local_address("65536").is_err());
        assert!(normalize_local_address(":70000").is_err());
        assert!(normalize_local_address(":abc").is_err());
        assert!(normalize_local_address("http://").is_err());
        assert!(normalize_local_address("https:///path").is_err());
        assert!(normalize_local_address("http://:3000").is_err());
        assert!(normalize_local_address("http://localhost:99999").is_err());
        assert!(normalize_local_address("ftp://localhost:21").is_err());
        assert!(normalize_local_address("ssh://localhost:22").is_err());
        assert!(normalize_local_address("::1:8000").is_err());
        assert!(normalize_local_address("[::1").is_err());
    }

    #[test]
    fn extract_url_from_cloudflared_banner_and_variants() {
        assert_eq!(
            extract_tunnel_url("2025-02-27T12:00:00Z INF |  https://gentle-river-1234.trycloudflare.com  |"),
            Some("https://gentle-river-1234.trycloudflare.com".to_string())
        );
        assert_eq!(
            extract_tunnel_url("Your quick Tunnel is live at \"https://alpha-beta.trycloudflare.com\"."),
            Some("https://alpha-beta.trycloudflare.com".to_string())
        );
        assert_eq!(
            extract_tunnel_url("Ready: https://clean-sky.trycloudflare.com/"),
            Some("https://clean-sky.trycloudflare.com".to_string())
        );
        assert_eq!(
            extract_tunnel_url("\x1b[32m|  https://colored-output.trycloudflare.com  |\x1b[0m"),
            Some("https://colored-output.trycloudflare.com".to_string())
        );
        assert_eq!(
            extract_tunnel_url("URL is https://trailing-dot.trycloudflare.com."),
            Some("https://trailing-dot.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn extract_url_ignores_non_tunnel_lines() {
        assert_eq!(
            extract_tunnel_url("Requesting new quick Tunnel on trycloudflare.com..."),
            None
        );
        assert_eq!(
            extract_tunnel_url(
                "ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\": i/o timeout"
            ),
            None
        );
        assert_eq!(extract_tunnel_url("https://trycloudflare.com"), None);
        assert_eq!(extract_tunnel_url("https://.trycloudflare.com"), None);
        assert_eq!(
            extract_tunnel_url("http://insecure.trycloudflare.com"),
            None
        );
        assert_eq!(
            extract_tunnel_url("https://fake.trycloudflare.com.example.com"),
            None
        );
    }

    #[test]
    fn parse_version_extracts_short_version() {
        assert_eq!(
            parse_version_output("cloudflared version 2025.2.1 (built 2025-02-27-1432 UTC)\n"),
            Some("2025.2.1".to_string())
        );
        assert_eq!(parse_version_output("   \n  "), None);
    }

    #[test]
    fn download_urls_cover_linux_and_windows_targets() {
        assert!(download_url_for("linux", "x86_64").unwrap().ends_with("cloudflared-linux-amd64"));
        assert!(download_url_for("linux", "aarch64").unwrap().ends_with("cloudflared-linux-arm64"));
        assert!(download_url_for("windows", "x86_64").unwrap().ends_with("cloudflared-windows-amd64.exe"));
        assert!(download_url_for("windows", "x86").unwrap().ends_with("cloudflared-windows-386.exe"));
        assert!(download_url_for("windows", "aarch64").unwrap().ends_with("cloudflared-windows-amd64.exe"));
        assert!(download_url_for("freebsd", "x86_64").is_err());
    }

    #[test]
    fn resolve_origin_probes_ipv4_and_ipv6_for_localhost() {
        let ipv4_only = |ip: IpAddr, port: u16| ip == IpAddr::V4(Ipv4Addr::LOCALHOST) && port == 3000;
        let res = resolve_origin_with_probe("http://localhost:3000", false, ipv4_only);
        assert_eq!(res.dial_url, "http://127.0.0.1:3000");
        assert!(!res.no_tls_verify);
        assert_eq!(res.origin_server_name, None);

        let ipv6_only = |ip: IpAddr, port: u16| ip == IpAddr::V6(Ipv6Addr::LOCALHOST) && port == 5173;
        let res = resolve_origin_with_probe("http://localhost:5173", true, ipv6_only);
        assert_eq!(res.dial_url, "http://[::1]:5173");

        let both_open = |_ip: IpAddr, port: u16| port == 3000;
        let res = resolve_origin_with_probe("http://localhost:3000", false, both_open);
        assert_eq!(res.dial_url, "http://127.0.0.1:3000");

        let none_open = |_ip: IpAddr, _port: u16| false;
        let res_win = resolve_origin_with_probe("http://localhost:3000", true, none_open);
        assert_eq!(res_win.dial_url, "http://127.0.0.1:3000");

        let res_unix = resolve_origin_with_probe("http://localhost:3000", false, none_open);
        assert_eq!(res_unix.dial_url, "http://localhost:3000");

        let port_80 = |ip: IpAddr, port: u16| ip == IpAddr::V4(Ipv4Addr::LOCALHOST) && port == 80;
        let res_default_port = resolve_origin_with_probe("http://localhost/api", false, port_80);
        assert_eq!(res_default_port.dial_url, "http://127.0.0.1/api");

        let port_8080 = |ip: IpAddr, port: u16| ip == IpAddr::V4(Ipv4Addr::LOCALHOST) && port == 8080;
        let res_userinfo =
            resolve_origin_with_probe("http://user:pass@localhost:8080/x?a=1", false, port_8080);
        assert_eq!(res_userinfo.dial_url, "http://user:pass@127.0.0.1:8080/x?a=1");
    }

    #[test]
    fn resolve_origin_sets_tls_flags_only_for_loopback_https() {
        let ipv4_8443 = |ip: IpAddr, port: u16| ip == IpAddr::V4(Ipv4Addr::LOCALHOST) && port == 8443;
        let res_localhost = resolve_origin_with_probe("https://localhost:8443", true, ipv4_8443);
        assert_eq!(res_localhost.dial_url, "https://127.0.0.1:8443");
        assert!(res_localhost.no_tls_verify);
        assert_eq!(res_localhost.origin_server_name, Some("localhost"));

        let res_v4 = resolve_origin_with_probe("https://127.0.0.1:8443", true, ipv4_8443);
        assert_eq!(res_v4.dial_url, "https://127.0.0.1:8443");
        assert!(res_v4.no_tls_verify);
        assert_eq!(res_v4.origin_server_name, None);

        let res_v6 = resolve_origin_with_probe("https://[::1]:8443", true, ipv4_8443);
        assert_eq!(res_v6.dial_url, "https://[::1]:8443");
        assert!(res_v6.no_tls_verify);
        assert_eq!(res_v6.origin_server_name, None);

        let res_lan = resolve_origin_with_probe("https://192.168.1.50:8443", true, ipv4_8443);
        assert_eq!(res_lan.dial_url, "https://192.168.1.50:8443");
        assert!(!res_lan.no_tls_verify);
        assert_eq!(res_lan.origin_server_name, None);
    }

    #[test]
    fn resolve_origin_detects_live_tcp_listener() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        let res = resolve_origin_target(&format!("http://localhost:{port}"));
        assert_eq!(res.dial_url, format!("http://127.0.0.1:{port}"));
        drop(listener);
    }

    #[test]
    fn build_tunnel_args_includes_platform_and_tls_flags() {
        let linux_http = ResolvedOrigin {
            dial_url: "http://localhost:3000".to_string(),
            no_tls_verify: false,
            origin_server_name: None,
        };
        let linux_args = build_tunnel_args(&linux_http, false);
        assert_eq!(&linux_args[..3], &["tunnel", "--url", "http://localhost:3000"]);
        assert_eq!(
            linux_args,
            vec![
                "tunnel",
                "--url",
                "http://localhost:3000",
                "--no-autoupdate",
                "--config",
                "",
                "--metrics",
                "127.0.0.1:0",
            ]
        );

        let win_http = ResolvedOrigin {
            dial_url: "http://127.0.0.1:3000".to_string(),
            no_tls_verify: false,
            origin_server_name: None,
        };
        let win_http_args = build_tunnel_args(&win_http, true);
        assert_eq!(&win_http_args[..3], &["tunnel", "--url", "http://127.0.0.1:3000"]);
        assert_eq!(
            win_http_args,
            vec![
                "tunnel",
                "--url",
                "http://127.0.0.1:3000",
                "--no-autoupdate",
                "--config",
                "",
                "--metrics",
                "127.0.0.1:0",
                "--protocol",
                "http2",
            ]
        );

        let win_https = ResolvedOrigin {
            dial_url: "https://127.0.0.1:8443".to_string(),
            no_tls_verify: true,
            origin_server_name: Some("localhost"),
        };
        let win_https_args = build_tunnel_args(&win_https, true);
        assert_eq!(&win_https_args[..3], &["tunnel", "--url", "https://127.0.0.1:8443"]);
        assert_eq!(
            win_https_args,
            vec![
                "tunnel",
                "--url",
                "https://127.0.0.1:8443",
                "--no-autoupdate",
                "--config",
                "",
                "--metrics",
                "127.0.0.1:0",
                "--protocol",
                "http2",
                "--no-tls-verify",
                "--origin-server-name",
                "localhost",
            ]
        );
    }
}
