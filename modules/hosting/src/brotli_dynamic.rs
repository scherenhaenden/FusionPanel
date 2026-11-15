#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrotliError {
    InvalidCompressionLevel(u8),
    InvalidMinLength(u32),
    PrecompressedTypeRequested(String),
}

impl std::fmt::Display for BrotliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCompressionLevel(level) => write!(
                f,
                "Compression level must be between 1 and 11, got {}",
                level
            ),
            Self::InvalidMinLength(len) => write!(
                f,
                "Minimum length must be greater than or equal to 20, got {}",
                len
            ),
            Self::PrecompressedTypeRequested(mime) => write!(
                f,
                "MIME type '{}' is already compressed and should not be dynamically compressed",
                mime
            ),
        }
    }
}

impl std::error::Error for BrotliError {}

#[derive(Debug, Clone)]
pub struct BrotliConfig {
    pub comp_level: u8,
    pub min_length: u32,
    pub window_size: Option<u32>, // in bytes, typically 1k - 16m
    pub types: Vec<String>,
}

impl Default for BrotliConfig {
    fn default() -> Self {
        Self {
            comp_level: 6,
            min_length: 20,
            window_size: None,
            types: vec![
                "text/plain".to_string(),
                "text/css".to_string(),
                "application/json".to_string(),
                "application/javascript".to_string(),
                "text/xml".to_string(),
                "application/xml".to_string(),
                "application/xml+rss".to_string(),
                "text/javascript".to_string(),
                "image/svg+xml".to_string(),
            ],
        }
    }
}

impl BrotliConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_comp_level(mut self, level: u8) -> Self {
        self.comp_level = level;
        self
    }

    pub fn with_min_length(mut self, length: u32) -> Self {
        self.min_length = length;
        self
    }

    pub fn with_window_size(mut self, window: u32) -> Self {
        self.window_size = Some(window);
        self
    }

    pub fn with_types(mut self, types: Vec<String>) -> Self {
        self.types = types;
        self
    }

    fn is_precompressed(mime: &str) -> bool {
        let mime = mime.trim().to_lowercase();
        // Common pre-compressed formats
        mime.starts_with("image/") && mime != "image/svg+xml" && mime != "image/x-icon"
            || mime.starts_with("video/")
            || mime.starts_with("audio/")
            || mime == "application/zip"
            || mime == "application/gzip"
            || mime == "application/x-gzip"
            || mime == "application/brotli"
            || mime == "application/x-brotli"
            || mime == "application/pdf"
            || mime == "application/x-7z-compressed"
            || mime == "application/x-rar-compressed"
            || mime == "application/x-tar"
            || mime == "application/x-xz"
    }

    pub fn validate(&self) -> Result<(), BrotliError> {
        if self.comp_level < 1 || self.comp_level > 11 {
            return Err(BrotliError::InvalidCompressionLevel(self.comp_level));
        }

        if self.min_length < 20 {
            return Err(BrotliError::InvalidMinLength(self.min_length));
        }

        for mime in &self.types {
            if Self::is_precompressed(mime) {
                return Err(BrotliError::PrecompressedTypeRequested(mime.clone()));
            }
        }

        Ok(())
    }

    pub fn generate_nginx_config(&self) -> Result<String, BrotliError> {
        self.validate()?;

        let mut config = String::new();
        config.push_str("brotli on;\n");
        config.push_str(&format!("brotli_comp_level {};\n", self.comp_level));
        config.push_str(&format!("brotli_min_length {};\n", self.min_length));

        if let Some(window) = self.window_size {
            // Usually represented with a size suffix, but we can just output the bytes
            // or format nicely if needed. Nginx brotli_window accepts size in bytes or with k/m.
            // Let's just output raw bytes for simplicity.
            config.push_str(&format!("brotli_window {};\n", window));
        }

        if !self.types.is_empty() {
            config.push_str(&format!("brotli_types {};\n", self.types.join(" ")));
        }

        Ok(config)
    }
}
