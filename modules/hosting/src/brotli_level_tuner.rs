#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TunerError {
    #[error("Content too short: length {length} is below minimum threshold of {threshold}")]
    ContentTooShort { length: usize, threshold: usize },
    #[error("MIME type {0} is explicitly excluded from compression")]
    ExcludedMimeType(String),
    #[error("Invalid compression level: {0}")]
    InvalidCompressionLevel(u8),
}

pub struct BrotliCompressionTuner {
    min_length: usize,
    level_dynamic: u8,
    level_static: u8,

    excluded_mimes: Vec<&'static str>,
}

impl Default for BrotliCompressionTuner {
    fn default() -> Self {
        Self {
            min_length: 20,
            level_dynamic: 4,
            level_static: 6,

            excluded_mimes: vec![
                "application/zip",
                "image/jpeg",
                "image/png",
                "image/gif",
                "image/webp",
                "video/mp4",
                "video/webm",
                "audio/mpeg",
                "audio/ogg",
                "application/x-rar-compressed",
                "application/x-7z-compressed",
                "application/x-bzip2",
                "application/gzip",
            ],
        }
    }
}

pub enum ContentTypeCategory {
    Dynamic,
    Static,
    Precompressed,
}

impl BrotliCompressionTuner {
    pub fn new(min_length: usize) -> Self {
        let mut tuner = Self::default();
        tuner.min_length = min_length;
        tuner
    }

    fn categorize_mime(&self, mime_type: &str) -> ContentTypeCategory {
        let mime_lower = mime_type.to_lowercase();
        let base_mime = mime_lower.split(';').next().unwrap_or("").trim();

        if self.excluded_mimes.contains(&base_mime) {
            return ContentTypeCategory::Precompressed;
        }

        match base_mime {
            "text/html" | "application/json" | "application/xml" | "text/xml" | "text/event-stream" => ContentTypeCategory::Dynamic,
            "text/css" | "application/javascript" | "text/javascript" | "image/svg+xml" | "text/plain" => ContentTypeCategory::Static,
            _ => ContentTypeCategory::Dynamic, // Default to dynamic for unknown compressible
        }
    }

    pub fn recommend_level(&self, mime_type: &str, content_length: usize) -> Result<u8, TunerError> {
        if content_length < self.min_length {
            return Err(TunerError::ContentTooShort {
                length: content_length,
                threshold: self.min_length,
            });
        }

        let mime_lower = mime_type.to_lowercase();
        let base_mime = mime_lower.split(';').next().unwrap_or("").trim();

        if self.excluded_mimes.contains(&base_mime) {
            return Err(TunerError::ExcludedMimeType(base_mime.to_string()));
        }

        let category = self.categorize_mime(mime_type);
        match category {
            ContentTypeCategory::Dynamic => Ok(self.level_dynamic),
            ContentTypeCategory::Static => Ok(self.level_static),
            ContentTypeCategory::Precompressed => Err(TunerError::ExcludedMimeType(base_mime.to_string())),
        }
    }

    pub fn generate_nginx_config(&self, comp_level: u8) -> Result<String, TunerError> {
        if comp_level < 1 || comp_level > 11 {
            return Err(TunerError::InvalidCompressionLevel(comp_level));
        }

        let mut config = String::new();
        config.push_str("brotli on;\n");
        config.push_str(&format!("brotli_comp_level {};\n", comp_level));
        config.push_str(&format!("brotli_min_length {};\n", self.min_length));

        // Output all non-excluded types that are typically compressed
        let mut brotli_types = vec![
            "text/plain",
            "text/css",
            "application/json",
            "application/javascript",
            "text/xml",
            "application/xml",
            "application/xml+rss",
            "text/javascript",
            "image/svg+xml",
        ];

        brotli_types.retain(|&m| !self.excluded_mimes.contains(&m));

        config.push_str(&format!("brotli_types {};\n", brotli_types.join(" ")));

        Ok(config)
    }

    pub fn generate_apache_config(&self, comp_level: u8) -> Result<String, TunerError> {
        if comp_level < 1 || comp_level > 11 {
            return Err(TunerError::InvalidCompressionLevel(comp_level));
        }

        let mut config = String::new();
        config.push_str("<IfModule mod_brotli.c>\n");
        config.push_str("    AddOutputFilterByType BROTLI_COMPRESS text/html text/plain text/xml text/css text/javascript application/javascript application/json image/svg+xml\n");
        config.push_str(&format!("    BrotliCompressionQuality {}\n", comp_level));
        config.push_str("</IfModule>\n");

        Ok(config)
    }
}
