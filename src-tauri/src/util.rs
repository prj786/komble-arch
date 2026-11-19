use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── work that must outlive the window ───────────────────────────────────────
// Komble quits when its window closes (lib.rs): nothing of it needs to stay
// resident — the bar's update indicator is the shell's own (updates-check.sh).
// The one exception is a transaction in flight: a pacman run under pkexec, a
// makepkg build, an AppImage download or extraction must finish, so every
// such stretch holds a `Working` guard and the process lingers (windowless)
// until the last one drops.
static WORKING: AtomicUsize = AtomicUsize::new(0);

/// Held across a transaction; dropping it releases the count.
pub struct Working(());

impl Working {
    pub fn begin() -> Self {
        WORKING.fetch_add(1, Ordering::SeqCst);
        Working(())
    }
}

impl Drop for Working {
    fn drop(&mut self) {
        WORKING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Is a transaction in flight?
pub fn working() -> bool {
    WORKING.load(Ordering::SeqCst) > 0
}

/// Shared HTTP client (rustls). No total timeout: AppImage downloads can be large.
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("komble/0.1.0")
            .connect_timeout(Duration::from_secs(20))
            .build()
            .expect("failed to build http client")
    })
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn estr<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Lowercase, alphanumeric + single dashes. Used for ids and file names.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("app");
    }
    out
}

pub fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(bin).is_file()))
        .unwrap_or(false)
}
