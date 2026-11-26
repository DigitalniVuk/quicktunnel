use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::cloudflared::{
    configure_background_command, detect_binary, extract_tunnel_url, install_cloudflared,
    normalize_local_address, probe_binary_version, strip_ansi,
};
use crate::model::{BinarySource, BinaryStatus, RepaintCallback, TunnelId, TunnelInfo, TunnelStatus};

const MAX_LOG_LINES: usize = 40;
const DOWNLOAD_PROGRESS_NOTIFY_INTERVAL: Duration = Duration::from_millis(50);
const STOP_ALL_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const READER_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);

pub struct TunnelManager {
    inner: Arc<ManagerState>,
}

struct ManagerState {
    binary_status: Mutex<BinaryStatus>,
    downloading: AtomicBool,
    next_id: AtomicU64,
    tunnels: Mutex<HashMap<TunnelId, TunnelRecord>>,
    on_update: Option<RepaintCallback>,
}

struct TunnelRecord {
    id: TunnelId,
    input_address: String,
    target_url: String,
    status: TunnelStatus,
    pid: Option<u32>,
    started_at: Instant,
    ended_at: Option<Instant>,
    recent_logs: VecDeque<String>,
    process: Arc<TunnelProcess>,
}

impl TunnelRecord {
    fn snapshot(&self) -> TunnelInfo {
        TunnelInfo {
            id: self.id,
            input_address: self.input_address.clone(),
            target_url: self.target_url.clone(),
            status: self.status.clone(),
            pid: self.pid,
            started_at: self.started_at,
            ended_at: self.ended_at,
            recent_logs: self.recent_logs.iter().cloned().collect(),
        }
    }

    fn push_log(&mut self, line: String) {
        if self.recent_logs.len() >= MAX_LOG_LINES {
            self.recent_logs.pop_front();
        }
        self.recent_logs.push_back(line);
    }
}

struct TunnelProcess {
    stop_requested: AtomicBool,
    state: Mutex<ProcessState>,
    wake: Condvar,
}

struct ProcessState {
    child: Option<Child>,
    readers_remaining: usize,
    done: bool,
}

struct DownloadResetGuard {
    state: Arc<ManagerState>,
}

impl Drop for DownloadResetGuard {
    fn drop(&mut self) {
        self.state.downloading.store(false, Ordering::SeqCst);
        self.state.notify_update();
    }
}

impl ManagerState {
    fn notify_update(&self) {
        if let Some(callback) = &self.on_update {
            callback();
        }
    }
}

impl TunnelManager {
    pub fn new(on_update: Option<RepaintCallback>) -> Self {
        let initial_status = detect_binary();
        Self {
            inner: Arc::new(ManagerState {
                binary_status: Mutex::new(initial_status),
                downloading: AtomicBool::new(false),
                next_id: AtomicU64::new(1),
                tunnels: Mutex::new(HashMap::new()),
                on_update,
            }),
        }
    }

    pub fn with_binary_path(path: PathBuf, on_update: Option<RepaintCallback>) -> Self {
        Self {
            inner: Arc::new(ManagerState {
                binary_status: Mutex::new(BinaryStatus::Ready {
                    path,
                    source: BinarySource::ManagedLocal,
                    version: None,
                }),
                downloading: AtomicBool::new(false),
                next_id: AtomicU64::new(1),
                tunnels: Mutex::new(HashMap::new()),
                on_update,
            }),
        }
    }

    pub fn binary_status(&self) -> BinaryStatus {
        lock_unpoison(&self.inner.binary_status).clone()
    }

    pub fn refresh_binary_status(&self) {
        if self.inner.downloading.load(Ordering::SeqCst) {
            return;
        }
        let detected = detect_binary();
        *lock_unpoison(&self.inner.binary_status) = detected;
        self.inner.notify_update();
    }

