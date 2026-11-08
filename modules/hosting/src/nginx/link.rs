#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRel {
    Preload,
    Preconnect,
}

impl LinkRel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkRel::Preload => "preload",
            LinkRel::Preconnect => "preconnect",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreloadAs {
    Style,
    Script,
    Image,
    Font,
    Fetch,
    Document,
}

impl PreloadAs {
    pub fn as_str(&self) -> &'static str {
        match self {
            PreloadAs::Style => "style",
            PreloadAs::Script => "script",
            PreloadAs::Image => "image",
            PreloadAs::Font => "font",
            PreloadAs::Fetch => "fetch",
            PreloadAs::Document => "document",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpLink {
    pub uri: String,
    pub rel: LinkRel,
    pub as_attr: Option<PreloadAs>,
    pub crossorigin: bool,
}

impl HttpLink {
    pub fn new(
        uri: &str,
        rel: LinkRel,
        as_attr: Option<PreloadAs>,
        crossorigin: bool,
    ) -> Result<Self, String> {
        validate_uri(uri)?;
        Ok(Self {
            uri: uri.to_string(),
            rel,
            as_attr,
            crossorigin,
        })
    }

    pub fn generate_nginx_directive(&self) -> String {
        let mut parts = format!("<{}>; rel={}", self.uri, self.rel.as_str());
        if let Some(ref a) = self.as_attr {
            parts.push_str(&format!("; as={}", a.as_str()));
        }
        if self.crossorigin {
            parts.push_str("; crossorigin");
        }
        format!("add_header Link \"{}\" always;", parts)
    }
}

fn validate_uri(uri: &str) -> Result<(), String> {
    if uri.is_empty() {
        return Err("URI cannot be empty".into());
    }

    // Must not be a protocol-relative target
    if uri.starts_with("//") {
        return Err("Protocol-relative targets are forbidden".into());
    }

    // Valid prefix: must be either an absolute local path (starts with / but not //)
    // or an absolute URL starting with http:// or https://
    if !uri.starts_with('/') && !uri.starts_with("http://") && !uri.starts_with("https://") {
        return Err("URI must be an absolute path (starting with /) or absolute URL".into());
    }

    let bytes = uri.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];

        // Percent encoding check
        if b == b'%' {
            if i + 2 >= bytes.len() {
                return Err("Invalid percent encoding length".into());
            }
            if !bytes[i + 1].is_ascii_hexdigit() || !bytes[i + 2].is_ascii_hexdigit() {
                return Err("Invalid percent encoding characters".into());
            }
            i += 3;
            continue;
        }

        // Allow: Alphanumeric and explicitly safe chars
        // Do NOT allow spaces, quotes (", '), semicolons (;), braces ({, }), directives/variables ($), CR/LF (\r, \n), or backslashes (\)
        let is_safe = b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'_' | b'.' | b'/' | b'?' | b'&' | b'=' | b':');

        if !is_safe {
            return Err(format!("Forbidden character in URI: '{}'", b as char));
        }

        i += 1;
    }

    Ok(())
}
