//! Update, the row above About in Settings: omega-zero updating itself from its GitHub
//! releases. A asks GitHub for the latest release; when it is newer than this build, A again
//! downloads its `omega-zero-aarch64`, checks it against the SHA-256 published beside it and
//! that it is an aarch64 program, and puts it in place of the running one, which is kept as
//! `omega-zero.previous`. A once more restarts the handheld onto it. Only omega-zero itself is
//! updated this way: the cores and the card's scripts change with a full release's zip.
//!
//! The work runs on a thread of its own, through `curl`, as the label downloads do; the row
//! reads where it stands each frame.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::json::Json;

/// The repository whose releases omega-zero updates from.
pub const REPO: &str = "MatanItzhaki12/omega-zero";

/// The release's omega-zero for the handheld, and its SHA-256 as `sha256sum` writes it.
pub const ASSET: &str = "omega-zero-aarch64";
pub const ASSET_SUM: &str = "omega-zero-aarch64.sha256";

/// What the running build is: the version it was released as.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the update stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing asked yet.
    Idle,
    Checking,
    /// This build is the latest release, or newer.
    Latest,
    /// A newer release, by its version, with where to fetch it and its sum.
    Available {
        version: String,
        url: String,
        sum_url: String,
    },
    Downloading(String),
    /// The newer release is in place, by its version: a restart runs it.
    Installed(String),
    /// What went wrong, in the row's few capitals.
    Failed(&'static str),
}

impl Update {
    /// What the Update row says on its right.
    pub fn text(&self) -> String {
        match self {
            Update::Idle => format!("V{VERSION}"),
            Update::Checking => "CHECKING…".to_string(),
            Update::Latest => "UP TO DATE".to_string(),
            Update::Available { version, .. } => format!("GET V{version}"),
            Update::Downloading(_) => "DOWNLOADING…".to_string(),
            Update::Installed(_) => "RESTART".to_string(),
            Update::Failed(why) => why.to_string(),
        }
    }

    /// The row's note: what A does now.
    pub fn note(&self) -> String {
        match self {
            Update::Idle | Update::Latest | Update::Failed(_) => {
                format!("A checks for a newer omega-zero than v{VERSION}")
            }
            Update::Checking => "Asking GitHub for the latest release".to_string(),
            Update::Available { version, .. } => format!("A downloads and installs v{version}"),
            Update::Downloading(version) => format!("Downloading v{version}"),
            Update::Installed(version) => format!("A restarts onto v{version}"),
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self, Update::Checking | Update::Downloading(_))
    }
}

/// The update under way, shared with the thread doing it.
#[derive(Clone)]
pub struct Updater {
    state: Arc<Mutex<Update>>,
}

impl Default for Updater {
    fn default() -> Self {
        Updater {
            state: Arc::new(Mutex::new(Update::Idle)),
        }
    }
}

impl Updater {
    pub fn state(&self) -> Update {
        self.state.lock().map(|s| s.clone()).unwrap_or(Update::Idle)
    }

    fn set(&self, to: Update) {
        if let Ok(mut s) = self.state.lock() {
            *s = to;
        }
    }

    /// Asks GitHub for the latest release, on a thread of its own.
    pub fn check(&self) {
        if self.state().busy() {
            return;
        }
        self.set(Update::Checking);
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
                let found = match fetch(&url) {
                    Fetched::Body(body) => match latest(&String::from_utf8_lossy(&body)) {
                        Some((version, url, sum_url)) if newer(&version, VERSION) => {
                            Update::Available {
                                version,
                                url,
                                sum_url,
                            }
                        }
                        Some(_) => Update::Latest,
                        None => Update::Failed("NO RELEASE YET"),
                    },
                    fetched => Update::Failed(fetched.why()),
                };
                me.set(found);
            });
        if spawned.is_err() {
            self.set(Update::Failed("FAILED"));
        }
    }

    /// Downloads the release found and puts it in place of `exe`, on a thread of its own.
    pub fn install(&self, exe: PathBuf) {
        let Update::Available {
            version,
            url,
            sum_url,
        } = self.state()
        else {
            return;
        };
        self.set(Update::Downloading(version.clone()));
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("omega-update".into())
            .spawn(move || {
                let done = match install(&exe, &url, &sum_url) {
                    Ok(()) => Update::Installed(version),
                    Err(why) => {
                        eprintln!("omega-zero: update: {why}");
                        Update::Failed(why)
                    }
                };
                me.set(done);
            });
        if spawned.is_err() {
            self.set(Update::Failed("FAILED"));
        }
    }
}

