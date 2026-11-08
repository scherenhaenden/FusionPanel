#![forbid(unsafe_code)]

#[path = "../src/autodiscover.rs"]
mod autodiscover;

use autodiscover::AutodiscoverGenerator;

#[test]
fn test_generate_autoconfig() {
    let domain = "example.com";
    let display_name = "Example User";
    let imap_hostname = "imap.example.com";
    let smtp_hostname = "smtp.example.com";
    let username = "user@example.com";

    let xml = AutodiscoverGenerator::generate_autoconfig(
        domain,
        display_name,
        imap_hostname,
        smtp_hostname,
        username,
    );

    // Verify basic structure
    assert!(xml.contains(r#"<clientConfig version="1.1">"#));
    assert!(xml.contains(r#"<emailProvider id="example.com">"#));
    assert!(xml.contains(r#"<domain>example.com</domain>"#));
    assert!(xml.contains(r#"<displayName>Example User</displayName>"#));

    // Verify IMAP
    assert!(xml.contains(r#"<incomingServer type="imap">"#));
    assert!(xml.contains(r#"<hostname>imap.example.com</hostname>"#));
    assert!(xml.contains(r#"<port>993</port>"#));
    assert!(xml.contains(r#"<socketType>SSL</socketType>"#));

    // Verify SMTP 465
    assert!(xml.contains(r#"<outgoingServer type="smtp">"#));
    assert!(xml.contains(r#"<hostname>smtp.example.com</hostname>"#));
    assert!(xml.contains(r#"<port>465</port>"#));
    assert!(xml.contains(r#"<socketType>SSL</socketType>"#));

    // Verify SMTP 587
    assert!(xml.contains(r#"<port>587</port>"#));
    assert!(xml.contains(r#"<socketType>STARTTLS</socketType>"#));
}

#[test]
fn test_generate_autodiscover() {
    let imap_hostname = "imap.example.com";
    let smtp_hostname = "smtp.example.com";
    let username = "user@example.com";

    let xml = AutodiscoverGenerator::generate_autodiscover(
        imap_hostname,
        smtp_hostname,
        username,
    );

    // Verify basic structure
    assert!(xml.contains(r#"<Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/responseschema/2006">"#));
    assert!(xml.contains(r#"<Response xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a">"#));
    assert!(xml.contains(r#"<AccountType>email</AccountType>"#));

    // Verify IMAP
    assert!(xml.contains(r#"<Type>IMAP</Type>"#));
    assert!(xml.contains(r#"<Server>imap.example.com</Server>"#));
    assert!(xml.contains(r#"<Port>993</Port>"#));
    assert!(xml.contains(r#"<SSL>on</SSL>"#));
    assert!(xml.contains(r#"<AuthRequired>on</AuthRequired>"#));

    // Verify SMTP
    assert!(xml.contains(r#"<Type>SMTP</Type>"#));
    assert!(xml.contains(r#"<Server>smtp.example.com</Server>"#));
    assert!(xml.contains(r#"<Port>465</Port>"#));
    assert!(xml.contains(r#"<SSL>on</SSL>"#));
    assert!(xml.contains(r#"<AuthRequired>on</AuthRequired>"#));
}

#[test]
fn test_xml_injection_sanitization() {
    let malicious_domain = "example.com<script>alert(1)</script>";
    let malicious_display_name = "\"John & Doe's\" <email>";
    let imap_hostname = "imap<.example.com";
    let smtp_hostname = "smtp>.example.com";
    let username = "<user>";

    let xml = AutodiscoverGenerator::generate_autoconfig(
        malicious_domain,
        malicious_display_name,
        imap_hostname,
        smtp_hostname,
        username,
    );

    // Confirm characters are escaped
    assert!(xml.contains("example.com&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(xml.contains("&quot;John &amp; Doe&apos;s&quot; &lt;email&gt;"));
    assert!(xml.contains("imap&lt;.example.com"));
    assert!(xml.contains("smtp&gt;.example.com"));
    assert!(xml.contains("&lt;user&gt;"));

    // Ensure raw unescaped strings are NOT present
    assert!(!xml.contains(malicious_domain));
    assert!(!xml.contains(malicious_display_name));
    assert!(!xml.contains(imap_hostname));
    assert!(!xml.contains(smtp_hostname));
    assert!(!xml.contains(username));

    // Check Autodiscover XML injection handling
    let ad_xml = AutodiscoverGenerator::generate_autodiscover(
        imap_hostname,
        smtp_hostname,
        username,
    );

    assert!(ad_xml.contains("imap&lt;.example.com"));
    assert!(ad_xml.contains("smtp&gt;.example.com"));
    assert!(ad_xml.contains("&lt;user&gt;"));
}
