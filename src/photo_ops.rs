use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder, ImageFormat};
use walkdir::WalkDir;

// Supported input photo extensions
pub const INPUT_PHOTO_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "tif", "ico", "tga", "qoi", "pnm",
    "farbfeld",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    KeepOriginal,
    Jpg,
    Png,
    Webp,
    Gif,
    Bmp,
    Tiff,
    Svg,
}

impl OutputFormat {
    pub const ALL: &'static [OutputFormat] = &[
        OutputFormat::KeepOriginal,
        OutputFormat::Jpg,
        OutputFormat::Png,
        OutputFormat::Webp,
        OutputFormat::Gif,
        OutputFormat::Bmp,
        OutputFormat::Tiff,
        OutputFormat::Svg,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OutputFormat::KeepOriginal => "Keep original",
            OutputFormat::Jpg => "JPG",
            OutputFormat::Png => "PNG",
            OutputFormat::Webp => "WEBP",
            OutputFormat::Gif => "GIF",
            OutputFormat::Bmp => "BMP",
            OutputFormat::Tiff => "TIFF",
            OutputFormat::Svg => "SVG",
        }
    }

    pub fn extension(self) -> Option<&'static str> {
        match self {
            OutputFormat::KeepOriginal => None,
            OutputFormat::Jpg => Some("jpg"),
            OutputFormat::Png => Some("png"),
            OutputFormat::Webp => Some("webp"),
            OutputFormat::Gif => Some("gif"),
            OutputFormat::Bmp => Some("bmp"),
            OutputFormat::Tiff => Some("tiff"),
            OutputFormat::Svg => Some("svg"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Ready,
    Done,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct PhotoItem {
    pub source: PathBuf,
    pub original_bytes: u64,
    pub new_bytes: Option<u64>,
    pub output_path: Option<PathBuf>,
    pub status: ItemStatus,
}

#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub quality: u8,
    pub scale_percent: u8,
    pub target_format: OutputFormat,
    pub overwrite_originals: bool,
    pub output_dir: Option<PathBuf>,
}

// Check if an extension belongs to a supported photo type
pub fn is_supported_photo(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let lower = ext.to_ascii_lowercase();
    INPUT_PHOTO_EXTENSIONS.contains(&lower.as_str())
}

// Scan a directory for supported photos
pub fn scan_folder(root: &Path, recursive: bool) -> Vec<PhotoItem> {
    if !root.is_dir() {
        return Vec::new();
    }

    let walker = if recursive {
        WalkDir::new(root)
    } else {
        WalkDir::new(root).max_depth(1)
    };

    let mut items = Vec::new();
    for entry in walker.into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let path = entry.into_path();
        if !is_supported_photo(&path) {
            continue;
        }
        // Skip files already inside demultimedia_out when scanning parent
        if path
            .components()
            .any(|c| c.as_os_str() == "demultimedia_out")
        {
            continue;
        }
        items.push(PhotoItem {
            source: path,
            original_bytes: size,
            new_bytes: None,
            output_path: None,
            status: ItemStatus::Ready,
        });
    }

    items.sort_by(|a, b| a.source.cmp(&b.source));
    items
}

// Build a PhotoItem list from individually picked or dropped files
pub fn items_from_paths(paths: &[PathBuf], recursive: bool) -> Vec<PhotoItem> {
    let mut items = Vec::new();
    for path in paths {
        if path.is_dir() {
            items.extend(scan_folder(path, recursive));
        } else if path.is_file() && is_supported_photo(path) {
            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            items.push(PhotoItem {
                source: path.clone(),
                original_bytes: size,
                new_bytes: None,
                output_path: None,
                status: ItemStatus::Ready,
            });
        }
    }
    items.sort_by(|a, b| a.source.cmp(&b.source));
    items.dedup_by(|a, b| a.source == b.source);
    items
}

// Resolve output format when KeepOriginal is selected
fn resolve_format(source: &Path, target: OutputFormat) -> OutputFormat {
    if target != OutputFormat::KeepOriginal {
        return target;
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" => OutputFormat::Jpg,
        "png" => OutputFormat::Png,
        "webp" => OutputFormat::Webp,
        "gif" => OutputFormat::Gif,
        "bmp" => OutputFormat::Bmp,
        "tif" | "tiff" => OutputFormat::Tiff,
        _ => OutputFormat::Png,
    }
}

// Compute destination path for a processed photo
fn compute_output_path(
    source: &Path,
    resolved_format: OutputFormat,
    opts: &ProcessOptions,
) -> Result<PathBuf, String> {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let ext = resolved_format.extension().unwrap_or("png");
    let file_name = format!("{stem}.{ext}");

    if opts.overwrite_originals {
        let parent = source
            .parent()
            .ok_or_else(|| "Missing parent folder".to_string())?;
        return Ok(parent.join(file_name));
    }

    if let Some(dir) = &opts.output_dir {
        return Ok(dir.join(file_name));
    }

    let parent = source
        .parent()
        .ok_or_else(|| "Missing parent folder".to_string())?;
    Ok(parent.join("demultimedia_out").join(file_name))
}