/// The latest release's version, its omega-zero's address and its sum's, from GitHub's answer
/// for it: none where it has no omega-zero for the handheld.
pub fn latest(body: &str) -> Option<(String, String, String)> {
    let release = Json::parse(body)?;
    let tag = release.get("tag_name")?.text()?;
    let version = tag.trim_start_matches('v').to_string();
    let asset = |name: &str| {
        release
            .get("assets")?
            .list()
            .iter()
            .find(|a| a.get("name").and_then(Json::text) == Some(name))?
            .get("browser_download_url")?
            .text()
            .map(str::to_string)
    };
    Some((version, asset(ASSET)?, asset(ASSET_SUM)?))
}

/// Whether version `a` is newer than `b`, each as dotted numbers: `1.10.0` is newer than
/// `1.9.2`. A part that is not a number counts as none.
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(a) > parts(b)
}

/// Whether `bytes` start as an aarch64 Linux program: a 64-bit little-endian ELF for machine
/// 183. Anything else would leave the handheld with no omega-zero to start.
pub fn is_aarch64_elf(bytes: &[u8]) -> bool {
    bytes.len() > 20
        && bytes[..4] == *b"\x7fELF"
        && bytes[4] == 2
        && bytes[5] == 1
        && u16::from_le_bytes([bytes[18], bytes[19]]) == 183
}

/// The first word of `sum`, a file `sha256sum` wrote, if it is a SHA-256 in hex.
pub fn sum_of(sum: &str) -> Option<String> {
    let hex = sum.split_whitespace().next()?.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn install(exe: &Path, url: &str, sum_url: &str) -> Result<(), &'static str> {
    let sum = fetch(sum_url).body()?;
    let want = sum_of(&String::from_utf8_lossy(&sum)).ok_or("BAD RELEASE")?;
    let bytes = fetch(url).body()?;
    if sha256_hex(&bytes) != want {
        return Err("BAD DOWNLOAD");
    }
    if !is_aarch64_elf(&bytes) {
        return Err("BAD RELEASE");
    }
    let dir = exe.parent().ok_or("FAILED")?;
    let name = exe.file_name().ok_or("FAILED")?.to_string_lossy();
    let fresh = dir.join(format!("{name}.download"));
    let previous = dir.join(format!("{name}.previous"));
    std::fs::write(&fresh, &bytes).map_err(|_| "CARD FULL")?;
    set_executable(&fresh);
    // the running one is kept to go back to; the new one takes its name in one step
    std::fs::copy(exe, &previous).map_err(|_| "CARD FULL")?;
    std::fs::rename(&fresh, exe).map_err(|_| "FAILED")?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

/// What `url` answers, following redirects, or none where it cannot be fetched.
/// What asking GitHub came to.
#[derive(Debug, PartialEq, Eq)]
pub enum Fetched {
    Body(Vec<u8>),
    /// GitHub answered, with this status rather than the thing asked for: 404 for a repository
    /// with no release yet.
    Status(u16),
    /// Nothing answered: no network, or none that reaches GitHub.
    Unreachable,
}

impl Fetched {
    /// What the row says for an answer that is not the thing asked for.
    pub fn why(&self) -> &'static str {
        match self {
            Fetched::Body(_) => "FAILED",
            Fetched::Status(404) => "NO RELEASE YET",
            Fetched::Status(403 | 429) => "GITHUB BUSY",
            Fetched::Status(_) => "GITHUB ERROR",
            Fetched::Unreachable => "NO NETWORK",
        }
    }

    fn body(self) -> Result<Vec<u8>, &'static str> {
        match self {
            Fetched::Body(body) => Ok(body),
            other => Err(other.why()),
        }
    }
}

