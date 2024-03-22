use std::borrow::Cow;

pub fn guess_mime_type(file_ext: &str) -> Option<Cow<'static, str>> {
    match file_ext {
        "mp4" => Some(Cow::Borrowed("video/mp4")),
        "avif" => Some(Cow::Borrowed("image/avif")),
        "webp" => Some(Cow::Borrowed("image/webp")),
        "jpg" | "jpeg" => Some(Cow::Borrowed("image/jpeg")),
        "png" => Some(Cow::Borrowed("image/png")),
        "heif" => Some(Cow::Borrowed("image/heif")),
        "heic" => Some(Cow::Borrowed("image/heic")),
        _ => None,
    }
}

pub fn guess_mime_type_path(path: &camino::Utf8Path) -> Option<Cow<'static, str>> {
    let ext = path.extension()?.to_ascii_lowercase();
    match guess_mime_type(&ext) {
        Some(m) => Some(m),
        None => {
            tracing::warn!(
                "can't guess MIME type for filename '{}'",
                &path
                    .file_name()
                    .map(|p| p.to_string())
                    .unwrap_or(String::new())
            );
            None
        }
    }
}
