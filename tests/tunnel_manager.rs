use std::path::PathBuf;

use quicktunnel::tunnel::TunnelManager;

#[test]
fn start_tunnel_rejects_invalid_addresses_before_spawning() {
    let manager = TunnelManager::with_binary_path(PathBuf::from("nonexistent-cloudflared"), None);

    assert!(manager.start_tunnel("").is_err());
    assert!(manager.start_tunnel("   ").is_err());
    assert!(manager.start_tunnel("localhost 3000").is_err());
    assert!(manager.start_tunnel("ftp://localhost:21").is_err());
    assert!(manager.tunnels().is_empty());
    assert_eq!(manager.active_count(), 0);
}

#[test]
fn stop_unknown_tunnel_returns_error() {
    let manager = TunnelManager::with_binary_path(PathBuf::from("nonexistent-cloudflared"), None);
    assert!(manager.stop_tunnel(999).is_err());
}

#[cfg(unix)]
mod unix_integration {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use quicktunnel::model::{TunnelInfo, TunnelStatus};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(1);
    /// Prevents Linux ETXTBSY ("Text file busy") when parallel test threads
    /// fork/exec while another thread holds a newly created executable script FD open.
    static SCRIPT_IO_LOCK: Mutex<()> = Mutex::new(());

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let n = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "quicktunnel-test-{prefix}-{}-{n}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp test dir");
            Self { path }
        }

        fn write_script(&self, name: &str, body: &str) -> PathBuf {
            let _guard = SCRIPT_IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let script_path = self.path.join(name);
            let mut file = File::create(&script_path).expect("create mock script");
            file.write_all(body.as_bytes()).expect("write mock script");
            file.sync_all().expect("sync mock script");
            drop(file);
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
                .expect("chmod +x mock script");
            script_path
        }
    }

    fn start_tunnel_synced(manager: &TunnelManager, addr: &str) -> Result<u64, String> {
        let _guard = SCRIPT_IO_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        manager.start_tunnel(addr)
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn wait_for<F>(timeout: Duration, mut predicate: F) -> bool
    where
        F: FnMut() -> bool,
    {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        predicate()
    }

    fn find_tunnel(manager: &TunnelManager, id: u64) -> Option<TunnelInfo> {
        manager.tunnels().into_iter().find(|t| t.id == id)
    }

    fn pid_is_alive(pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    const MOCK_LIVE_SCRIPT: &str = r#"#!/bin/sh
url="$3"
slug=$(printf '%s' "$url" | tr -cs 'a-zA-Z0-9' '-' | sed 's/^-//;s/-$//')
echo "2026-01-01T00:00:00Z INF Starting mock tunnel for $url" >&2
echo "2026-01-01T00:00:00Z INF +------------------------------------------------------------+" >&2
echo "2026-01-01T00:00:00Z INF |  https://mock-${slug}.trycloudflare.com  |" >&2
echo "2026-01-01T00:00:00Z INF +------------------------------------------------------------+" >&2
exec sleep 60
"#;

    const MOCK_FAIL_SCRIPT: &str = r#"#!/bin/sh
echo "2026-01-01T00:00:00Z INF Requesting new quick Tunnel on trycloudflare.com..." >&2
echo "2026-01-01T00:00:01Z ERR failed to request quick Tunnel: Post \"https://api.trycloudflare.com/tunnel\": dial tcp: connection refused" >&2
exit 1
"#;

    const MOCK_NOISY_SCRIPT: &str = r#"#!/bin/sh
i=1
while [ "$i" -le 60 ]; do
    echo "log-line-$i" >&2
    i=$((i + 1))
done
echo "INF |  https://noisy-logs-test.trycloudflare.com  |" >&2
exec sleep 60
"#;

    #[test]
    fn start_multiple_tunnels_extract_urls_and_stop_individually() {
        let dir = TempDir::new("multi");
        let mock_bin = dir.write_script("mock-cloudflared", MOCK_LIVE_SCRIPT);

        let updates = Arc::new(AtomicUsize::new(0));
        let updates_clone = Arc::clone(&updates);
        let manager = TunnelManager::with_binary_path(
            mock_bin,
            Some(Arc::new(move || {
                updates_clone.fetch_add(1, Ordering::Relaxed);
            })),
        );

        let id1 = start_tunnel_synced(&manager, "3000").expect("start tunnel 1");
        let id2 = start_tunnel_synced(&manager, "127.0.0.1:5173").expect("start tunnel 2");

        assert!(id2 > id1);

        let both_running = wait_for(Duration::from_secs(5), || {
            let t1 = find_tunnel(&manager, id1);
            let t2 = find_tunnel(&manager, id2);
            matches!(t1.map(|t| t.status), Some(TunnelStatus::Running { .. }))
                && matches!(t2.map(|t| t.status), Some(TunnelStatus::Running { .. }))
        });
        assert!(both_running, "both tunnels should reach Running state");

        let snapshots = manager.tunnels();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].id, id2, "tunnels should be sorted newest-first");
        assert_eq!(snapshots[1].id, id1);
        assert_eq!(manager.active_count(), 2);
        assert!(updates.load(Ordering::Relaxed) > 0);

        let t1 = find_tunnel(&manager, id1).unwrap();
        let t2 = find_tunnel(&manager, id2).unwrap();
        assert_eq!(t1.target_url, "http://localhost:3000");
        assert_eq!(
            t1.status.public_url(),
            Some("https://mock-http-localhost-3000.trycloudflare.com")
        );
        assert_eq!(t2.target_url, "http://127.0.0.1:5173");
        assert_eq!(
            t2.status.public_url(),
            Some("https://mock-http-127-0-0-1-5173.trycloudflare.com")
        );

        let pid1 = t1.pid.expect("tunnel 1 pid");
        let pid2 = t2.pid.expect("tunnel 2 pid");

        manager.stop_tunnel(id1).expect("stop tunnel 1");

        let t1_stopped = wait_for(Duration::from_secs(3), || {
            matches!(
                find_tunnel(&manager, id1).map(|t| t.status),
                Some(TunnelStatus::Stopped)
            )
        });
        assert!(t1_stopped, "tunnel 1 should transition to Stopped");
        assert!(find_tunnel(&manager, id1).unwrap().ended_at.is_some());
        assert!(!pid_is_alive(pid1), "stopped tunnel process must be terminated");

        let t2_after = find_tunnel(&manager, id2).unwrap();
        assert!(t2_after.status.is_running(), "tunnel 2 should remain live");
        assert!(pid_is_alive(pid2), "tunnel 2 process should remain alive");
        assert_eq!(manager.active_count(), 1);

        manager.clear_inactive();
        let after_clear = manager.tunnels();
        assert_eq!(after_clear.len(), 1);
        assert_eq!(after_clear[0].id, id2);

        manager.stop_all();
        assert!(!pid_is_alive(pid2), "stop_all should terminate tunnel 2");
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn failing_tunnel_reports_informative_error() {
        let dir = TempDir::new("fail");
        let fail_bin = dir.write_script("mock-cloudflared-fail", MOCK_FAIL_SCRIPT);

        let manager = TunnelManager::with_binary_path(fail_bin, None);
        let id = start_tunnel_synced(&manager, ":8080").expect("spawn failing tunnel");

        let reached_failed = wait_for(Duration::from_secs(5), || {
            matches!(
                find_tunnel(&manager, id).map(|t| t.status),
                Some(TunnelStatus::Failed(_))
            )
        });
        assert!(reached_failed, "failing tunnel should transition to Failed");

        let info = find_tunnel(&manager, id).unwrap();
        assert_eq!(info.target_url, "http://localhost:8080");
        assert!(info.ended_at.is_some());
        assert_eq!(manager.active_count(), 0);

        match info.status {
            TunnelStatus::Failed(reason) => {
                assert!(
                    reason.contains("connection refused"),
                    "expected informative error reason, got: {reason}"
                );
            }
            other => panic!("expected Failed status, got: {other:?}"),
        }

        manager.remove_tunnel(id);
        assert!(manager.tunnels().is_empty());
    }

    #[test]
    fn log_ring_buffer_is_bounded_to_40_lines() {
        let dir = TempDir::new("logs");
        let noisy_bin = dir.write_script("mock-cloudflared-noisy", MOCK_NOISY_SCRIPT);

        let manager = TunnelManager::with_binary_path(noisy_bin, None);
        let id = start_tunnel_synced(&manager, "4000").expect("start noisy tunnel");

        let running = wait_for(Duration::from_secs(5), || {
            matches!(
                find_tunnel(&manager, id).map(|t| t.status),
                Some(TunnelStatus::Running { .. })
            )
        });
        assert!(running, "noisy tunnel should reach Running");

        let info = find_tunnel(&manager, id).unwrap();
        assert_eq!(info.recent_logs.len(), 40);
        assert!(
            info.recent_logs
                .last()
                .unwrap()
                .contains("noisy-logs-test.trycloudflare.com")
        );
        assert!(
            !info.recent_logs.iter().any(|l| l == "log-line-1"),
            "oldest lines should be evicted from bounded ring buffer"
        );
    }

    #[test]
    fn drop_cleans_up_all_running_tunnel_processes() {
        let dir = TempDir::new("drop");
        let mock_bin = dir.write_script("mock-cloudflared", MOCK_LIVE_SCRIPT);

        let (pid1, pid2) = {
            let manager = TunnelManager::with_binary_path(mock_bin, None);
            let id1 = start_tunnel_synced(&manager, "3001").expect("start tunnel 1");
            let id2 = start_tunnel_synced(&manager, "3002").expect("start tunnel 2");

            let ready = wait_for(Duration::from_secs(5), || {
                matches!(
                    find_tunnel(&manager, id1).map(|t| t.status),
                    Some(TunnelStatus::Running { .. })
                ) && matches!(
                    find_tunnel(&manager, id2).map(|t| t.status),
                    Some(TunnelStatus::Running { .. })
                )
            });
            assert!(ready, "both tunnels should be running before drop");

            let p1 = find_tunnel(&manager, id1).unwrap().pid.unwrap();
            let p2 = find_tunnel(&manager, id2).unwrap().pid.unwrap();
            assert!(pid_is_alive(p1));
            assert!(pid_is_alive(p2));
            (p1, p2)
        };

        assert!(
            !pid_is_alive(pid1),
            "Drop must terminate and reap tunnel 1 (pid {pid1})"
        );
        assert!(
            !pid_is_alive(pid2),
            "Drop must terminate and reap tunnel 2 (pid {pid2})"
        );
    }

    #[test]
    fn localhost_rewrites_to_listening_ipv4_for_cloudflared() {
        let dir = TempDir::new("loopback");
        let mock_bin = dir.write_script("mock-cloudflared", MOCK_LIVE_SCRIPT);
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback");
        let port = listener.local_addr().unwrap().port();

        let manager = TunnelManager::with_binary_path(mock_bin, None);
        let id = start_tunnel_synced(&manager, &format!("localhost:{port}"))
            .expect("start loopback tunnel");

        let running = wait_for(Duration::from_secs(5), || {
            matches!(
                find_tunnel(&manager, id).map(|t| t.status),
                Some(TunnelStatus::Running { .. })
            )
        });
        assert!(running, "loopback tunnel should reach Running state");

        let info = find_tunnel(&manager, id).unwrap();
        assert_eq!(info.target_url, format!("http://localhost:{port}"));
        let expected_url = format!("https://mock-http-127-0-0-1-{port}.trycloudflare.com");
        assert_eq!(info.status.public_url(), Some(expected_url.as_str()));

        drop(listener);
        manager.stop_all();
    }
}
