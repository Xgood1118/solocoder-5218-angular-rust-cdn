use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref UUID_REGEX: Regex = Regex::new(
        r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
    ).unwrap();
}

pub const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
pub const MAX_TAGS: usize = 20;

pub const ALLOWED_MIME_PREFIXES: &[&str] = &[
    "image/",
    "video/",
    "font/",
    "application/javascript",
    "text/css",
    "application/json",
    "text/javascript",
    "application/ecmascript",
    "text/plain",
    "application/pdf",
    "application/zip",
    "audio/",
];

pub fn is_valid_uuid(id: &str) -> bool {
    UUID_REGEX.is_match(id)
}

pub fn is_valid_filename(filename: &str) -> bool {
    if filename.contains("..") {
        return false;
    }
    if filename.contains('/') || filename.contains('\\') {
        return false;
    }
    if filename.is_empty() {
        return false;
    }
    true
}

pub fn is_valid_mime(mime: &str) -> bool {
    for prefix in ALLOWED_MIME_PREFIXES {
        if mime.starts_with(prefix) {
            return true;
        }
    }
    false
}

pub fn is_valid_tags(tags: &[String]) -> bool {
    if tags.len() > MAX_TAGS {
        return false;
    }
    for tag in tags {
        if tag.is_empty() || tag.len() > 50 {
            return false;
        }
    }
    true
}

pub fn is_valid_file_size(size: u64) -> bool {
    size <= MAX_FILE_SIZE
}

pub fn sanitize_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut result = Vec::new();
    for part in parts {
        if part == ".." {
            continue;
        }
        if part.is_empty() {
            continue;
        }
        result.push(part);
    }
    result.join("/")
}
