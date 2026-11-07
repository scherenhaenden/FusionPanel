#![forbid(unsafe_code)]

use crate::domain::error::Result;

pub struct CompressionSettings {
    pub gzip_enabled: bool,
    pub brotli_enabled: bool,
    pub static_asset_caching_enabled: bool,
    pub etag_enabled: bool,
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            gzip_enabled: true,
            brotli_enabled: true,
            static_asset_caching_enabled: true,
            etag_enabled: true,
        }
    }
}

pub fn generate_compression_config(settings: &CompressionSettings) -> Result<String> {
    let mut config = String::new();

    if settings.gzip_enabled {
        config.push_str(
            "    gzip on;\n\
                 gzip_comp_level 5;\n\
                 gzip_min_length 256;\n\
                 gzip_proxied any;\n\
                 gzip_vary on;\n\
                 gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript image/svg+xml;\n\n",
        );
    }

    if settings.brotli_enabled {
        config.push_str(
            "    brotli on;\n\
                 brotli_comp_level 6;\n\
                 brotli_static on;\n\
                 brotli_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript image/svg+xml;\n\n",
        );
    }

    if settings.static_asset_caching_enabled {
        config.push_str(
            "    location ~* \\.(?:css|js|jpg|jpeg|gif|png|ico|cur|gz|svg|svgz|mp4|ogg|ogv|webm|htc)$ {\n\
                     expires 1M;\n\
                     access_log off;\n\
                     add_header Cache-Control \"public\";\n\
                 }\n\n",
        );
    }

    if settings.etag_enabled {
        config.push_str("    etag on;\n\n");
    } else {
        config.push_str("    etag off;\n\n");
    }

    Ok(config)
}
