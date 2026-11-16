use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// Default photo and camera raw extensions
pub const DEFAULT_PHOTO_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "tif", "heic", "heif", "avif", "raw",
    "cr2", "nef", "arw", "dng",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateStatus {
    Ready,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct RenameCandidate {
    pub source: PathBuf,
    pub target: PathBuf,
    pub status: CandidateStatus,
}

#[derive(Debug, Default, Clone)]
pub struct ApplySummary {
    pub renamed: usize,
    pub skipped_conflicts: usize,
    pub errors: Vec<String>,
}

// Check if an extension matches the supported photo list regardless of case
pub fn is_photo_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    DEFAULT_PHOTO_EXTENSIONS.contains(&lower.as_str())
}

// Build the target path with a lowercase extension while keeping the stem intact
fn lowercased_ext_path(path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?;
    if !is_photo_extension(ext) {
        return None;
    }

    let lower_ext = ext.to_ascii_lowercase();
    if ext == lower_ext {
        return None;
    }

    let stem = path.file_stem()?.to_str()?;
    let new_name = format!("{stem}.{lower_ext}");
    Some(path.with_file_name(new_name))
}

// Scan a folder and return all photo files whose extension is not lowercase yet
pub fn scan_folder(root: &Path, recursive: bool) -> Vec<RenameCandidate> {
    if !root.is_dir() {
        return Vec::new();
    }

    let walker = if recursive {
        WalkDir::new(root)
    } else {
        WalkDir::new(root).max_depth(1)
    };

    let mut dir_entries_cache: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut candidates = Vec::new();

    for entry in walker.into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let source = entry.into_path();
        let Some(target) = lowercased_ext_path(&source) else {
            continue;
        };

        let status = match (source.parent(), target.file_name().and_then(|n| n.to_str())) {
            (Some(parent), Some(target_name)) => {
                let names = dir_entries_cache
                    .entry(parent.to_path_buf())
                    .or_insert_with(|| collect_exact_filenames(parent));
                if names.contains(target_name) {
                    CandidateStatus::Conflict
                } else {
                    CandidateStatus::Ready
                }
            }
            _ => CandidateStatus::Conflict,
        };

        candidates.push(RenameCandidate {
            source,
            target,
            status,
        });
    }

    candidates.sort_by(|a, b| a.source.cmp(&b.source));
    candidates
}

// Read exact filenames in a directory so collision checks work on case-insensitive filesystems too
fn collect_exact_filenames(dir: &Path) -> HashSet<String> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return HashSet::new();
    };

    read_dir
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

// Rename a single file and fall back to a two-step rename on case-insensitive filesystems
fn rename_case_safe(source: &Path, target: &Path) -> std::io::Result<()> {
    if !target.exists() {
        return fs::rename(source, target);
    }

    // Target reports as existing on case-insensitive filesystems so hop through a temp name
    let parent = source
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("invalid filename"))?;

    let mut temp_path = parent.join(format!(".{file_name}.demultimedia_tmp"));
    let mut counter = 0u32;
    while temp_path.exists() {
        counter += 1;
        temp_path = parent.join(format!(".{file_name}.demultimedia_tmp_{counter}"));
    }

    fs::rename(source, &temp_path)?;
    if let Err(err) = fs::rename(&temp_path, target) {
        let _ = fs::rename(&temp_path, source);
        return Err(err);
    }
    Ok(())
}

// Apply all ready renames and leave conflicts untouched
pub fn apply_renames(candidates: &[RenameCandidate]) -> ApplySummary {
    let mut summary = ApplySummary::default();

    for item in candidates {
        if item.status != CandidateStatus::Ready {
            summary.skipped_conflicts += 1;
            continue;
        }

        match rename_case_safe(&item.source, &item.target) {
            Ok(()) => summary.renamed += 1,
            Err(err) => summary
                .errors
                .push(format!("{}: {err}", item.source.display())),
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("demultimedia_test_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    #[test]
    fn scans_and_applies_photo_extensions_only() {
        let root = make_temp_dir("basic");
        fs::write(root.join("IMG_001.JPG"), b"a").unwrap();
        fs::write(root.join("Screenshot.Png"), b"b").unwrap();
        fs::write(root.join("already.webp"), b"c").unwrap();
        fs::write(root.join("notes.TXT"), b"d").unwrap();

        let candidates = scan_folder(&root, false);
        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|c| c.status == CandidateStatus::Ready));

        let summary = apply_renames(&candidates);
        assert_eq!(summary.renamed, 2);
        assert_eq!(summary.skipped_conflicts, 0);
        assert!(summary.errors.is_empty());

        assert!(root.join("IMG_001.jpg").exists());
        assert!(root.join("Screenshot.png").exists());
        assert!(root.join("already.webp").exists());
        assert!(root.join("notes.TXT").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_conflicts_without_overwriting() {
        let root = make_temp_dir("conflict");
        fs::write(root.join("photo.JPG"), b"upper").unwrap();
        fs::write(root.join("photo.jpg"), b"lower").unwrap();

        let candidates = scan_folder(&root, false);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, CandidateStatus::Conflict);

        let summary = apply_renames(&candidates);
        assert_eq!(summary.renamed, 0);
        assert_eq!(summary.skipped_conflicts, 1);
        assert_eq!(fs::read(root.join("photo.jpg")).unwrap(), b"lower");
        assert_eq!(fs::read(root.join("photo.JPG")).unwrap(), b"upper");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn respects_recursive_flag() {
        let root = make_temp_dir("recursive");
        let sub = root.join("nested");
        fs::create_dir_all(&sub).unwrap();
        fs::write(root.join("top.JPEG"), b"1").unwrap();
        fs::write(sub.join("deep.HEIC"), b"2").unwrap();

        let shallow = scan_folder(&root, false);
        assert_eq!(shallow.len(), 1);

        let deep = scan_folder(&root, true);
        assert_eq!(deep.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }
}
