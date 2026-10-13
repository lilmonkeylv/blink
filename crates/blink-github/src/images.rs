use crate::error::{Error, Result};
use crate::oauth::http_client;

pub const MAX_IMAGE_BYTES: usize = 1024 * 1024;
const IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];

pub fn image_type(content_type: &str) -> Option<&'static str> {
    let essence = content_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    IMAGE_TYPES.iter().copied().find(|t| *t == essence)
}

pub async fn fetch_image(url: &str) -> Result<(&'static str, Vec<u8>)> {
    let mut response = http_client().get(url).send().await?;
    if !response.status().is_success() {
        return Err(Error::Api { status: response.status().as_u16(), message: "image request failed".into() });
    }
    let kind = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(image_type)
        .ok_or_else(|| Error::Api { status: 415, message: "not an image".into() })?;
    if response.content_length().is_some_and(|len| len as usize > MAX_IMAGE_BYTES) {
        return Err(Error::Api { status: 413, message: "image too large".into() });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_IMAGE_BYTES {
            return Err(Error::Api { status: 413, message: "image too large".into() });
        }
    }
    Ok((kind, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_raster_images() {
        assert_eq!(image_type("image/png"), Some("image/png"));
        assert_eq!(image_type("IMAGE/JPEG; charset=binary"), Some("image/jpeg"));
        assert_eq!(image_type("image/svg+xml"), None);
        assert_eq!(image_type("text/html"), None);
    }
}