    pub fn start_binary_download(&self) {
        if self
            .inner
            .downloading
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        *lock_unpoison(&self.inner.binary_status) = BinaryStatus::Downloading {
            downloaded_bytes: 0,
            total_bytes: None,
        };
        self.inner.notify_update();

        let inner = Arc::clone(&self.inner);
        let spawn_result = thread::Builder::new()
            .name("cloudflared-download".to_string())
            .spawn(move || {
                let _reset_guard = DownloadResetGuard {
                    state: Arc::clone(&inner),
                };

                let last_notified = Mutex::new(Instant::now() - DOWNLOAD_PROGRESS_NOTIFY_INTERVAL);
                let install_result = install_cloudflared(|downloaded_bytes, total_bytes| {
                    let should_notify = {
                        let mut last = lock_unpoison(&last_notified);
                        let done = total_bytes.is_some_and(|total| downloaded_bytes >= total);
                        if downloaded_bytes == 0
                            || done
                            || last.elapsed() >= DOWNLOAD_PROGRESS_NOTIFY_INTERVAL
                        {
                            *last = Instant::now();
                            true
                        } else {
                            false
                        }
                    };

                    if should_notify {
                        *lock_unpoison(&inner.binary_status) = BinaryStatus::Downloading {
                            downloaded_bytes,
                            total_bytes,
                        };
                        inner.notify_update();
                    }
                });

                let final_status = match install_result {
                    Ok(path) => {
                        let version = probe_binary_version(&path);
                        BinaryStatus::Ready {
                            path,
                            source: BinarySource::ManagedLocal,
                            version,
                        }
                    }
                    Err(err) => BinaryStatus::DownloadFailed(err),
                };

                *lock_unpoison(&inner.binary_status) = final_status;
            });

        if let Err(err) = spawn_result {
            self.inner.downloading.store(false, Ordering::SeqCst);
            *lock_unpoison(&self.inner.binary_status) =
                BinaryStatus::DownloadFailed(format!("Failed to start download thread: {err}"));
            self.inner.notify_update();
        }
    }

