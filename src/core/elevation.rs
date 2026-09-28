//! Elevation broker for the few admin-only steps (Revive install, artwork into Program Files).
//!
//! Operations are first tried in-process. Only if one fails for lack of rights does the
//! app ask for consent and relaunch *itself* elevated as `--admin-helper <pipe> <pid>`;
//! the helper is then reused for the rest of the session (one UAC prompt).
//!
//! Trust model (replacing the Java localhost socket, whose token sat in a user-readable
//! temp file so any same-user process could connect first and run anything as admin):
//! * the unelevated app creates the pipe with a random name, `FILE_FLAG_FIRST_PIPE_INSTANCE`
//!   (nobody can pre-create or share it) and an ACL for only the current user and the
//!   Administrators group; remote clients are rejected;
//! * after the connect, the app checks the client PID is the process it just launched,
//!   and the helper checks the server PID is its parent;
//! * the helper honours a fixed set of operations whose inputs it validates itself: the
//!   installer must hash to the pinned Revive build (checked through a handle that denies
//!   writes while it runs), and the artwork is downloaded by the helper and extracted,
//!   Zip-Slip-safe, into one fixed folder.

use std::io::{Read, Write};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const HELPER_FLAG: &str = "--admin-helper";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    Ping,
    /// Run the Revive installer at this path -- only if it is the pinned build.
    RunReviveInstaller {
        path: String,
    },
    InstallArtwork,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    Ok,
    ExitCode(i32),
    Err(String),
}

#[cfg_attr(not(windows), allow(dead_code))]
const MAX_MESSAGE: u32 = 64 * 1024;

#[cfg_attr(not(windows), allow(dead_code))]
pub fn write_msg<T: Serialize>(w: &mut impl Write, msg: &T) -> Result<()> {
    let body = serde_json::to_vec(msg)?;
    w.write_all(&(body.len() as u32).to_le_bytes())?;
    w.write_all(&body)?;
    w.flush()?;
    Ok(())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn read_msg<T: for<'de> Deserialize<'de>>(r: &mut impl Read) -> Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len);
    if len > MAX_MESSAGE {
        bail!("oversized message ({len} bytes)");
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}

/// Pipe names are ours only: `\\.\pipe\echovr-installer-<32 hex>`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn valid_pipe_name(name: &str) -> bool {
    name.strip_prefix(r"\\.\pipe\echovr-installer-")
        .is_some_and(|rest| rest.len() == 32 && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

/// What the helper does for one request. Platform-independent so it can be tested.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn handle(req: &Request) -> Reply {
    match req {
        Request::Ping | Request::Shutdown => Reply::Ok,
        Request::RunReviveInstaller { path } => match run_pinned_installer(path) {
            Ok(code) => Reply::ExitCode(code),
            Err(e) => Reply::Err(format!("{e:#}")),
        },
        Request::InstallArtwork => {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            match super::revive::install_artwork(&cancel) {
                Ok(()) => Reply::Ok,
                Err(e) => Reply::Err(format!("{e:#}")),
            }
        }
    }
}

/// Opens the installer so nobody can modify it, verifies it is the pinned Revive build,
/// then runs it silently while still holding the file.
fn run_pinned_installer(path: &str) -> Result<i32> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        opts.share_mode(FILE_SHARE_READ);
    }
    let mut f = opts.open(path)?;
    if !super::download::sha256_reader(&mut f)?
        .eq_ignore_ascii_case(super::revive::REVIVE_INSTALLER_SHA256)
    {
        bail!("refusing to run an installer that is not the pinned Revive build");
    }
    let status = super::process::command(path).arg("/S").status()?;
    drop(f);
    Ok(status.code().unwrap_or(-1))
}

// ---------------------------------------------------------------------------------------
// Client side (the normal, unelevated app)
// ---------------------------------------------------------------------------------------

/// Runs the Revive installer, elevating (after `consent`) if launching it needs admin.
pub fn run_revive_installer(
    path: &std::path::Path,
    consent: &mut dyn FnMut() -> bool,
) -> Result<i32> {
    match run_pinned_installer(&path.to_string_lossy()) {
        Ok(code) => Ok(code),
        Err(e) if super::revive::needs_elevation(&e) => {
            tracing::info!("installer needs elevation ({e:#}); using the helper");
            match broker::request(
                &Request::RunReviveInstaller {
                    path: path.to_string_lossy().into(),
                },
                consent,
            )? {
                Reply::ExitCode(c) => Ok(c),
                Reply::Ok => Ok(0),
                Reply::Err(m) => bail!("{m}"),
            }
        }
        Err(e) => Err(e),
    }
}

