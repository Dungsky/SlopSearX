//! Backend-neutral one-request worker ownership. Backend validates its own
//! operation header, payload, output envelope, stages and native resource policy.
use std::{
    fmt,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Notify, Semaphore, oneshot},
    time::Instant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    InvalidCapacity,
    CleanupUnverified,
    Deadline,
    AdmissionClosed,
    Shutdown,
    Cancelled,
    OutputBound,
    Truncated,
    TrailingBytes,
    Allocation,
    Spawn(String),
    Io(String),
    Exit,
    Supervisor,
}
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ProcessError {}
#[derive(Default)]
struct State {
    closing: bool,
    active: usize,
    cleanup_failed: bool,
    supervisor_failed: bool,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
struct Inner {
    helper: PathBuf,
    permits: Arc<Semaphore>,
    state: Mutex<State>,
    drained: Notify,
    shutdown: tokio::sync::watch::Sender<bool>,
    #[cfg(test)]
    cleanup_gate: Mutex<Option<Arc<CleanupGate>>>,
    #[cfg(test)]
    cleanup_error: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    delivery_gate: Mutex<Option<Arc<CleanupGate>>>,
}
#[derive(Clone)]
pub struct ProcessPool {
    inner: Arc<Inner>,
}
struct Active<L> {
    inner: Arc<Inner>,
    completed: bool,
    cleanup_verified: bool,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    lease: Option<L>,
}
impl<L> Drop for Active<L> {
    fn drop(&mut self) {
        if !self.completed {
            fail_supervisor(&self.inner);
        }
        if self.cleanup_verified {
            let lease = self.lease.take();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(lease))).is_err() {
                fail_supervisor(&self.inner);
            }
            drop(self.permit.take());
        } else {
            // Reuse is permanently closed on unknown cleanup. Retain caller
            // accounting rather than pretending orphaned resources disappeared.
            if let Some(permit) = self.permit.take() {
                permit.forget();
            }
            if let Some(lease) = self.lease.take() {
                std::mem::forget(lease);
            }
        }
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.active -= 1;
        drop(state);
        self.inner.drained.notify_waiters();
    }
}
fn fail_supervisor(inner: &Inner) {
    let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
    state.supervisor_failed = true;
    state.closing = true;
    inner.permits.close();
    inner.shutdown.send_replace(true);
}
impl ProcessPool {
    pub fn new(helper: PathBuf, capacity: NonZeroUsize) -> Result<Self, ProcessError> {
        if capacity.get() > Semaphore::MAX_PERMITS {
            return Err(ProcessError::InvalidCapacity);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                helper,
                permits: Arc::new(Semaphore::new(capacity.get())),
                state: Mutex::new(State::default()),
                drained: Notify::new(),
                shutdown: tokio::sync::watch::channel(false).0,
                #[cfg(test)]
                cleanup_gate: Mutex::new(None),
                #[cfg(test)]
                cleanup_error: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                delivery_gate: Mutex::new(None),
            }),
        })
    }
    pub fn available_slots(&self) -> usize {
        self.inner.permits.available_permits()
    }
    /// All callers wait for all admitted work to be killed/reaped. Dropping one
    /// shutdown waiter cannot remove another caller's ability to drain.
    /// Must be explicitly awaited before runtime teardown. Runtime task abort
    /// cannot provide the normal asynchronous kill-and-reap guarantee.
    pub async fn shutdown(&self) -> Result<(), ProcessError> {
        {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            state.closing = true;
            self.inner.permits.close();
            self.inner.shutdown.send_replace(true);
        }
        loop {
            let notified = self.inner.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let tasks = {
                let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.active == 0 {
                    Some(std::mem::take(&mut state.tasks))
                } else {
                    None
                }
            };
            if let Some(tasks) = tasks {
                for task in tasks {
                    if task.await.is_err() {
                        self.inner
                            .state
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .supervisor_failed = true;
                    }
                }
                let state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
                return if state.cleanup_failed {
                    Err(ProcessError::CleanupUnverified)
                } else if state.supervisor_failed {
                    Err(ProcessError::Supervisor)
                } else {
                    Ok(())
                };
            }
            notified.await;
        }
    }
    /// No child starts after admission closes. The supervisor owns the permit
    /// through kill+wait even when the calling future disappears.
    pub async fn exchange(
        &self,
        deadline: Instant,
        header: Vec<u8>,
        payload: Vec<u8>,
        bound: usize,
    ) -> Result<Vec<u8>, ProcessError> {
        self.exchange_with_lease(deadline, header, payload, bound, ())
            .await
    }
    /// Caller passes a clone of its operation accounting owner. It survives
    /// receiver cancellation until frame resources drop and cleanup is verified.
    /// The caller retains its original owner while consuming returned bytes.
    /// Unknown cleanup/panic permanently poisons admission and retains this lease.
    pub async fn exchange_with_lease<L: Send + 'static>(
        &self,
        deadline: Instant,
        header: Vec<u8>,
        payload: Vec<u8>,
        bound: usize,
        lease: L,
    ) -> Result<Vec<u8>, ProcessError> {
        if Instant::now() >= deadline {
            return Err(ProcessError::Deadline);
        }
        let permit = tokio::time::timeout_at(deadline, self.inner.permits.clone().acquire_owned())
            .await
            .map_err(|_| ProcessError::Deadline)?
            .map_err(|_| ProcessError::AdmissionClosed)?;
        let (sender, receiver) = oneshot::channel();
        let inner = self.inner.clone();
        let mut shutdown = inner.shutdown.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closing {
                return Err(ProcessError::AdmissionClosed);
            }
            state.tasks.retain(|t| !t.is_finished());
            state.active += 1;
            let active = Active {
                inner: inner.clone(),
                completed: false,
                cleanup_verified: false,
                permit: Some(permit),
                lease: Some(lease),
            };
            let task = tokio::spawn(async move {
                let mut active = active;
                let mut sender = sender;
                let result = run_worker(
                    &inner,
                    &mut sender,
                    &mut shutdown,
                    Request {
                        deadline,
                        header,
                        payload,
                        bound,
                    },
                )
                .await;
                active.cleanup_verified = result != Err(ProcessError::CleanupUnverified);
                if active.cleanup_verified {
                    drop(active.permit.take());
                }
                #[cfg(test)]
                {
                    let gate = inner.delivery_gate.lock().unwrap().clone();
                    if let Some(gate) = gate {
                        gate.entered.add_permits(1);
                        gate.release.acquire().await.unwrap().forget();
                    }
                }
                // Failed delivery returns ownership of the reply. Destroy it
                // before Active drops the accounting lease.
                drop(sender.send(result));
                active.completed = true;
            });
            state.tasks.push(task);
        }
        receiver.await.map_err(|_| ProcessError::Supervisor)?
    }
}
struct Request {
    deadline: Instant,
    header: Vec<u8>,
    payload: Vec<u8>,
    bound: usize,
}
async fn run_worker(
    inner: &Inner,
    sender: &mut oneshot::Sender<Result<Vec<u8>, ProcessError>>,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
    request: Request,
) -> Result<Vec<u8>, ProcessError> {
    let Request {
        deadline,
        header,
        payload,
        bound,
    } = request;
    if sender.is_closed() {
        return Err(ProcessError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ProcessError::Deadline);
    }
    let mut child = {
        // Actual spawn and closing share this lock; never await while held.
        let state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closing {
            return Err(ProcessError::Shutdown);
        }
        if sender.is_closed() {
            return Err(ProcessError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(ProcessError::Deadline);
        }
        tokio::process::Command::new(&inner.helper)
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ProcessError::Spawn(e.to_string()))?
    };
    let (Some(mut stdin), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.start_kill();
        if child.wait().await.is_err() {
            fail_cleanup(inner);
            return Err(ProcessError::CleanupUnverified);
        }
        return Err(ProcessError::Supervisor);
    };
    let work = async {
        let write = async move {
            stdin.write_all(&header).await.map_err(io)?;
            stdin.write_all(&payload).await.map_err(io)?;
            stdin.shutdown().await.map_err(io)?;
            drop(stdin);
            Ok::<(), ProcessError>(())
        };
        let read = async {
            let length = stdout.read_u64().await.map_err(frame_io)?;
            let length = usize::try_from(length).map_err(|_| ProcessError::OutputBound)?;
            if length > bound {
                return Err(ProcessError::OutputBound);
            }
            let mut out = Vec::new();
            out.try_reserve_exact(length)
                .map_err(|_| ProcessError::Allocation)?;
            out.resize(length, 0);
            stdout.read_exact(&mut out).await.map_err(frame_io)?;
            let mut trailing = [0];
            if stdout.read(&mut trailing).await.map_err(io)? != 0 {
                return Err(ProcessError::TrailingBytes);
            }
            Ok(out)
        };
        let (_, out) = tokio::try_join!(write, read)?;
        if !child.wait().await.map_err(io)?.success() {
            return Err(ProcessError::Exit);
        }
        Ok(out)
    };
    let result = tokio::select! {
        result = work => result,
        _ = tokio::time::sleep_until(deadline) => Err(ProcessError::Deadline),
        _ = sender.closed() => Err(ProcessError::Cancelled),
        _ = async {
            if *shutdown.borrow() { return; }
            let _ = shutdown.changed().await;
        } => Err(ProcessError::Shutdown),
    };
    #[cfg(test)]
    {
        let gate = inner.cleanup_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.entered.add_permits(1);
            gate.release.acquire().await.unwrap().forget();
        }
    }
    // Even a correct frame cannot leave a wedged child. wait may already have
    // reaped success; repeated wait is supported, kill-on-exit error is benign.
    let _ = child.start_kill();
    let unverified = child.wait().await.is_err();
    #[cfg(test)]
    let unverified = unverified
        | inner
            .cleanup_error
            .swap(false, std::sync::atomic::Ordering::SeqCst);
    if unverified {
        fail_cleanup(inner);
        return Err(ProcessError::CleanupUnverified);
    }
    result
}