    pub fn start_tunnel(&self, raw_address: &str) -> Result<TunnelId, String> {
        let target_url = normalize_local_address(raw_address)?;

        let binary_path = match self.binary_status() {
            BinaryStatus::Ready { path, .. } => path,
            BinaryStatus::Downloading { .. } => {
                return Err(
                    "cloudflared is currently downloading. Please wait until it finishes."
                        .to_string(),
                );
            }
            BinaryStatus::Missing | BinaryStatus::DownloadFailed(_) => {
                return Err(
                    "cloudflared is not installed yet. Download or install cloudflared first."
                        .to_string(),
                );
            }
        };

        let mut cmd = Command::new(&binary_path);
        cmd.args(["tunnel", "--url", &target_url, "--no-autoupdate"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_background_command(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start cloudflared: {e}"))?;

        let pid = Some(child.id());
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let readers_count = usize::from(stdout.is_some()) + usize::from(stderr.is_some());

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let process = Arc::new(TunnelProcess {
            stop_requested: AtomicBool::new(false),
            state: Mutex::new(ProcessState {
                child: Some(child),
                readers_remaining: readers_count,
                done: false,
            }),
            wake: Condvar::new(),
        });

        let record = TunnelRecord {
            id,
            input_address: raw_address.trim().to_string(),
            target_url,
            status: TunnelStatus::Starting,
            pid,
            started_at: Instant::now(),
            ended_at: None,
            recent_logs: VecDeque::with_capacity(MAX_LOG_LINES),
            process: Arc::clone(&process),
        };

        lock_unpoison(&self.inner.tunnels).insert(id, record);
        self.inner.notify_update();

        if let Some(out) = stdout {
            spawn_output_reader(
                format!("tunnel-{id}-stdout"),
                out,
                id,
                Arc::clone(&self.inner),
                Arc::clone(&process),
            );
        }
        if let Some(err) = stderr {
            spawn_output_reader(
                format!("tunnel-{id}-stderr"),
                err,
                id,
                Arc::clone(&self.inner),
                Arc::clone(&process),
            );
        }

        spawn_process_watcher(id, Arc::clone(&self.inner), process);

        Ok(id)
    }

    pub fn stop_tunnel(&self, id: TunnelId) -> Result<(), String> {
        let process = {
            let mut tunnels = lock_unpoison(&self.inner.tunnels);
            let Some(record) = tunnels.get_mut(&id) else {
                return Err(format!("Tunnel #{id} was not found."));
            };
            if !record.status.is_active() {
                return Ok(());
            }
            record.status = TunnelStatus::Stopping;
            Arc::clone(&record.process)
        };

        let kill_err = request_process_stop(&process);
        self.inner.notify_update();

        if let Some(err) = kill_err {
            return Err(format!("Failed to stop tunnel #{id}: {err}"));
        }

        Ok(())
    }

    pub fn stop_all(&self) {
        let processes: Vec<Arc<TunnelProcess>> = {
            let mut tunnels = lock_unpoison(&self.inner.tunnels);
            let mut active = Vec::new();
            for record in tunnels.values_mut() {
                if record.status.is_active() {
                    record.status = TunnelStatus::Stopping;
                    active.push(Arc::clone(&record.process));
                }
            }
            active
        };

        if processes.is_empty() {
            return;
        }

        for process in &processes {
            let _ = request_process_stop(process);
        }
        self.inner.notify_update();

        let deadline = Instant::now() + STOP_ALL_WAIT_TIMEOUT;
        for process in processes {
            let mut state = lock_unpoison(&process.state);
            while !state.done {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                let remaining = deadline.saturating_duration_since(now);
                let (guard, _) = process
                    .wake
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|e| e.into_inner());
                state = guard;
            }
        }
    }

    pub fn remove_tunnel(&self, id: TunnelId) {
        let removed = lock_unpoison(&self.inner.tunnels).remove(&id);
        if let Some(record) = removed {
            if record.status.is_active() {
                let _ = request_process_stop(&record.process);
            }
            self.inner.notify_update();
        }
    }

    pub fn clear_inactive(&self) {
        let changed = {
            let mut tunnels = lock_unpoison(&self.inner.tunnels);
            let before = tunnels.len();
            tunnels.retain(|_, record| record.status.is_active());
            tunnels.len() != before
        };
        if changed {
            self.inner.notify_update();
        }
    }

    pub fn tunnels(&self) -> Vec<TunnelInfo> {
        let mut snapshots: Vec<TunnelInfo> = lock_unpoison(&self.inner.tunnels)
            .values()
            .map(TunnelRecord::snapshot)
            .collect();
        snapshots.sort_by(|a, b| b.id.cmp(&a.id));
        snapshots
    }

    pub fn active_count(&self) -> usize {
        lock_unpoison(&self.inner.tunnels)
            .values()
            .filter(|record| record.status.is_active())
            .count()
    }
}

impl Drop for TunnelManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

fn request_process_stop(process: &TunnelProcess) -> Option<io::Error> {
    process.stop_requested.store(true, Ordering::SeqCst);
    let mut state = lock_unpoison(&process.state);
    let mut kill_err = None;
    if let Some(child) = state.child.as_mut()
        && let Err(err) = child.kill()
    {
        let already_exited =
            err.kind() == io::ErrorKind::InvalidInput || matches!(child.try_wait(), Ok(Some(_)));
        if !already_exited {
            kill_err = Some(err);
        }
    }
    process.wake.notify_all();
    kill_err
}

fn spawn_output_reader<R: Read + Send + 'static>(
    thread_name: String,
    stream: R,
    id: TunnelId,
    inner: Arc<ManagerState>,
    process: Arc<TunnelProcess>,
) {
    let _ = thread::Builder::new().name(thread_name).spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();

        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let raw_line = String::from_utf8_lossy(&buf);
                    let cleaned = strip_ansi(raw_line.trim_end_matches(['\r', '\n']));
                    let trimmed = cleaned.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let detected_url = extract_tunnel_url(trimmed);
                    let log_line = trimmed.to_string();

                    let updated = {
                        let mut tunnels = lock_unpoison(&inner.tunnels);
                        if let Some(record) = tunnels.get_mut(&id) {
                            record.push_log(log_line);
                            if let Some(public_url) = detected_url
                                && matches!(record.status, TunnelStatus::Starting)
                            {
                                record.status = TunnelStatus::Running { public_url };
                            }
                            true
                        } else {
                            false
                        }
                    };

                    if updated {
                        inner.notify_update();
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        let mut state = lock_unpoison(&process.state);
        state.readers_remaining = state.readers_remaining.saturating_sub(1);
        process.wake.notify_all();
    });
}

