//! Output filename generation for screenshots and recordings.
//!
//! Prior implementations stamped filenames with second-level precision
//! (`%Y%m%d_%H%M%S`). Two captures taken within the same wall-clock second
//! produced identical paths, so the newer file silently overwrote the older
//! one. This module guarantees uniqueness by combining millisecond-precise
//! timestamps with a counter-based collision resolver.

use std::path::{Path, PathBuf};

/// Build a unique output path under `save_dir` for the given `prefix` and
/// `ext` (extension without the leading dot, e.g. `"png"`, `"webm"`).
///
/// The base filename is `<prefix>_<local-timestamp>` where the timestamp
/// carries millisecond precision (`%Y%m%d_%H%M%S%3f`). If a file already
/// exists at the candidate path — e.g. two captures inside the same
/// millisecond, or a previously saved file with the same name — a counter
/// suffix `_2`, `_3`, ... is appended until a free slot is found.
///
/// The caller is responsible for ensuring `save_dir` exists; this function
/// only picks a path and never writes to disk.
pub fn unique_path(save_dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S%3f");
    let base = format!("{prefix}_{timestamp}");
    resolve_unique(save_dir, &base, ext)
}

/// Pick a non-existing path of the form `<base>.<ext>`, falling back to
/// `<base>_2.<ext>`, `<base>_3.<ext>`, ... until a free slot is found.
///
/// Factored out of [`unique_path`] so the collision logic can be exercised
/// deterministically without racing the wall clock.
pub(crate) fn resolve_unique(save_dir: &Path, base: &str, ext: &str) -> PathBuf {
    let primary = save_dir.join(format!("{base}.{ext}"));
    if !primary.exists() {
        return primary;
    }

    for counter in 2..=u32::MAX {
        let candidate = save_dir.join(format!("{base}_{counter}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    // Practically unreachable (would require ~4 billion collisions); the
    // nanosecond suffix keeps the uniqueness guarantee intact rather than
    // silently overwriting.
    let nanos = chrono::Local::now()
        .timestamp_nanos_opt()
        .unwrap_or(0);
    save_dir.join(format!("{base}_n{nanos}.{ext}"))
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
    fn primary_path_has_expected_shape() {
        let dir = fresh_dir();
        let path = unique_path(&dir, "screenshot", "png");
        assert!(path.starts_with(&dir), "path should live under save_dir");

        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("screenshot_"), "name={name}");
        assert!(name.ends_with(".png"), "name={name}");

        // timestamp portion is YYYYMMDD_HHMMSSmmm = 8 digits + '_' + 9 digits
        let ts = &name["screenshot_".len()..name.len() - ".png".len()];
        let (date, rest) = ts
            .split_once('_')
            .unwrap_or_else(|| panic!("timestamp must contain '_': ts={ts}"));
        assert_eq!(date.len(), 8, "date={date}");
        assert_eq!(rest.len(), 9, "time+ms={rest}");
        assert!(date.chars().all(|c| c.is_ascii_digit()), "date={date}");
        assert!(rest.chars().all(|c| c.is_ascii_digit()), "rest={rest}");

        assert!(!path.exists(), "fresh path must not already exist");
    }

    #[test]
    fn resolve_unique_returns_primary_when_free() {
        let dir = fresh_dir();
        let got = resolve_unique(&dir, "region_20260707_090202123", "png");
        assert_eq!(
            got.file_name().unwrap().to_string_lossy(),
            "region_20260707_090202123.png"
        );
    }

    #[test]
    fn resolve_unique_appends_counter_on_collision() {
        let dir = fresh_dir();
        let base = "recording_20260707_090202500";

        // Occupy the primary slot.
        std::fs::write(dir.join(format!("{base}.webm")), b"first").expect("write primary");

        let got = resolve_unique(&dir, base, "webm");
        assert_eq!(
            got.file_name().unwrap().to_string_lossy(),
            "recording_20260707_090202500_2.webm"
        );
        assert!(!got.exists(), "resolved path must not already exist");
    }

    #[test]
    fn resolve_unique_skips_occupied_counters() {
        let dir = fresh_dir();
        let base = "screenshot_20260707_090202999";

        // Occupy primary, _2, and _3.
        std::fs::write(dir.join(format!("{base}.png")), b"a").expect("write primary");
        std::fs::write(dir.join(format!("{base}_2.png")), b"b").expect("write _2");
        std::fs::write(dir.join(format!("{base}_3.png")), b"c").expect("write _3");

        let got = resolve_unique(&dir, base, "png");
        assert_eq!(
            got.file_name().unwrap().to_string_lossy(),
            "screenshot_20260707_090202999_4.png"
        );
    }

    #[test]
    fn unique_path_never_overwrites_existing_file() {
        let dir = fresh_dir();
        let first = unique_path(&dir, "region", "png");
        std::fs::write(&first, b"seed").expect("write seed");

        // Even if the wall clock returns the same millisecond, the resolver
        // must avoid the occupied path.
        let second = unique_path(&dir, "region", "png");
        assert_ne!(first, second, "second must not equal first");
        assert!(!second.exists(), "second must not point at an existing file");
    }
}
