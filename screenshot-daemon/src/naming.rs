//! Output filename generation for screenshots and recordings.
//!
//! Filenames combine a local date+time stamp with a short random token so
//! that rapid successive captures never produce identical paths. A previous
//! implementation used second-level timestamps alone, which caused new files
//! to overwrite older ones when two captures landed in the same wall-clock
//! second.

use std::io::Read;
use std::path::{Path, PathBuf};

/// Alphabet used for the random token suffix: letters + digits. Filename-safe
/// and unambiguous in a terminal.
const TOKEN_ALPHABET: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
/// Length of the random token. 6 chars over 62 symbols gives ~5.7e10
/// combinations — collision probability among realistic screenshot counts
/// is negligible.
const TOKEN_LEN: usize = 6;
/// Number of fresh random attempts before falling back to a nanosecond
/// suffix. Each attempt is an independent draw, so this is purely defensive
/// against the astronomically unlikely collision.
const MAX_RANDOM_ATTEMPTS: usize = 16;

/// Build a unique output path under `save_dir` for the given `prefix` and
/// `ext` (extension without the leading dot, e.g. `"png"`, `"webm"`).
///
/// The resulting filename has the shape:
/// ```text
/// <prefix>_<YYYYMMDD>_<HHMMSS>_<random6>.<ext>
/// ```
/// e.g. `screenshot_20260707_095012_aB3xK9.png`,
///      `recording_20260707_095012_qW8mP2.webm`.
///
/// The random token is drawn from `/dev/urandom` (this is a Linux-only
/// tool). If — by extreme coincidence — the generated path already exists,
/// a new token is drawn, up to [`MAX_RANDOM_ATTEMPTS`] times, before
/// falling back to a nanosecond-qualified name so an existing file is never
/// silently overwritten.
///
/// The timestamp is read with [`chrono::Local::now`] on every call, so the
/// date/time portion always reflects the moment of capture rather than the
/// daemon's start time.
///
/// The caller is responsible for ensuring `save_dir` exists; this function
/// only picks a path and never writes to disk.
pub fn unique_path(save_dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    let now = chrono::Local::now();
    let stamp = now.format("%Y%m%d_%H%M%S");

    for _ in 0..MAX_RANDOM_ATTEMPTS {
        let token = random_token();
        let candidate = save_dir.join(format!("{prefix}_{stamp}_{token}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    // Practically unreachable: 16 fresh 6-char tokens all collided with
    // existing files. Disambiguate with nanoseconds so we never overwrite.
    let nanos = now.timestamp_nanos_opt().unwrap_or(0);
    save_dir.join(format!("{prefix}_{stamp}_n{nanos}.{ext}"))
}

/// Generate a short random alphanumeric token by sampling `/dev/urandom`.
fn random_token() -> String {
    let mut buf = [0u8; TOKEN_LEN];
    read_urandom(&mut buf);
    // Modulo bias over 62 symbols from 256 values is negligible for
    // filename uniqueness (this is not a security-sensitive context).
    let mut out = String::with_capacity(TOKEN_LEN);
    for b in buf {
        out.push(TOKEN_ALPHABET[b as usize % TOKEN_ALPHABET.len()] as char);
    }
    out
}

/// Read exactly `buf.len()` cryptographically-random bytes from
/// `/dev/urandom`. Panics only if `/dev/urandom` is unavailable or short,
/// which on a functioning Linux system never happens after early boot.
fn read_urandom(buf: &mut [u8]) {
    let mut f = std::fs::File::open("/dev/urandom").expect("open /dev/urandom");
    f.read_exact(buf).expect("read /dev/urandom");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "screenshot-daemon-naming-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&dir).expect("create_dir_all");
        dir
    }

    #[test]
    fn path_has_date_time_random_shape() {
        let dir = fresh_dir();
        let path = unique_path(&dir, "screenshot", "png");
        assert!(path.starts_with(&dir), "path should live under save_dir");

        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("screenshot_"), "name={name}");
        assert!(name.ends_with(".png"), "name={name}");

        // Strip prefix and extension, leaving <YYYYMMDD>_<HHMMSS>_<token>.
        let core = &name["screenshot_".len()..name.len() - ".png".len()];
        let parts: Vec<&str> = core.split('_').collect();
        assert_eq!(
            parts.len(),
            3,
            "core should split into date/time/token: core={core}"
        );
        let (date, time, token) = (parts[0], parts[1], parts[2]);

        assert_eq!(date.len(), 8, "date={date}");
        assert!(date.chars().all(|c| c.is_ascii_digit()), "date={date}");
        assert_eq!(time.len(), 6, "time={time}");
        assert!(time.chars().all(|c| c.is_ascii_digit()), "time={time}");
        assert_eq!(token.len(), TOKEN_LEN, "token={token}");
        assert!(
            token.chars().all(|c| c.is_ascii_alphanumeric()),
            "token={token}"
        );

        assert!(!path.exists(), "fresh path must not already exist");
    }

    #[test]
    fn two_consecutive_calls_produce_different_names() {
        // The random token makes a collision between two independent draws
        // astronomically unlikely (1 / 62^6 ~= 1.75e-11).
        let dir = fresh_dir();
        let a = unique_path(&dir, "region", "png");
        let b = unique_path(&dir, "region", "png");
        assert_ne!(
            a, b,
            "consecutive calls must differ via the random token"
        );
    }

    #[test]
    fn never_returns_an_existing_path() {
        let dir = fresh_dir();
        let first = unique_path(&dir, "recording", "webm");
        std::fs::write(&first, b"seed").expect("write seed");

        let second = unique_path(&dir, "recording", "webm");
        assert_ne!(first, second, "must not collide with existing file");
        assert!(!second.exists(), "returned path must not already exist");
    }

    #[test]
    fn random_token_is_alphanumeric_of_fixed_length() {
        for _ in 0..256 {
            let t = random_token();
            assert_eq!(t.len(), TOKEN_LEN, "token={t}");
            assert!(t.chars().all(|c| c.is_ascii_alphanumeric()), "token={t}");
        }
    }
}
