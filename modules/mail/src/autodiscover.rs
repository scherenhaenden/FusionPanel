#![forbid(unsafe_code)]

pub struct AutodiscoverGenerator;

impl AutodiscoverGenerator {
    /// Escapes characters for safe inclusion in XML
    fn escape_xml(input: &str) -> String {
        let mut escaped = String::with_capacity(input.len());
        for c in input.chars() {
            match c {
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '&' => escaped.push_str("&amp;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&apos;"),
                _ => escaped.push(c),
            }
        }
        escaped
    }

    /// Generates Mozilla Thunderbird Autoconfig XML (config-v1.1.xml)
    pub fn generate_autoconfig(
        domain: &str,
        display_name: &str,
        imap_hostname: &str,
        smtp_hostname: &str,
        username: &str,
    ) -> String {
        let domain = Self::escape_xml(domain);
        let display_name = Self::escape_xml(display_name);
        let imap_hostname = Self::escape_xml(imap_hostname);
        let smtp_hostname = Self::escape_xml(smtp_hostname);
        let username = Self::escape_xml(username);

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="{domain}">
    <domain>{domain}</domain>
    <displayName>{display_name}</displayName>
    <displayShortName>{display_name}</displayShortName>
    <incomingServer type="imap">
      <hostname>{imap_hostname}</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>{username}</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>{smtp_hostname}</hostname>
      <port>465</port>
      <socketType>SSL</socketType>
      <authentication>password-cleartext</authentication>
      <username>{username}</username>
    </outgoingServer>
    <outgoingServer type="smtp">
      <hostname>{smtp_hostname}</hostname>
      <port>587</port>
      <socketType>STARTTLS</socketType>
      <authentication>password-cleartext</authentication>
      <username>{username}</username>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#,
            domain = domain,
            display_name = display_name,
            imap_hostname = imap_hostname,
            smtp_hostname = smtp_hostname,
            username = username
        )
    }

    /// Generates Microsoft Outlook Autodiscover XML response
    pub fn generate_autodiscover(
        imap_hostname: &str,
        smtp_hostname: &str,
        username: &str,
    ) -> String {
        let imap_hostname = Self::escape_xml(imap_hostname);
        let smtp_hostname = Self::escape_xml(smtp_hostname);
        let username = Self::escape_xml(username);

        format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<Autodiscover xmlns="http://schemas.microsoft.com/exchange/autodiscover/responseschema/2006">
  <Response xmlns="http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a">
    <Account>
      <AccountType>email</AccountType>
      <Action>settings</Action>
      <Protocol>
        <Type>IMAP</Type>
        <Server>{imap_hostname}</Server>
        <Port>993</Port>
        <DomainRequired>off</DomainRequired>
        <LoginName>{username}</LoginName>
        <SPA>off</SPA>
        <SSL>on</SSL>
        <AuthRequired>on</AuthRequired>
      </Protocol>
      <Protocol>
        <Type>SMTP</Type>
        <Server>{smtp_hostname}</Server>
        <Port>465</Port>
        <DomainRequired>off</DomainRequired>
        <LoginName>{username}</LoginName>
        <SPA>off</SPA>
        <SSL>on</SSL>
        <AuthRequired>on</AuthRequired>
      </Protocol>
    </Account>
  </Response>
</Autodiscover>"#,
            imap_hostname = imap_hostname,
            smtp_hostname = smtp_hostname,
            username = username
        )
    }
}
