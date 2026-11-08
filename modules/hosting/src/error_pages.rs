#![forbid(unsafe_code)]

/// Generator for Nginx custom error pages configuration and default HTML templates.
pub struct ErrorPageGenerator;

impl ErrorPageGenerator {
    /// Generates Nginx directives for mapping standard error codes to custom pages.
    ///
    /// Configures 404, 500, 502, 503, and 504 errors to route to `/_errors/<code_or_custom>.html`.
    /// Also includes the `location ^~ /_errors/` block with `internal;` to prevent direct external access.
    pub fn generate_nginx_directives(error_docs_root: &str) -> String {
        // We must safely format the root path. Since this is an nginx config, we should
        // ideally escape things, but simple path validation is usually assumed.
        // We'll rely on the caller providing a safe path, but we'll quote it.
        // Or we can just use the provided string.
        let mut config = String::new();

        // Define error pages
        config.push_str("error_page 404 /_errors/404.html;\n");
        config.push_str("error_page 500 /_errors/500.html;\n");
        config.push_str("error_page 502 /_errors/502.html;\n");
        config.push_str("error_page 503 /_errors/503.html;\n");
        config.push_str("error_page 504 /_errors/504.html;\n");

        // Define internal location block
        config.push_str("\n");
        config.push_str("location ^~ /_errors/ {\n");
        config.push_str("    internal;\n");

        // We need to safely escape the path for nginx
        let safe_root = error_docs_root.replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$");

        // Ensure trailing slash for alias directive when used with location ending in slash
        let final_root = if safe_root.ends_with('/') {
            safe_root
        } else {
            format!("{}/", safe_root)
        };

        config.push_str(&format!("    alias \"{}\";\n", final_root));
        config.push_str("}\n");

        config
    }

    /// Generates a lightweight, responsive, self-contained HTML template for a given HTTP status code.
    pub fn generate_default_html(status_code: u16) -> String {
        let (title, message) = match status_code {
            404 => ("404 Not Found", "The requested resource could not be found on this server."),
            500 => ("500 Internal Server Error", "The server encountered an internal error and was unable to complete your request."),
            502 => ("502 Bad Gateway", "The server received an invalid response from the upstream server."),
            503 => ("503 Service Unavailable", "The server is temporarily unable to service your request due to maintenance downtime or capacity problems."),
            504 => ("504 Gateway Timeout", "The server did not receive a timely response from the upstream server."),
            _ => ("Error", "An unexpected error occurred."),
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{}</title>
    <style>
        body {{
            font-family: system-ui, -apple-system, sans-serif;
            background-color: #f3f4f6;
            color: #1f2937;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
        }}
        .container {{
            text-align: center;
            background: white;
            padding: 2rem 3rem;
            border-radius: 0.5rem;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
            max-width: 90%;
        }}
        h1 {{
            font-size: 3rem;
            font-weight: 700;
            margin: 0 0 1rem 0;
            color: #ef4444;
        }}
        p {{
            font-size: 1.125rem;
            margin: 0;
            color: #4b5563;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>{}</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
            title, status_code, message
        )
    }
}
