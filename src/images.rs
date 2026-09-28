//! Picture validation and naming.
//!
//! Only PNG, JPEG, WebP, and GIF are accepted. The decision is based on the
//! file's actual bytes (magic numbers) and a real decode under size limits,
//! never on the file name or a browser-supplied content type. SVG and
//! anything else that can carry active content is rejected.

use std::io::Cursor;

use serde::Serialize;

use crate::error::{CairnError, Result};
use crate::fsutil::sha256_hex;

#[derive(Debug, Clone, Copy)]
pub struct ImageLimits {
    pub max_bytes: usize,
    pub max_dimension: u32,
    pub max_pixels: u64,
}

impl Default for ImageLimits {
    fn default() -> Self {
        ImageLimits {
            max_bytes: 10 * 1024 * 1024,
            max_dimension: 8000,
            max_pixels: 40_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageInfo {
    pub ext: &'static str,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

fn sniff(bytes: &[u8]) -> Option<(image::ImageFormat, &'static str, &'static str)> {
    use image::ImageFormat::*;
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some((Png, "png", "image/png"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some((Jpeg, "jpg", "image/jpeg"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some((Gif, "gif", "image/gif"))
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some((WebP, "webp", "image/webp"))
    } else {
        None
    }
}

/// The content type to serve a stored picture with, decided from its bytes.
pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    sniff(bytes).map(|(_, _, mime)| mime)
}

fn looks_like_markup(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    let trimmed = head.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with('<') || head.contains("<svg")
}

fn ext_family(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e)?;
    match ext {
        "png" => Some("png"),
        "jpg" | "jpeg" | "jfif" => Some("jpg"),
        "gif" => Some("gif"),
        "webp" => Some("webp"),
        _ => None,
    }
}

fn damaged() -> CairnError {
    CairnError::InvalidImage(
        "That picture file is damaged or incomplete and can't be opened.".into(),
    )
}

/// Validate picture bytes. `file_name`, if given, must not claim a different
/// supported type than the content actually is.
pub fn validate_image(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &ImageLimits,
) -> Result<ImageInfo> {
    if bytes.is_empty() {
        return Err(CairnError::InvalidImage(
            "That picture file is empty.".into(),
        ));
    }
    if bytes.len() > limits.max_bytes {
        return Err(CairnError::InvalidImage(format!(
            "That picture is too large ({:.1} MB). The limit is {:.0} MB. Try a smaller \
             screenshot or save it as JPEG.",
            bytes.len() as f64 / 1_048_576.0,
            limits.max_bytes as f64 / 1_048_576.0
        )));
    }
    let Some((format, ext, mime)) = sniff(bytes) else {
        if looks_like_markup(bytes) {
            return Err(CairnError::InvalidImage(
                "SVG and other drawing formats aren't supported. Please use a PNG, JPEG, WebP, \
                 or GIF picture."
                    .into(),
            ));
        }
        return Err(CairnError::InvalidImage(
            "That file isn't a supported picture. Please use a PNG, JPEG, WebP, or GIF picture."
                .into(),
        ));
    };
    if let Some(claimed) = file_name.and_then(ext_family)
        && claimed != ext
    {
        return Err(CairnError::InvalidImage(format!(
            "The file is named like a {} picture but actually contains a {} picture. Please \
             re-save it with the correct type.",
            claimed.to_uppercase(),
            ext.to_uppercase()
        )));
    }

    let mut img_limits = image::Limits::default();
    img_limits.max_image_width = Some(limits.max_dimension);
    img_limits.max_image_height = Some(limits.max_dimension);
    img_limits.max_alloc = Some(
        limits
            .max_pixels
            .saturating_mul(4)
            .saturating_add(64 * 1024 * 1024),
    );

    let too_big = || {
        CairnError::InvalidImage(format!(
            "That picture is too big. Pictures can be at most {} pixels wide or tall.",
            limits.max_dimension
        ))
    };
    let map_err = |e: image::ImageError| match e {
        image::ImageError::Limits(_) => too_big(),
        _ => damaged(),
    };

    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(img_limits.clone());
    let (width, height) = reader.into_dimensions().map_err(map_err)?;
    if width == 0 || height == 0 {
        return Err(CairnError::InvalidImage(
            "That picture has no visible area.".into(),
        ));
    }
    if width > limits.max_dimension
        || height > limits.max_dimension
        || u64::from(width) * u64::from(height) > limits.max_pixels
    {
        return Err(too_big());
    }

    // Full decode to catch truncated or corrupt data.
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(img_limits);
    reader.decode().map_err(map_err)?;

    Ok(ImageInfo {
        ext,
        mime,
        width,
        height,
    })
}

/// Collision-resistant, content-addressed file name:
/// `<slug of original name>-<first 8 hex of SHA-256>.<ext>`.
/// `hash_len` lets the caller lengthen the hash on the (unlikely) event of a
/// name collision with different content.
pub fn asset_file_name(original: Option<&str>, bytes: &[u8], ext: &str, hash_len: usize) -> String {
    let stem = original
        .and_then(|n| n.rsplit(['/', '\\']).next())
        .map(|n| n.rsplit_once('.').map(|(s, _)| s).unwrap_or(n))
        .unwrap_or("");
    let mut slug = String::new();
    let mut dash = false;
    for c in stem.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
        if slug.len() >= 40 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "picture" } else { slug };
    let hash = sha256_hex(bytes);
    format!("{slug}-{}.{ext}", &hash[..hash_len.clamp(8, 64)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naming_is_stable_and_slugged() {
        let a = asset_file_name(Some("My Screenshot (1).PNG"), b"abc", "png", 8);
        assert!(a.starts_with("my-screenshot-1-"));
        assert!(a.ends_with(".png"));
        assert_eq!(
            a,
            asset_file_name(Some("My Screenshot (1).PNG"), b"abc", "png", 8)
        );
        assert!(asset_file_name(None, b"abc", "gif", 8).starts_with("picture-"));
    }
}
