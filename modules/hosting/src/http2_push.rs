#![forbid(unsafe_code)]

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LinkElement {
    pub uri: String,
    pub rel: Option<String>,
    pub as_attr: Option<String>,
    pub crossorigin: Option<String>,
    pub nopush: bool,
}

impl LinkElement {
    pub fn to_link_header_value(&self) -> String {
        let mut s = format!("<{}>", self.uri);
        if let Some(rel) = &self.rel {
            s.push_str(&format!("; rel=\"{}\"", rel));
        }
        if let Some(as_attr) = &self.as_attr {
            s.push_str(&format!("; as=\"{}\"", as_attr));
        }
        if let Some(crossorigin) = &self.crossorigin {
            if crossorigin.is_empty() {
                s.push_str("; crossorigin");
            } else {
                s.push_str(&format!("; crossorigin=\"{}\"", crossorigin));
            }
        }
        if self.nopush {
            s.push_str("; nopush");
        }
        s
    }
}

pub struct LinkHeaderGenerator;

impl LinkHeaderGenerator {
    pub fn parse_html(html: &str) -> Vec<LinkElement> {
        let mut links = Vec::new();
        let mut cursor = 0;
        let bytes = html.as_bytes();

        while let Some(start) = html[cursor..].find('<') {
            cursor += start + 1;
            let tag_start = cursor;
            let mut tag_end = tag_start;
            while tag_end < bytes.len() && !bytes[tag_end].is_ascii_whitespace() && bytes[tag_end] != b'>' {
                tag_end += 1;
            }

            let tag_name = &html[tag_start..tag_end].to_ascii_lowercase();
            cursor = tag_end;

            if tag_name == "link" || tag_name == "script" || tag_name == "style" {
                if let Some(end_tag) = html[cursor..].find('>') {
                    let attrs_str = &html[cursor..cursor+end_tag];
                    cursor += end_tag + 1;

                    if let Some(link_element) = Self::parse_attributes(tag_name, attrs_str) {
                        links.push(link_element);
                    }
                }
            }
        }
        links
    }

    fn extract_attrs(mut s: &str) -> Vec<(String, String)> {
        let mut attrs = Vec::new();
        while !s.is_empty() {
            s = s.trim_start();
            if s.is_empty() { break; }

            let mut eq_idx = None;
            let mut space_idx = None;
            for (i, c) in s.char_indices() {
                if c == '=' {
                    eq_idx = Some(i);
                    break;
                } else if c.is_whitespace() {
                    space_idx = Some(i);
                    break;
                }
            }

            let key = if let Some(eq) = eq_idx {
                let k = &s[..eq];
                s = &s[eq+1..];
                k.trim_end().to_ascii_lowercase()
            } else if let Some(sp) = space_idx {
                let k = &s[..sp];
                s = &s[sp+1..];
                attrs.push((k.to_ascii_lowercase(), "".to_string()));
                continue;
            } else {
                attrs.push((s.to_ascii_lowercase(), "".to_string()));
                break;
            };

            s = s.trim_start();
            if s.starts_with('"') {
                if let Some(end) = s[1..].find('"') {
                    let val = &s[1..1+end];
                    s = &s[1+end+1..];
                    attrs.push((key, val.to_string()));
                } else {
                    break;
                }
            } else if s.starts_with('\'') {
                if let Some(end) = s[1..].find('\'') {
                    let val = &s[1..1+end];
                    s = &s[1+end+1..];
                    attrs.push((key, val.to_string()));
                } else {
                    break;
                }
            } else if !s.is_empty() {
                let end = s.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(s.len());
                let val = &s[..end];
                s = &s[end..];
                attrs.push((key, val.to_string()));
            }
        }
        attrs
    }

    fn parse_attributes(tag_name: &str, attrs_str: &str) -> Option<LinkElement> {
        let attrs = Self::extract_attrs(attrs_str);
        let get_attr = |name: &str| attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());

        let href = get_attr("href");
        let src = get_attr("src");
        let uri = href.or(src)?;

        let mut rel = get_attr("rel");
        let mut as_attr = get_attr("as");
        let crossorigin = get_attr("crossorigin");
        let nopush = get_attr("nopush").is_some();

        if tag_name == "script" {
            if rel.is_none() { rel = Some("preload".to_string()); }
            if as_attr.is_none() { as_attr = Some("script".to_string()); }
        } else if tag_name == "style" {
            if rel.is_none() { rel = Some("preload".to_string()); }
            if as_attr.is_none() { as_attr = Some("style".to_string()); }
        } else if tag_name == "link" {
            if let Some(r) = &rel {
                if r == "stylesheet" {
                    rel = Some("preload".to_string());
                    if as_attr.is_none() {
                        as_attr = Some("style".to_string());
                    }
                }
            }
        }

        if let Some(r) = &rel {
            if r != "preload" && r != "preconnect" && r != "modulepreload" {
                return None;
            }
        } else {
            return None;
        }

        Some(LinkElement {
            uri,
            rel,
            as_attr,
            crossorigin,
            nopush,
        })
    }

    pub fn generate_nginx_config(html: &str) -> Vec<String> {
        let elements = Self::parse_html(html);
        let mut directives = Vec::new();

        for el in &elements {
            if !el.nopush && el.uri.starts_with('/') && !el.uri.starts_with("//") {
                if let Some(rel) = &el.rel {
                    if rel == "preload" || rel == "modulepreload" {
                        directives.push(format!("http2_push {};", el.uri));
                    }
                }
            }
        }

        let link_values: Vec<String> = elements.iter().map(|e| e.to_link_header_value()).collect();
        for val in link_values {
            let escaped_val = val.replace("\"", "\\\"");
            directives.push(format!("add_header Link \"{}\";", escaped_val));
        }

        directives
    }
}