/// Installs the Revive artwork, elevating (after `consent`) if Program Files is locked.
pub fn install_artwork(consent: &mut dyn FnMut() -> bool) -> Result<()> {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match super::revive::install_artwork(&cancel) {
        Ok(()) => Ok(()),
        Err(e) if super::revive::needs_elevation(&e) => {
            tracing::info!("artwork needs elevation ({e:#}); using the helper");
            match broker::request(&Request::InstallArtwork, consent)? {
                Reply::Err(m) => bail!("{m}"),
                _ => Ok(()),
            }
        }
        Err(e) => Err(e),
    }
}

/// Tells a running helper to exit (on app shutdown).
pub fn shutdown() {
    broker::shutdown();
}

#[cfg(not(windows))]
mod broker {
    use super::{Reply, Request};
    use anyhow::{bail, Result};

    pub fn request(_req: &Request, _consent: &mut dyn FnMut() -> bool) -> Result<Reply> {
        bail!("Administrator elevation is only supported on Windows.")
    }

    pub fn shutdown() {}
}

#[cfg(windows)]
mod broker {
    use super::{read_msg, valid_pipe_name, write_msg, Reply, Request, HELPER_FLAG};
    use anyhow::{anyhow, bail, Context, Result};
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::Authorization::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::Storage::FileSystem::*;
    use windows_sys::Win32::System::Pipes::*;
    use windows_sys::Win32::System::Threading::*;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    pub(super) fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Conn {
        pipe: File,
    }

    static CONN: Mutex<Option<Conn>> = Mutex::new(None);

    pub fn request(req: &Request, consent: &mut dyn FnMut() -> bool) -> Result<Reply> {
        let mut guard = CONN.lock().unwrap_or_else(|p| p.into_inner());
        // Reuse a live helper.
        if let Some(c) = guard.as_mut() {
            match exchange(&mut c.pipe, req) {
                Ok(r) => return Ok(r),
                Err(e) => {
                    tracing::warn!("elevated helper lost ({e:#}); relaunching");
                    *guard = None;
                }
            }
        }
        if !consent() {
            bail!("Administrator rights were declined.");
        }
        let pipe = launch().context("Couldn't start the elevated helper")?;
        *guard = Some(Conn { pipe });
        exchange(&mut guard.as_mut().expect("just set").pipe, req)
    }

    fn exchange(pipe: &mut File, req: &Request) -> Result<Reply> {
        write_msg(pipe, req)?;
        read_msg(pipe)
    }

    pub fn shutdown() {
        let mut guard = CONN.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = guard.as_mut() {
            let _ = write_msg(&mut c.pipe, &Request::Shutdown);
        }
        *guard = None;
    }