/// Splits what `curl -w '\n%{http_code}'` wrote into the body and the status after it.
pub fn answered(out: &[u8]) -> Fetched {
    let Some(split) = out.iter().rposition(|&b| b == b'\n') else {
        return Fetched::Unreachable;
    };
    let status: u16 = std::str::from_utf8(&out[split + 1..])
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    match status {
        0 => Fetched::Unreachable,
        200..=299 => Fetched::Body(out[..split].to_vec()),
        status => Fetched::Status(status),
    }
}

/// What `url` answers, following redirects.
fn fetch(url: &str) -> Fetched {
    let child = Command::new("curl")
        .args([
            "-sL",
            "-m",
            "300",
            "-A",
            "omega-zero",
            "-w",
            "\n%{http_code}",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return Fetched::Unreachable;
    };
    let mut out = Vec::new();
    let read = child
        .stdout
        .take()
        .is_some_and(|mut s| s.read_to_end(&mut out).is_ok());
    let _ = child.wait();
    match read {
        true => answered(&out),
        false => Fetched::Unreachable,
    }
}

/// `bytes`' SHA-256, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    let bits = (bytes.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_be_bytes());
    for block in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            sha256_hex(long),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn a_version_is_newer_by_its_numbers_not_its_letters() {
        assert!(newer("1.3.0", "1.2.0"));
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("2.0.0", "1.99.99"));
        assert!(!newer("1.2.0", "1.2.0"));
        assert!(!newer("1.1.9", "1.2.0"));
    }

    #[test]
    fn the_latest_release_names_its_omega_zero_and_sum() {
        let body = r#"{"tag_name":"v1.3.0","assets":[
            {"name":"omega-zero-v1.3.0-h700.zip","browser_download_url":"https://x/zip"},
            {"name":"omega-zero-aarch64","browser_download_url":"https://x/bin"},
            {"name":"omega-zero-aarch64.sha256","browser_download_url":"https://x/sum"}]}"#;
        assert_eq!(
            latest(body),
            Some((
                "1.3.0".to_string(),
                "https://x/bin".to_string(),
                "https://x/sum".to_string()
            ))
        );
        let zip_only = r#"{"tag_name":"v1.3.0","assets":[
            {"name":"omega-zero-v1.3.0-h700.zip","browser_download_url":"https://x/zip"}]}"#;
        assert_eq!(
            latest(zip_only),
            None,
            "a release with no omega-zero of its own"
        );
    }

    #[test]
    fn only_an_aarch64_program_is_put_in_place() {
        let mut elf = vec![0u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18..20].copy_from_slice(&183u16.to_le_bytes());
        assert!(is_aarch64_elf(&elf));
        elf[18..20].copy_from_slice(&62u16.to_le_bytes());
        assert!(!is_aarch64_elf(&elf), "an x86-64 program");
        assert!(!is_aarch64_elf(b"<html>not found</html>"));
    }

    #[test]
    fn github_with_no_release_is_not_a_missing_network() {
        assert_eq!(answered(b"{}\n200"), Fetched::Body(b"{}".to_vec()));
        assert_eq!(
            answered(b"{\"message\":\"Not Found\"}\n404").why(),
            "NO RELEASE YET"
        );
        assert_eq!(answered(b"\n403").why(), "GITHUB BUSY");
        assert_eq!(answered(b"\n000").why(), "NO NETWORK");
        assert_eq!(answered(b"").why(), "NO NETWORK");
    }

    #[test]
    fn a_sum_file_gives_its_hex() {
        let hex = "ab".repeat(32);
        assert_eq!(sum_of(&format!("{hex}  omega-zero-aarch64\n")), Some(hex));
        assert_eq!(sum_of("Not Found"), None);
    }
}