fn io(e: std::io::Error) -> ProcessError {
    ProcessError::Io(e.to_string())
}
fn frame_io(e: std::io::Error) -> ProcessError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ProcessError::Truncated
    } else {
        io(e)
    }
}

fn fail_cleanup(inner: &Inner) {
    let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
    state.cleanup_failed = true;
    state.closing = true;
    inner.permits.close();
    inner.shutdown.send_replace(true);
}

#[cfg(test)]
struct CleanupGate {
    entered: Semaphore,
    release: Semaphore,
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[tokio::test]
    async fn unknown_cleanup_closes_reuse_and_reports_shutdown_failure() {
        let pool =
            ProcessPool::new(PathBuf::from("unused"), NonZeroUsize::new(1).unwrap()).unwrap();
        fail_cleanup(&pool.inner);
        assert_eq!(
            pool.exchange(
                Instant::now() + std::time::Duration::from_secs(1),
                vec![],
                vec![],
                1
            )
            .await,
            Err(ProcessError::AdmissionClosed)
        );
        assert_eq!(pool.shutdown().await, Err(ProcessError::CleanupUnverified));
    }
    #[tokio::test]
    async fn both_shutdown_waiters_observe_injected_supervisor_panic() {
        let pool =
            ProcessPool::new(PathBuf::from("unused"), NonZeroUsize::new(1).unwrap()).unwrap();
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let active = Active {
            inner: pool.inner.clone(),
            completed: false,
            cleanup_verified: false,
            permit: None,
            lease: Some(lease),
        };
        let task = tokio::spawn(async move {
            let _active = active;
            panic!("injected supervisor fault");
        });
        {
            let mut state = pool.inner.state.lock().unwrap();
            state.active = 1;
            state.tasks.push(task);
        }
        let (first, second) = tokio::join!(pool.shutdown(), pool.shutdown());
        assert_eq!(first, Err(ProcessError::Supervisor));
        assert_eq!(second, Err(ProcessError::Supervisor));
        assert_eq!(pool.shutdown().await, Err(ProcessError::Supervisor));
        assert_eq!(
            accounting.available_permits(),
            0,
            "panic must not reopen caller accounting"
        );
    }
    #[cfg(unix)]
    fn fixture(body: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("a06-lease-{}-{id}", std::process::id()));
        let pid = path.with_extension("pid");
        std::fs::write(
            &path,
            format!("#!/bin/sh\necho $$ > '{}'\n{body}\n", pid.display()),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (path, pid)
    }
    #[cfg(unix)]
    async fn actual_pid(path: &PathBuf) -> String {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Ok(s) = std::fs::read_to_string(path) {
                    break s.trim().to_owned();
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap()
    }
    #[cfg(unix)]
    fn pid_gone(pid: &str) -> bool {
        !std::process::Command::new("/bin/kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_caller_lease_survives_blocked_cleanup_until_real_reap() {
        let (path, pidfile) = fixture("exec /bin/sleep 30");
        let pool = ProcessPool::new(path.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
        let gate = Arc::new(CleanupGate {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
        });
        *pool.inner.cleanup_gate.lock().unwrap() = Some(gate.clone());
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let weak = Arc::downgrade(&lease);
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    vec![7; 65536],
                    2,
                    lease,
                )
                .await
        });
        let pid = actual_pid(&pidfile).await;
        caller.abort();
        let _ = caller.await;
        tokio::time::timeout(std::time::Duration::from_secs(2), gate.entered.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
        let blocked = accounting.clone().try_acquire_owned().is_err();
        let alive = !pid_gone(&pid);
        let slots = pool.available_slots();
        gate.release.add_permits(1);
        pool.shutdown().await.unwrap();
        assert!(blocked, "caller accounting reopened before cleanup");
        assert!(alive, "gate did not block actual live child cleanup");
        assert_eq!(slots, 0);
        assert_eq!(accounting.available_permits(), 1);
        assert!(weak.upgrade().is_none());
        assert!(pid_gone(&pid));
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(pidfile);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn verified_success_and_protocol_error_release_lease_after_cleanup() {
        for (index, body) in [
            "/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'",
            "/bin/cat >/dev/null; printf x",
        ]
        .into_iter()
        .enumerate()
        {
            let (path, pidfile) = fixture(body);
            let pool = ProcessPool::new(path.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
            let accounting = Arc::new(Semaphore::new(1));
            let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
            let result = pool
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(2),
                    vec![],
                    vec![],
                    2,
                    lease,
                )
                .await;
            assert_eq!(
                result,
                if index == 0 {
                    Ok(b"ok".to_vec())
                } else {
                    Err(ProcessError::Truncated)
                }
            );
            pool.shutdown().await.unwrap();
            assert_eq!(accounting.available_permits(), 1);
            assert!(pid_gone(&actual_pid(&pidfile).await));
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_file(pidfile);
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn closed_receiver_during_success_delivery_retains_lease_until_reply_drop() {
        let (path, pidfile) =
            fixture("/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'");
        let pool = ProcessPool::new(path.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
        let gate = Arc::new(CleanupGate {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
        });
        *pool.inner.delivery_gate.lock().unwrap() = Some(gate.clone());
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    vec![],
                    2,
                    lease,
                )
                .await
        });
        gate.entered.acquire().await.unwrap().forget();
        caller.abort();
        let _ = caller.await;
        assert_eq!(accounting.available_permits(), 0);
        gate.release.add_permits(1);
        pool.shutdown().await.unwrap();
        assert_eq!(accounting.available_permits(), 1);
        assert!(pid_gone(&actual_pid(&pidfile).await));
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(pidfile);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn unknown_cleanup_retains_lease_and_permit_and_reports_failed_drain() {
        let (path, pidfile) =
            fixture("/bin/cat >/dev/null; printf '\\000\\000\\000\\000\\000\\000\\000\\002ok'");
        let pool = ProcessPool::new(path.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
        pool.inner
            .cleanup_error
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let accounting = Arc::new(Semaphore::new(1));
        let lease = Arc::new(accounting.clone().acquire_owned().await.unwrap());
        let weak = Arc::downgrade(&lease);
        let worker = pool.clone();
        let caller = tokio::spawn(async move {
            worker
                .exchange_with_lease(
                    Instant::now() + std::time::Duration::from_secs(10),
                    vec![],
                    vec![],
                    2,
                    lease,
                )
                .await
        });
        let pid = actual_pid(&pidfile).await;
        assert_eq!(caller.await.unwrap(), Err(ProcessError::CleanupUnverified));
        assert_eq!(pool.shutdown().await, Err(ProcessError::CleanupUnverified));
        assert_eq!(pool.available_slots(), 0);
        assert_eq!(accounting.available_permits(), 0);
        assert!(weak.upgrade().is_some());
        assert!(
            pid_gone(&pid),
            "injected uncertainty follows actual successful reap"
        );
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(pidfile);
    }
}
