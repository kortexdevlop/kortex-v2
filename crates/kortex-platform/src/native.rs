//! Native host backend. Package policy stays in kortex-pkg; OS calls live here.
//! This module is unavailable to browser builds and never executes commands through a shell.
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub mod fs {
    use super::*;
    pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }
    pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }
    pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
    pub fn write(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
        std::fs::write(path, data)
    }
    pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }
    pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::remove_file(path)
    }
    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    pub fn read_dir(path: impl AsRef<Path>) -> io::Result<std::fs::ReadDir> {
        std::fs::read_dir(path)
    }
    pub fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
        std::fs::metadata(path)
    }
    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }
    pub fn is_dir(path: impl AsRef<Path>) -> bool {
        path.as_ref().is_dir()
    }
    /// Stream with a hard cap, including when a file changes after its metadata was read.
    pub fn read_limited(path: impl AsRef<Path>, limit: u64) -> io::Result<Vec<u8>> {
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "file exceeds size limit",
            ));
        }
        Ok(bytes)
    }
    /// Create a file exclusively; never overwrite an existing path or follow its symlink.
    pub fn write_new(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        let path = path.as_ref();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        if let Err(error) = file.write_all(bytes) {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(error);
        }
        Ok(())
    }
    pub fn set_executable(path: impl AsRef<Path>) -> io::Result<()> {
        set_mode(path, 0o755)
    }
    pub fn set_private(path: impl AsRef<Path>) -> io::Result<()> {
        set_mode(path, 0o600)
    }
    fn set_mode(path: impl AsRef<Path>, mode: u32) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
            Ok(())
        }
    }
}