    /// "S-1-5-21-..." of the current user.
    fn current_user_sid() -> Result<String> {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                bail!("OpenProcessToken failed ({})", GetLastError());
            }
            let mut len = 0u32;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
            let mut buf = vec![0u8; len as usize];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
            CloseHandle(token);
            if ok == 0 {
                bail!("GetTokenInformation failed ({})", GetLastError());
            }
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
                bail!("ConvertSidToStringSidW failed ({})", GetLastError());
            }
            let mut n = 0;
            while *s.add(n) != 0 {
                n += 1;
            }
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
            LocalFree(s.cast());
            Ok(sid)
        }
    }

    fn create_pipe(name: &str) -> Result<HANDLE> {
        let sddl = format!("D:P(A;;GA;;;{})(A;;GA;;;BA)", current_user_sid()?);
        unsafe {
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(&sddl).as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            ) == 0
            {
                bail!("security descriptor ({})", GetLastError());
            }
            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd,
                bInheritHandle: 0,
            };
            let h = CreateNamedPipeW(
                wide(name).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                64 * 1024,
                64 * 1024,
                0,
                &sa,
            );
            LocalFree(sd);
            if h == INVALID_HANDLE_VALUE {
                bail!("CreateNamedPipeW failed ({})", GetLastError());
            }
            Ok(h)
        }
    }

    /// Launches the elevated helper and returns the verified pipe connected to it.
    fn launch() -> Result<File> {
        let name = format!(
            r"\\.\pipe\echovr-installer-{}",
            hex::encode(rand::random::<[u8; 16]>())
        );
        debug_assert!(valid_pipe_name(&name));
        let pipe = create_pipe(&name)?;
        // SAFETY: we own the handle; File closes it.
        let pipe_file = unsafe { File::from_raw_handle(pipe as _) };

        let exe = std::env::current_exe()?;
        let params = format!("{HELPER_FLAG} {name} {}", std::process::id());
        let verb = wide("runas");
        let file = wide(&exe.to_string_lossy());
        let params_w = wide(&params);
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params_w.as_ptr();
        info.nShow = SW_HIDE;
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_CANCELLED {
                bail!("The Windows administrator prompt was declined.");
            }
            bail!("ShellExecuteExW failed ({err})");
        }
        let child = info.hProcess;
        let child_pid = unsafe { GetProcessId(child) };

        // ConnectNamedPipe blocks; run it on a thread so a helper that dies (or never
        // starts) can be noticed. We unblock it by connecting to ourselves if needed.
        let raw = pipe as usize;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let ok = unsafe { ConnectNamedPipe(raw as HANDLE, std::ptr::null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            let _ = tx.send(ok);
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        let connected = loop {
            if let Ok(ok) = rx.recv_timeout(Duration::from_millis(200)) {
                break ok;
            }
            let exited = unsafe { WaitForSingleObject(child, 0) } == WAIT_OBJECT_0;
            if exited || Instant::now() >= deadline {
                // Unblock the connect thread, then reject whatever connected.
                let _ = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&name);
                let _ = rx.recv_timeout(Duration::from_secs(2));
                unsafe { CloseHandle(child) };
                bail!("the elevated helper did not start");
            }
        };
        if !connected {
            unsafe { CloseHandle(child) };
            bail!("ConnectNamedPipe failed");
        }
        let mut client_pid = 0u32;
        let ok = unsafe { GetNamedPipeClientProcessId(pipe, &mut client_pid) } != 0;
        unsafe { CloseHandle(child) };
        if !ok || client_pid != child_pid {
            bail!("an unexpected process connected to the helper pipe (pid {client_pid})");
        }
        let mut f = pipe_file;
        match exchange(&mut f, &Request::Ping)? {
            Reply::Ok => Ok(f),
            other => Err(anyhow!("unexpected helper reply {other:?}")),
        }
    }

    /// The elevated side: connect, verify the server is our parent, serve requests.
    pub fn helper_main(pipe_name: &str, parent_pid: u32) -> Result<()> {
        if !valid_pipe_name(pipe_name) {
            bail!("invalid pipe name");
        }
        // Exit as soon as the parent goes away.
        let parent = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_pid) };
        if parent.is_null() {
            bail!("parent process {parent_pid} not found");
        }
        let parent_raw = parent as usize;
        std::thread::spawn(move || {
            unsafe { WaitForSingleObject(parent_raw as HANDLE, INFINITE) };
            tracing::info!("parent gone -- exiting");
            std::process::exit(0);
        });

        let mut pipe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(pipe_name)
            .context("open helper pipe")?;
        let mut server_pid = 0u32;
        let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&pipe);
        if unsafe { GetNamedPipeServerProcessId(handle as HANDLE, &mut server_pid) } == 0
            || server_pid != parent_pid
        {
            bail!("pipe server is not our parent (pid {server_pid})");
        }
        tracing::info!("helper connected to {pipe_name}");
        loop {
            let req: Request = match read_msg(&mut pipe) {
                Ok(r) => r,
                Err(_) => return Ok(()), // parent closed the pipe
            };
            tracing::info!("helper op: {req:?}");
            let reply = super::handle(&req);
            write_msg(&mut pipe, &reply)?;
            if req == Request::Shutdown {
                return Ok(());
            }
        }
    }
}

/// Entry point for `--admin-helper <pipe> <parent-pid>`.
pub fn helper_main(args: &[String]) -> i32 {
    #[cfg(windows)]
    {
        let (Some(pipe), Some(pid)) = (args.get(2), args.get(3).and_then(|p| p.parse().ok()))
        else {
            return 2;
        };
        match broker::helper_main(pipe, pid) {
            Ok(()) => 0,
            Err(e) => {
                tracing::error!("helper: {e:#}");
                3
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_round_trip() {
        let mut buf = Vec::new();
        let req = Request::RunReviveInstaller {
            path: "C:\\x\\ReviveInstaller.exe".into(),
        };
        write_msg(&mut buf, &req).unwrap();
        let back: Request = read_msg(&mut buf.as_slice()).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn rejects_oversized_messages() {
        let mut buf = (MAX_MESSAGE + 1).to_le_bytes().to_vec();
        buf.extend(vec![b' '; 16]);
        assert!(read_msg::<Request>(&mut buf.as_slice()).is_err());
    }

    #[test]
    fn pipe_name_validation() {
        assert!(valid_pipe_name(
            r"\\.\pipe\echovr-installer-0123456789abcdef0123456789abcdef"
        ));
        assert!(!valid_pipe_name(r"\\.\pipe\other"));
        assert!(!valid_pipe_name(r"\\.\pipe\echovr-installer-xyz"));
    }

    #[test]
    fn helper_refuses_unpinned_installer() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("ReviveInstaller.exe");
        std::fs::write(&fake, b"not revive").unwrap();
        match handle(&Request::RunReviveInstaller {
            path: fake.to_string_lossy().into(),
        }) {
            Reply::Err(m) => assert!(m.contains("pinned")),
            other => panic!("expected refusal, got {other:?}"),
        }
    }
}