// Encode image bytes for the chosen format and quality
fn encode_image(img: &DynamicImage, format: OutputFormat, quality: u8) -> Result<Vec<u8>, String> {
    let q = quality.clamp(1, 100);
    let mut buf = Vec::new();

    match format {
        OutputFormat::Jpg | OutputFormat::KeepOriginal => {
            let rgb = img.to_rgb8();
            let mut enc = JpegEncoder::new_with_quality(&mut buf, q);
            enc.encode_image(&rgb).map_err(|e| e.to_string())?;
        }
        OutputFormat::Png => {
            let rgba = img.to_rgba8();
            let enc = PngEncoder::new_with_quality(&mut buf, CompressionType::Best, PngFilter::Adaptive);
            enc.write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        }
        OutputFormat::Webp => {
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::WebP)
                .map_err(|e| e.to_string())?;
        }
        OutputFormat::Gif => {
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Gif)
                .map_err(|e| e.to_string())?;
        }
        OutputFormat::Bmp => {
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Bmp)
                .map_err(|e| e.to_string())?;
        }
        OutputFormat::Tiff => {
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Tiff)
                .map_err(|e| e.to_string())?;
        }
        OutputFormat::Svg => {
            let has_alpha = img.color().has_alpha();
            let (mime, inner) = if has_alpha && q >= 90 {
                ("image/png", encode_image(img, OutputFormat::Png, q)?)
            } else {
                ("image/jpeg", encode_image(img, OutputFormat::Jpg, q)?)
            };
            let b64 = encode_base64(&inner);
            let w = img.width();
            let h = img.height();
            let svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n  <image width=\"{w}\" height=\"{h}\" href=\"data:{mime};base64,{b64}\"/>\n</svg>\n"
            );
            buf = svg.into_bytes();
        }
    }

    Ok(buf)
}

// Process a single photo item and return updated metadata
pub fn process_one(item: &mut PhotoItem, opts: &ProcessOptions) {
    let resolved = resolve_format(&item.source, opts.target_format);
    let dest = match compute_output_path(&item.source, resolved, opts) {
        Ok(p) => p,
        Err(e) => {
            item.status = ItemStatus::Error(e);
            return;
        }
    };

    let mut img = match image::open(&item.source) {
        Ok(i) => i,
        Err(e) => {
            item.status = ItemStatus::Error(e.to_string());
            return;
        }
    };

    let scale = opts.scale_percent.clamp(10, 100);
    if scale < 100 {
        let new_w = ((img.width() as u64 * scale as u64) / 100).max(1) as u32;
        let new_h = ((img.height() as u64 * scale as u64) / 100).max(1) as u32;
        img = img.resize_exact(new_w, new_h, FilterType::Lanczos3);
    }

    let bytes = match encode_image(&img, resolved, opts.quality) {
        Ok(b) => b,
        Err(e) => {
            item.status = ItemStatus::Error(e);
            return;
        }
    };

    if let Some(parent) = dest.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        item.status = ItemStatus::Error(e.to_string());
        return;
    }

    if let Err(e) = fs::write(&dest, &bytes) {
        item.status = ItemStatus::Error(e.to_string());
        return;
    }

    // Remove old file only when overwriting and extension changed
    if opts.overwrite_originals && dest != item.source {
        let _ = fs::remove_file(&item.source);
    }

    item.new_bytes = Some(bytes.len() as u64);
    item.output_path = Some(dest);
    item.status = ItemStatus::Done;
}

// Minimal base64 encoder with no external crate needed
fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("demultimedia_ops_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    fn write_sample_png(path: &Path, w: u32, h: u32) {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(w, h, |x, y| {
            Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8])
        });
        img.save(path).expect("failed to save sample png");
    }

    #[test]
    fn compresses_and_converts_png_to_jpg() {
        let root = make_temp_dir("jpg");
        let src = root.join("sample.png");
        write_sample_png(&src, 120, 80);

        let mut items = scan_folder(&root, false);
        assert_eq!(items.len(), 1);

        let opts = ProcessOptions {
            quality: 45,
            scale_percent: 100,
            target_format: OutputFormat::Jpg,
            overwrite_originals: false,
            output_dir: None,
        };
        process_one(&mut items[0], &opts);

        assert_eq!(items[0].status, ItemStatus::Done);
        let out = items[0].output_path.as_ref().unwrap();
        assert!(out.ends_with("demultimedia_out/sample.jpg"));
        assert!(out.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn converts_and_downscales_to_svg() {
        let root = make_temp_dir("svg");
        let src = root.join("pic.png");
        write_sample_png(&src, 100, 60);

        let mut items = scan_folder(&root, false);
        let opts = ProcessOptions {
            quality: 60,
            scale_percent: 50,
            target_format: OutputFormat::Svg,
            overwrite_originals: false,
            output_dir: None,
        };
        process_one(&mut items[0], &opts);

        assert_eq!(items[0].status, ItemStatus::Done);
        let out = items[0].output_path.as_ref().unwrap();
        let content = fs::read_to_string(out).unwrap();
        assert!(content.contains("<svg"));
        assert!(content.contains("width=\"50\" height=\"30\""));
        assert!(content.contains("data:image/jpeg;base64,"));

        let _ = fs::remove_dir_all(&root);
    }
}