fn spawn_process_watcher(id: TunnelId, inner: Arc<ManagerState>, process: Arc<TunnelProcess>) {
    let _ = thread::Builder::new()
        .name(format!("tunnel-{id}-watcher"))
        .spawn(move || {
            let (exit_status, wait_err) = {
                let mut state = lock_unpoison(&process.state);
                loop {
                    let wait_result = match state.child.as_mut() {
                        Some(child) => child.try_wait(),
                        None => break (None, None),
                    };

                    match wait_result {
                        Ok(Some(status)) => {
                            state.child = None;
                            break (Some(status), None);
                        }
                        Err(err) => {
                            state.child = None;
                            break (None, Some(err));
                        }
                        Ok(None) => {
                            let timeout = if process.stop_requested.load(Ordering::SeqCst) {
                                if let Some(child) = state.child.as_mut() {
                                    let _ = child.kill();
                                }
                                Duration::from_millis(10)
                            } else {
                                Duration::from_millis(100)
                            };
                            let (guard, _) = process
                                .wake
                                .wait_timeout(state, timeout)
                                .unwrap_or_else(|e| e.into_inner());
                            state = guard;
                        }
                    }
                }
            };

            if !process.stop_requested.load(Ordering::SeqCst) {
                let drain_deadline = Instant::now() + READER_DRAIN_TIMEOUT;
                let mut state = lock_unpoison(&process.state);
                while state.readers_remaining > 0 {
                    let now = Instant::now();
                    if now >= drain_deadline {
                        break;
                    }
                    let remaining = drain_deadline.saturating_duration_since(now);
                    let (guard, _) = process
                        .wake
                        .wait_timeout(state, remaining)
                        .unwrap_or_else(|e| e.into_inner());
                    state = guard;
                }
            }

            {
                let mut tunnels = lock_unpoison(&inner.tunnels);
                if let Some(record) = tunnels.get_mut(&id) {
                    record.ended_at = Some(Instant::now());
                    if process.stop_requested.load(Ordering::SeqCst)
                        || matches!(record.status, TunnelStatus::Stopping)
                    {
                        record.status = TunnelStatus::Stopped;
                    } else {
                        let reason =
                            extract_error_reason(&record.recent_logs, exit_status, wait_err);
                        record.status = TunnelStatus::Failed(reason);
                    }
                }
            }

            {
                let mut state = lock_unpoison(&process.state);
                state.done = true;
                process.wake.notify_all();
            }

            inner.notify_update();
        });
}

fn extract_error_reason(
    logs: &VecDeque<String>,
    exit_status: Option<ExitStatus>,
    wait_err: Option<io::Error>,
) -> String {
    if let Some(line) = logs.iter().rev().find(|line| is_error_log_line(line)) {
        return clean_log_line(line);
    }

    if let Some(line) = logs.iter().rev().find(|line| !is_decorative_log_line(line)) {
        return clean_log_line(line);
    }

    if let Some(err) = wait_err {
        return format!("Tunnel process error: {err}");
    }

    if let Some(status) = exit_status {
        if let Some(code) = status.code() {
            return format!("cloudflared exited unexpectedly (exit code {code}).");
        }
        return "cloudflared was terminated unexpectedly.".to_string();
    }

    "cloudflared exited unexpectedly.".to_string()
}

fn is_error_log_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains(" err ")
        || lower.starts_with("err ")
        || lower.contains(" ftl ")
        || lower.starts_with("ftl ")
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fatal")
        || lower.contains("unable to")
        || lower.contains("refused")
        || lower.contains("denied")
        || lower.contains("invalid")
}

fn is_decorative_log_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed
            .chars()
            .all(|c| matches!(c, '+' | '-' | '|' | '=' | ' '))
}

fn clean_log_line(line: &str) -> String {
    let trimmed = line.trim();
    if let Some((first, rest)) = trimmed.split_once(' ')
        && first.len() >= 19
        && first.contains('T')
        && first.ends_with('Z')
    {
        let rest_trimmed = rest.trim();
        if !rest_trimmed.is_empty() {
            return rest_trimmed.to_string();
        }
    }
    trimmed.to_string()
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
