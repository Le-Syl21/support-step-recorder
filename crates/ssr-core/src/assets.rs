//! Content-addressed image store of an editing workspace.
//!
//! Every screenshot the editor knows about lives under
//! `<workspace>/assets/<sha256>.<ext>`. The name is derived from the bytes, so
//! two recordings that both contain a `step-0001.webp` never overwrite each
//! other, while two identical images are stored once and shared.
//!
//! Images coming from outside (a ZIP, a file picked by the user) are checked
//! before being stored: known format by its magic bytes, bounded dimensions,
//! and a full decode under an allocation limit.

use std::fmt;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Sub-folder of a workspace (and member prefix in a `.ssrproj`) for images.
pub const ASSET_DIR: &str = "assets";

/// The image formats accepted in a guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Webp,
}

impl ImageKind {
    /// Canonical file extension.
    pub fn ext(self) -> &'static str {
        match self {
            ImageKind::Png => "png",
            ImageKind::Jpeg => "jpg",
            ImageKind::Webp => "webp",
        }
    }

    fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "png" => Some(ImageKind::Png),
            "jpg" => Some(ImageKind::Jpeg),
            "webp" => Some(ImageKind::Webp),
            _ => None,
        }
    }

    fn format(self) -> image::ImageFormat {
        match self {
            ImageKind::Png => image::ImageFormat::Png,
            ImageKind::Jpeg => image::ImageFormat::Jpeg,
            ImageKind::Webp => image::ImageFormat::WebP,
        }
    }

    /// The format announced by the first bytes (never by a file name).
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(ImageKind::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(ImageKind::Jpeg)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(ImageKind::Webp)
        } else {
            None
        }
    }
}

/// Why an image was refused.
#[derive(Debug)]
pub enum ImageError {
    /// Not PNG, JPEG or WebP.
    UnsupportedFormat,
    /// Larger than the pixel budget (decompression bomb guard).
    TooManyPixels { width: u32, height: u32 },
    /// The bytes do not decode.
    Corrupt(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::UnsupportedFormat => write!(f, "not a PNG, JPEG or WebP image"),
            ImageError::TooManyPixels { width, height } => {
                write!(f, "image too large ({width}×{height} pixels)")
            }
            ImageError::Corrupt(e) => write!(f, "image cannot be decoded ({e})"),
        }
    }
}

impl std::error::Error for ImageError {}

/// Check that `bytes` is a sane PNG/JPEG/WebP of at most `max_pixels` pixels.
///
/// The dimensions are read from the header first, so a decompression bomb is
/// refused before anything is allocated; the full decode then runs under an
/// allocation limit and proves the file is not truncated or malformed.
pub fn validate_image(bytes: &[u8], max_pixels: u64) -> Result<(ImageKind, u32, u32), ImageError> {
    let kind = ImageKind::sniff(bytes).ok_or(ImageError::UnsupportedFormat)?;
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), kind.format())
        .into_dimensions()
        .map_err(|e| ImageError::Corrupt(e.to_string()))?;
    if width == 0 || height == 0 {
        return Err(ImageError::Corrupt("empty image".into()));
    }
    if u64::from(width) * u64::from(height) > max_pixels {
        return Err(ImageError::TooManyPixels { width, height });
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    // RGBA16 of the announced size, plus slack for the decoder's buffers.
    limits.max_alloc = Some(u64::from(width) * u64::from(height) * 8 + 64 * 1024 * 1024);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), kind.format());
    reader.limits(limits);
    reader
        .decode()
        .map_err(|e| ImageError::Corrupt(e.to_string()))?;
    Ok((kind, width, height))
}

/// SHA-256 of `bytes`, in lower-case hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// SHA-256 of a whole file, streamed.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = io::Read::read(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Whether `s` is a well-formed asset reference: `assets/<64 hex>.<ext>`.
pub fn is_asset_ref(s: &str) -> bool {
    parse_asset_ref(s).is_some()
}

/// The digest and format of an asset reference.
pub fn parse_asset_ref(s: &str) -> Option<(&str, ImageKind)> {
    let rest = s.strip_prefix("assets/")?;
    let (digest, ext) = rest.split_once('.')?;
    let ok = digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !ok {
        return None;
    }
    Some((digest, ImageKind::from_ext(ext)?))
}

/// The images of one workspace, addressed by content.
#[derive(Clone, Debug)]
pub struct AssetStore {
    root: PathBuf,
}

