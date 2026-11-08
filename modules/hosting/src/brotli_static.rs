#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum CompressionType {
    Brotli,
    Gzip,
    None,
}

pub struct BrotliStaticMatcher;

impl BrotliStaticMatcher {
    /// Parses the Accept-Encoding header and returns the preferred compression type.
    pub fn negotiate_encoding(accept_encoding: &str) -> CompressionType {
        let mut supports_brotli = false;
        let mut supports_gzip = false;

        for encoding in accept_encoding.split(',') {
            let encoding = encoding.trim();
            if encoding.starts_with("br") {
                supports_brotli = true;
            } else if encoding.starts_with("gzip") {
                supports_gzip = true;
            }
        }

        if supports_brotli {
            CompressionType::Brotli
        } else if supports_gzip {
            CompressionType::Gzip
        } else {
            CompressionType::None
        }
    }

    /// Sanitizes the requested file path to prevent path traversals and null bytes.
    pub fn sanitize_path(path: &str) -> Option<PathBuf> {
        if path.contains('\0') {
            return None;
        }

        let p = Path::new(path);

        // Prevent absolute paths from escaping document root
        if p.is_absolute() {
            return None;
        }

        let mut sanitized = PathBuf::new();
        for component in p.components() {
            match component {
                std::path::Component::Normal(c) => sanitized.push(c),
                std::path::Component::ParentDir => return None, // Strict rejection of ".."
                std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
                std::path::Component::CurDir => continue, // Ignore "."
            }
        }

        if sanitized.as_os_str().is_empty() {
            return None;
        }

        Some(sanitized)
    }

    /// Checks if a pre-compressed sidecar file exists for the given base path within the document root.
    /// Returns the matched compression type (Brotli or Gzip) and the sidecar path.
    pub fn find_sidecar(
        document_root: &Path,
        requested_path: &str,
        accept_encoding: &str,
    ) -> Option<(CompressionType, PathBuf)> {
        let preferred = Self::negotiate_encoding(accept_encoding);
        if preferred == CompressionType::None {
            return None;
        }

        let sanitized = Self::sanitize_path(requested_path)?;
        let base_path = document_root.join(sanitized);

        if preferred == CompressionType::Brotli {
            let br_path = Self::append_ext(&base_path, "br");
            if br_path.exists() {
                return Some((CompressionType::Brotli, br_path));
            }
        }

        if preferred == CompressionType::Brotli || preferred == CompressionType::Gzip {
            let gz_path = Self::append_ext(&base_path, "gz");
            if gz_path.exists() {
                return Some((CompressionType::Gzip, gz_path));
            }
        }

        None
    }

    pub fn append_ext(path: &Path, ext: &str) -> PathBuf {
        let mut new_path = path.to_path_buf().into_os_string();
        new_path.push(".");
        new_path.push(ext);
        PathBuf::from(new_path)
    }

    /// Generates Nginx directives based on the sidecar existence.
    pub fn generate_nginx_directives(compression_type: CompressionType) -> String {
        match compression_type {
            CompressionType::Brotli => "brotli_static on;\n".to_string(),
            CompressionType::Gzip => "gzip_static on;\n".to_string(),
            CompressionType::None => String::new(),
        }
    }
}
