use mail::spf_flatten::{DnsResolver, SpfFlattener};

struct MockDnsResolver;

impl DnsResolver for MockDnsResolver {
    fn resolve_txt(&self, domain: &str) -> Result<Vec<String>, String> {
        match domain {
            "example.com" => Ok(vec!["v=spf1 include:example.net a mx -all".to_string()]),
            "example.net" => Ok(vec!["v=spf1 ip4:192.168.1.1 ip6:2001:db8::1 redirect=example.org".to_string()]),
            "example.org" => Ok(vec!["v=spf1 a:other.com/24/64 ~all".to_string()]),
            "loop1.com" => Ok(vec!["v=spf1 include:loop2.com -all".to_string()]),
            "loop2.com" => Ok(vec!["v=spf1 include:loop1.com ip4:10.0.0.1 -all".to_string()]),
            "toomany.com" => Ok(vec!["v=spf1 include:toomany1.com -all".to_string()]),
            "toomany1.com" => Ok(vec!["v=spf1 include:toomany2.com -all".to_string()]),
            "toomany2.com" => Ok(vec!["v=spf1 include:toomany3.com -all".to_string()]),
            "toomany3.com" => Ok(vec!["v=spf1 include:toomany4.com -all".to_string()]),
            "toomany4.com" => Ok(vec!["v=spf1 include:toomany5.com -all".to_string()]),
            "toomany5.com" => Ok(vec!["v=spf1 include:toomany6.com -all".to_string()]),
            "toomany6.com" => Ok(vec!["v=spf1 include:toomany7.com -all".to_string()]),
            "toomany7.com" => Ok(vec!["v=spf1 include:toomany8.com -all".to_string()]),
            "toomany8.com" => Ok(vec!["v=spf1 include:toomany9.com -all".to_string()]),
            "toomany9.com" => Ok(vec!["v=spf1 include:toomany10.com -all".to_string()]),
            "toomany10.com" => Ok(vec!["v=spf1 include:toomany11.com -all".to_string()]),
            "toomany11.com" => Ok(vec!["v=spf1 ip4:10.0.0.1 -all".to_string()]),
            _ => Ok(vec![]),
        }
    }

    fn resolve_a(&self, domain: &str) -> Result<Vec<String>, String> {
        match domain {
            "example.com" => Ok(vec!["1.1.1.1".to_string()]),
            "other.com" => Ok(vec!["2.2.2.2".to_string()]),
            "mx.example.com" => Ok(vec!["3.3.3.3".to_string(), "3.3.3.4".to_string()]),
            _ => Ok(vec![]),
        }
    }

    fn resolve_aaaa(&self, domain: &str) -> Result<Vec<String>, String> {
        match domain {
            "example.com" => Ok(vec!["2001:1111::1".to_string()]),
            "other.com" => Ok(vec!["2001:2222::2".to_string()]),
            "mx.example.com" => Ok(vec!["2001:3333::3".to_string(), "2001:3333::4".to_string()]),
            _ => Ok(vec![]),
        }
    }

    fn resolve_mx(&self, domain: &str) -> Result<Vec<String>, String> {
        match domain {
            "example.com" => Ok(vec!["mx.example.com".to_string()]),
            _ => Ok(vec![]),
        }
    }
}

#[test]
fn test_flatten_success() {
    let resolver = MockDnsResolver;
    let flattener = SpfFlattener::new(&resolver);
    let result = flattener.flatten("example.com").unwrap();

    assert!(result.starts_with("v=spf1"));
    assert!(result.contains("ip4:192.168.1.1"));
    assert!(result.contains("ip6:2001:db8::1"));
    assert!(result.contains("ip4:1.1.1.1"));
    assert!(result.contains("ip6:2001:1111::1"));
    assert!(result.contains("ip4:2.2.2.2/24"));
    assert!(result.contains("ip6:2001:2222::2/64"));
    assert!(result.contains("ip4:3.3.3.3"));
    assert!(result.contains("ip6:2001:3333::3"));
    assert!(result.contains("ip4:3.3.3.4"));
    assert!(result.contains("ip6:2001:3333::4"));
    assert!(result.ends_with("-all"));
}

#[test]
fn test_flatten_loop() {
    let resolver = MockDnsResolver;
    let flattener = SpfFlattener::new(&resolver);
    let result = flattener.flatten("loop1.com").unwrap();

    assert!(result.contains("ip4:10.0.0.1"));
}

#[test]
fn test_flatten_too_many_lookups() {
    let resolver = MockDnsResolver;
    let flattener = SpfFlattener::new(&resolver);
    let result = flattener.flatten("toomany.com");

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "DNS lookup limit exceeded (RFC 7208)");
}