pub mod env {
    use super::*;
    pub fn var(key: &str) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }
    pub fn var_os(key: &str) -> Option<std::ffi::OsString> {
        std::env::var_os(key)
    }
    pub fn current_exe() -> io::Result<PathBuf> {
        std::env::current_exe()
    }
    /// Command-line arguments after the program name.
    pub fn args() -> Vec<String> {
        std::env::args().skip(1).collect()
    }
    pub fn home_dir() -> Option<PathBuf> {
        var_os("HOME")
            .or_else(|| var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

pub fn launch_executable(path: &Path) -> io::Result<()> {
    std::process::Command::new(path).spawn().map(|_| ())
}
pub fn device_name() -> String {
    env::var("HOSTNAME")
        .or_else(|_| env::var("COMPUTERNAME"))
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|v| String::from_utf8(v.stdout).ok())
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_else(|| "kortex-cli".to_owned())
}
pub fn terminal_line(label: Option<&str>, trim: bool) -> Option<String> {
    if let Some(label) = label {
        eprint!("{label}");
        let _ = io::stderr().flush();
    }
    let mut line = String::new();
    if io::stdin().read_line(&mut line).ok()? == 0 {
        return None;
    }
    Some(if trim {
        line.trim().to_owned()
    } else {
        line.trim_end_matches(['\r', '\n']).to_owned()
    })
}
pub fn terminal_password(label: &str) -> Option<String> {
    rpassword::prompt_password(label).ok()
}

/// Save a user-requested output without races that could overwrite an existing file.
pub fn save_output(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    if matches!(name, "." | "..") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid output name",
        ));
    }
    fs::create_dir_all(directory)?;
    let base = Path::new(name)
        .file_name()
        .filter(|v| !v.is_empty())
        .map_or_else(
            || "kortex-output".to_owned(),
            |v| v.to_string_lossy().into_owned(),
        );
    if base == "." || base == ".." || base.contains(['/', '\\']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid output name",
        ));
    }
    let (stem, extension) = base
        .rsplit_once('.')
        .map_or((base.as_str(), String::new()), |(s, e)| {
            (s, format!(".{e}"))
        });
    let mut candidate = directory.join(&base);
    let mut suffix = 2;
    loop {
        match fs::write_new(&candidate, bytes) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = directory.join(format!("{stem} ({suffix}){extension}"));
                suffix += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
/// Prints a diagnostic line on the standard error stream.
pub fn report(message: &str) {
    eprintln!("{message}");
}

pub fn save_download(name: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = env::home_dir()
        .ok_or("dossier personnel introuvable")?
        .join("Downloads");
    save_output(&dir, name, bytes)
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// Bounded native HTTP request. Callers receive data, never a transport-specific client.
pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: Option<&'a [u8]>,
    pub timeout: std::time::Duration,
    pub max_bytes: u64,
}
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    /// The `Location` header of a redirect (only meaningful for [`http_once`]).
    pub location: Option<String>,
}
#[must_use]
pub fn is_local_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    if let Some(port) = authority.strip_prefix("[::1]") {
        return port.is_empty()
            || port
                .strip_prefix(':')
                .is_some_and(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()));
    }
    let host = authority
        .split_once(':')
        .map_or(authority, |(host, _)| host);
    matches!(host, "localhost" | "127.0.0.1")
}
/// Like [`http`], but hands the response body to `sink` as it arrives instead of buffering it.
/// `on_status` is called once with the status code before any body byte; `sink` returns `false`
/// to stop reading (the connection is dropped). Returns the status code.
///
/// # Errors
/// Transport failure, a body larger than `max_bytes`, or a non-UTF-8-agnostic read error.
pub fn http_stream(
    request: HttpRequest<'_>,
    on_status: impl FnOnce(u16),
    mut sink: impl FnMut(&[u8]) -> bool,
) -> Result<u16, String> {
    use std::io::Read;
    let secure = request.url.starts_with("https://");
    if !secure && !is_local_http(request.url) {
        return Err("unencrypted remote HTTP is forbidden".into());
    }
    if request.method != "POST" {
        return Err("unsupported HTTP method".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .http_status_as_error(false)
        .https_only(secure)
        .max_redirects(0)
        .build()
        .into();
    let mut builder = agent.post(request.url);
    for (name, value) in request.headers {
        builder = builder.header(*name, *value);
    }
    let mut response = builder
        .send(request.body.unwrap_or_default())
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    on_status(status);
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(request.max_bytes)
        .reader();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 || !sink(&buffer[..read]) {
            return Ok(status);
        }
    }
}

pub fn http(request: HttpRequest<'_>) -> Result<HttpResponse, String> {
    send(request, None)
}

/// Like [`http`] but never follows a redirect: the caller checks the `Location` itself.
///
/// # Errors
/// Transport failure, a non-HTTPS remote URL, an unsupported method or a body over the limit.
pub fn http_once(request: HttpRequest<'_>) -> Result<HttpResponse, String> {
    send(request, Some(0))
}

fn send(request: HttpRequest<'_>, redirects: Option<u32>) -> Result<HttpResponse, String> {
    let secure = request.url.starts_with("https://");
    if !secure && !is_local_http(request.url) {
        return Err("unencrypted remote HTTP is forbidden".into());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(request.timeout))
        .http_status_as_error(false)
        .https_only(secure)
        .max_redirects(redirects.unwrap_or(if secure { 10 } else { 0 }))
        .build()
        .into();
    let mut response = match request.method {
        "GET" | "DELETE" => {
            let mut builder = if request.method == "GET" {
                agent.get(request.url)
            } else {
                agent.delete(request.url)
            };
            for (name, value) in request.headers {
                builder = builder.header(*name, *value);
            }
            builder.call().map_err(|e| e.to_string())?
        }
        "POST" | "PUT" | "PATCH" => {
            let mut builder = match request.method {
                "POST" => agent.post(request.url),
                "PUT" => agent.put(request.url),
                _ => agent.patch(request.url),
            };
            for (name, value) in request.headers {
                builder = builder.header(*name, *value);
            }
            builder
                .send(request.body.unwrap_or_default())
                .map_err(|e| e.to_string())?
        }
        _ => return Err("unsupported HTTP method".into()),
    };
    let status = response.status().as_u16();
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response
        .body_mut()
        .with_config()
        .limit(request.max_bytes)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(HttpResponse {
        status,
        body,
        location,
    })
}

#[must_use]
pub fn process_id() -> u32 {
    std::process::id()
}
/// Native CLI bootstrap; command logic stays injectable and free of terminal access.
pub fn run_cli(
    execute: impl FnOnce(&[String], &mut dyn Write) -> Result<(), String>,
) -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match execute(&args, &mut io::stdout()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erreur : {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Process input/output for headless modes (`--engine`); commands stay testable by taking
/// generic readers and writers.
pub mod stdio {
    use std::io::{self, BufRead, Write};

    /// Locked standard input and output, for a request/response loop.
    #[must_use]
    pub fn streams() -> (impl BufRead, impl Write) {
        (io::stdin().lock(), io::stdout().lock())
    }
}

/// Background execution: a dedicated thread plus a single-value completion slot that can be
/// awaited from a UI future or blocked on from a headless loop.
pub mod task {
    use std::future::Future;
    use std::io;
    use std::pin::Pin;
    use std::sync::{Arc, Condvar, Mutex, MutexGuard};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    struct State<T> {
        value: Option<T>,
        done: bool,
        waker: Option<Waker>,
    }

    struct Inner<T> {
        state: Mutex<State<T>>,
        signal: Condvar,
    }

    /// First-writer-wins completion slot with a single consumer.
    pub struct Slot<T>(Arc<Inner<T>>);

    impl<T> Clone for Slot<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }

    impl<T> Default for Slot<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Slot<T> {
        #[must_use]
        pub fn new() -> Self {
            Self(Arc::new(Inner {
                state: Mutex::new(State {
                    value: None,
                    done: false,
                    waker: None,
                }),
                signal: Condvar::new(),
            }))
        }

        fn lock(&self) -> MutexGuard<'_, State<T>> {
            // A poisoned lock only means another thread panicked while holding it; the state
            // is a plain value and stays consistent.
            self.0
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        /// Stores `value` unless the slot is already complete; returns whether it was stored.
        pub fn complete(&self, value: T) -> bool {
            let waker = {
                let mut state = self.lock();
                if state.done {
                    return false;
                }
                state.value = Some(value);
                state.done = true;
                state.waker.take()
            };
            self.0.signal.notify_all();
            if let Some(waker) = waker {
                waker.wake();
            }
            true
        }

        #[must_use]
        pub fn is_done(&self) -> bool {
            self.lock().done
        }

        /// Blocks up to `timeout`; returns whether the slot completed in time.
        #[must_use]
        pub fn wait_done(&self, timeout: Duration) -> bool {
            let guard = self.lock();
            let (guard, _) = self
                .0
                .signal
                .wait_timeout_while(guard, timeout, |state| !state.done)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.done
        }

        /// Blocks until completion and takes the value.
        #[must_use]
        pub fn wait_blocking(&self) -> Option<T> {
            let guard = self.lock();
            let mut guard = self
                .0
                .signal
                .wait_while(guard, |state| !state.done)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.value.take()
        }

        /// Future resolving to the value once completed (`None` if another consumer took it).
        #[must_use]
        pub fn wait(&self) -> Wait<T> {
            Wait(self.clone())
        }
    }

    pub struct Wait<T>(Slot<T>);

    impl<T> Future for Wait<T> {
        type Output = Option<T>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut state = self.0.lock();
            if state.done {
                Poll::Ready(state.value.take())
            } else {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    struct ChannelState<T> {
        queue: std::collections::VecDeque<T>,
        closed: bool,
        waker: Option<Waker>,
    }

    /// Unbounded multi-message channel with an awaitable receiver: a background thread streams
    /// values to a UI future. Clone the handle to give the sender side to another thread.
    pub struct Channel<T>(Arc<Mutex<ChannelState<T>>>);

    impl<T> Clone for Channel<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }

    impl<T> Default for Channel<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Channel<T> {
        #[must_use]
        pub fn new() -> Self {
            Self(Arc::new(Mutex::new(ChannelState {
                queue: std::collections::VecDeque::new(),
                closed: false,
                waker: None,
            })))
        }

        fn lock(&self) -> MutexGuard<'_, ChannelState<T>> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        /// Queues a value; ignored once the channel is closed.
        pub fn send(&self, value: T) {
            let waker = {
                let mut state = self.lock();
                if state.closed {
                    return;
                }
                state.queue.push_back(value);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }

        /// No more values will come: receivers drain the queue, then get `None`.
        pub fn close(&self) {
            let waker = {
                let mut state = self.lock();
                state.closed = true;
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }

        /// Future of the next value, or `None` once closed and drained.
        #[must_use]
        pub fn recv(&self) -> Recv<T> {
            Recv(self.clone())
        }
    }

    pub struct Recv<T>(Channel<T>);

    impl<T> Future for Recv<T> {
        type Output = Option<T>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut state = self.0.lock();
            if let Some(value) = state.queue.pop_front() {
                Poll::Ready(Some(value))
            } else if state.closed {
                Poll::Ready(None)
            } else {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    /// Runs `job` on a new named thread.
    ///
    /// # Errors
    /// The operating system could not create the thread.
    pub fn spawn(name: &str, job: impl FnOnce() + Send + 'static) -> io::Result<()> {
        std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(job)
            .map(|_| ())
    }

    /// [`spawn`] with a stack of `stack_bytes` (reserved address space: only touched pages cost
    /// memory). Engines run here: an interpreted `engine.wasm` can need far more than the 2 MiB
    /// default, in particular in debug builds.
    ///
    /// # Errors
    /// The operating system could not create the thread.
    pub fn spawn_with_stack(
        name: &str,
        stack_bytes: usize,
        job: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        std::thread::Builder::new()
            .name(name.to_owned())
            .stack_size(stack_bytes)
            .spawn(job)
            .map(|_| ())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn block<F: Future>(fut: F) -> F::Output {
            let mut fut = std::pin::pin!(fut);
            let mut cx = Context::from_waker(Waker::noop());
            loop {
                if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                    return v;
                }
                std::thread::yield_now();
            }
        }

        #[test]
        fn first_completion_wins_and_value_is_awaitable() {
            let slot = Slot::new();
            assert!(slot.complete(1));
            assert!(!slot.complete(2));
            assert_eq!(block(slot.wait()), Some(1));
        }

        #[test]
        fn background_thread_completes_a_blocked_waiter() {
            let slot = Slot::new();
            let remote = slot.clone();
            assert!(
                spawn("test-worker", move || {
                    remote.complete(42);
                })
                .is_ok()
            );
            assert_eq!(slot.wait_blocking(), Some(42));
        }

        #[test]
        fn channel_delivers_in_order_then_ends_when_closed() {
            let channel = Channel::new();
            let remote = channel.clone();
            assert!(
                spawn("test-sender", move || {
                    for n in 0..3 {
                        remote.send(n);
                    }
                    remote.close();
                    remote.send(99); // ignored after close
                })
                .is_ok()
            );
            let mut seen = Vec::new();
            while let Some(n) = block(channel.recv()) {
                seen.push(n);
            }
            assert_eq!(seen, [0, 1, 2]);
            assert_eq!(block(channel.recv()), None);
        }

        #[test]
        fn wait_done_times_out_when_nothing_completes() {
            let slot = Slot::<u8>::new();
            assert!(!slot.wait_done(Duration::from_millis(20)));
            slot.complete(1);
            assert!(slot.wait_done(Duration::from_millis(20)));
        }
    }
}

/// Child processes started by hosts (Flow runs installed tool binaries in `--engine` mode).
pub mod process {
    use std::io::{self, Read, Write};
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// Starts `program` with `args`, writes `input` to its standard input, closes it and returns
    /// what the process printed on standard output. The process is killed after `timeout`.
    ///
    /// # Errors
    /// The process could not start, its streams failed, it exceeded `timeout`
    /// (`io::ErrorKind::TimedOut`) or it exited unsuccessfully.
    pub fn exchange(
        program: &Path,
        args: &[&str],
        input: &str,
        timeout: Duration,
    ) -> io::Result<String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("no stdin"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("no stdout"))?;
        let reader = std::thread::spawn(move || {
            let mut text = String::new();
            stdout.read_to_string(&mut text).map(|_| text)
        });
        // A child that exits early closes the pipe: the exit status below tells why.
        let _ = stdin.write_all(input.as_bytes());
        drop(stdin);
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(io::ErrorKind::TimedOut, "process timed out"));
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let text = reader
            .join()
            .map_err(|_| io::Error::other("reader thread panicked"))??;
        if status.success() {
            Ok(text)
        } else {
            Err(io::Error::other(format!("process exited with {status}")))
        }
    }

    #[cfg(all(test, unix))]
    mod tests {
        use super::*;

        #[test]
        fn exchange_pipes_input_through_a_real_process() {
            let out = exchange(Path::new("/bin/cat"), &[], "ping\n", Duration::from_secs(5));
            assert_eq!(out.unwrap_or_default(), "ping\n");
        }

        #[test]
        fn exchange_kills_a_process_that_exceeds_its_timeout() {
            let err = exchange(
                Path::new("/bin/sleep"),
                &["5"],
                "",
                Duration::from_millis(100),
            );
            assert_eq!(err.map_err(|e| e.kind()), Err(io::ErrorKind::TimedOut));
        }

        #[test]
        fn exchange_reports_a_failing_exit_status() {
            assert!(
                exchange(Path::new("/usr/bin/false"), &[], "", Duration::from_secs(5)).is_err()
            );
        }
    }
}

#[cfg(all(test, unix))]
mod http_stream_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    /// One-shot server: replies with a chunked body, one chunk every 80 ms.
    fn serve(chunks: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| unreachable!("{e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| unreachable!("{e}"));
        std::thread::spawn(move || {
            let Ok((mut socket, _)) = listener.accept() else {
                return;
            };
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request);
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
            for chunk in chunks {
                let _ = socket.write_all(format!("{:x}\r\n{chunk}\r\n", chunk.len()).as_bytes());
                let _ = socket.flush();
                std::thread::sleep(Duration::from_millis(80));
            }
            let _ = socket.write_all(b"0\r\n\r\n");
        });
        format!("http://127.0.0.1:{}/", addr.port())
    }

    fn request(url: &str) -> HttpRequest<'_> {
        HttpRequest {
            method: "POST",
            url,
            headers: &[],
            body: Some(b"{}"),
            timeout: Duration::from_secs(10),
            max_bytes: 1 << 20,
        }
    }

    #[test]
    fn chunks_reach_the_sink_one_by_one_while_the_server_is_still_sending() {
        let url = serve(vec!["data: a\n\n", "data: b\n\n", "data: c\n\n"]);
        let started = std::time::Instant::now();
        let mut arrivals: Vec<(String, Duration)> = Vec::new();
        let mut announced = None;
        let status = http_stream(
            request(&url),
            |code| announced = Some(code),
            |bytes| {
                arrivals.push((
                    String::from_utf8_lossy(bytes).into_owned(),
                    started.elapsed(),
                ));
                true
            },
        );
        assert_eq!(announced, Some(200), "the status is known before the body");
        assert_eq!(status, Ok(200));
        let text: String = arrivals.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(text, "data: a\n\ndata: b\n\ndata: c\n\n");
        assert!(
            arrivals.len() >= 3,
            "received incrementally, not as one buffer: {arrivals:?}"
        );
        assert!(
            arrivals[0].1 < Duration::from_millis(200),
            "the first chunk does not wait for the last"
        );
    }

    #[test]
    fn the_sink_can_stop_the_read_early() {
        let url = serve(vec!["one", "two", "three"]);
        let mut seen = 0;
        let status = http_stream(
            request(&url),
            |_| {},
            |_| {
                seen += 1;
                false
            },
        );
        assert_eq!(status, Ok(200));
        assert_eq!(seen, 1);
    }

    #[test]
    fn remote_plain_http_and_other_methods_are_refused() {
        let mut remote = request("http://example.com/");
        assert!(http_stream(remote_ref(&mut remote), |_| {}, |_| true).is_err());
        let mut get = request("http://127.0.0.1:1/");
        get.method = "GET";
        assert_eq!(
            http_stream(get, |_| {}, |_| true),
            Err("unsupported HTTP method".into())
        );
    }

    fn remote_ref<'a>(r: &'a mut HttpRequest<'_>) -> HttpRequest<'a> {
        HttpRequest {
            method: r.method,
            url: r.url,
            headers: r.headers,
            body: r.body,
            timeout: r.timeout,
            max_bytes: r.max_bytes,
        }
    }
}