impl AssetStore {
    /// A store rooted at `root` (its `assets/` folder is created, private).
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        crate::workspace::create_private_dir(&root.join(ASSET_DIR))?;
        Ok(Self { root })
    }

    /// The folder asset references are relative to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file behind a (well-formed) reference.
    pub fn path_of(&self, asset_ref: &str) -> Option<PathBuf> {
        is_asset_ref(asset_ref).then(|| self.root.join(asset_ref))
    }

    pub fn contains(&self, asset_ref: &str) -> bool {
        self.path_of(asset_ref).is_some_and(|p| p.is_file())
    }

    pub fn read(&self, asset_ref: &str) -> io::Result<Vec<u8>> {
        let path = self.path_of(asset_ref).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid asset reference")
        })?;
        fs::read(path)
    }

    /// Store already validated image bytes. Returns the reference and whether
    /// the file was newly created (so a failed import can roll it back).
    pub fn put(&self, bytes: &[u8], kind: ImageKind) -> io::Result<(String, bool)> {
        let asset_ref = format!("{ASSET_DIR}/{}.{}", sha256_hex(bytes), kind.ext());
        let path = self.root.join(&asset_ref);
        if path.is_file() {
            return Ok((asset_ref, false));
        }
        let tmp = self
            .root
            .join(ASSET_DIR)
            .join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
        let result = (|| {
            let mut f = private_file(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            fs::rename(&tmp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map(|()| (asset_ref, true))
    }

    /// Validate then store foreign bytes.
    pub fn put_checked(&self, bytes: &[u8], max_pixels: u64) -> Result<(String, bool), PutError> {
        let (kind, _, _) = validate_image(bytes, max_pixels).map_err(PutError::Image)?;
        self.put(bytes, kind).map_err(PutError::Io)
    }

    /// Delete an asset (rollback of a failed import).
    pub fn remove(&self, asset_ref: &str) {
        if let Some(p) = self.path_of(asset_ref) {
            let _ = fs::remove_file(p);
        }
    }
}

/// Failure of [`AssetStore::put_checked`].
#[derive(Debug)]
pub enum PutError {
    Image(ImageError),
    Io(io::Error),
}

impl fmt::Display for PutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PutError::Image(e) => e.fmt(f),
            PutError::Io(e) => e.fmt(f),
        }
    }
}

/// A new file readable by its owner only (0600 on Unix).
pub(crate) fn private_file(path: &Path) -> io::Result<fs::File> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, shade: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([shade, 0, 0, 255]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn sniffs_formats_by_content() {
        assert_eq!(ImageKind::sniff(&png(2, 2, 1)), Some(ImageKind::Png));
        assert_eq!(
            ImageKind::sniff(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(ImageKind::Webp)
        );
        assert_eq!(ImageKind::sniff(b"<svg>"), None);
    }

    #[test]
    fn asset_refs_are_strict() {
        let good = format!("assets/{}.webp", "a".repeat(64));
        assert!(is_asset_ref(&good));
        assert!(!is_asset_ref("assets/../x.webp"));
        assert!(!is_asset_ref(&format!("assets/{}.svg", "a".repeat(64))));
        assert!(!is_asset_ref(&format!("assets/{}.webp", "A".repeat(64))));
        assert!(!is_asset_ref("step-0001.webp"));
    }

    #[test]
    fn rejects_bad_images() {
        assert!(matches!(
            validate_image(b"not an image", 1_000),
            Err(ImageError::UnsupportedFormat)
        ));
        let mut truncated = png(64, 64, 9);
        truncated.truncate(truncated.len() / 2);
        assert!(validate_image(&truncated, 1_000_000).is_err());
        assert!(matches!(
            validate_image(&png(100, 100, 1), 1_000),
            Err(ImageError::TooManyPixels { .. })
        ));
        assert!(validate_image(&png(10, 10, 1), 1_000).is_ok());
    }

    #[test]
    fn identical_bytes_share_one_asset() {
        let dir = std::env::temp_dir().join(format!("ssr-assets-{}", uuid::Uuid::new_v4()));
        let store = AssetStore::new(&dir).unwrap();
        let a = png(4, 4, 1);
        let b = png(4, 4, 2);
        let (ra, new_a) = store.put_checked(&a, 1_000).unwrap();
        let (ra2, new_a2) = store.put_checked(&a, 1_000).unwrap();
        let (rb, _) = store.put_checked(&b, 1_000).unwrap();
        assert!(new_a && !new_a2);
        assert_eq!(ra, ra2);
        assert_ne!(ra, rb);
        assert_eq!(store.read(&ra).unwrap(), a);
        let _ = fs::remove_dir_all(&dir);
    }
}
