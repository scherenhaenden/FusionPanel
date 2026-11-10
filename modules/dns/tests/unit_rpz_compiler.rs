#![forbid(unsafe_code)]

use dns::rpz_compiler::{RpzAction, RpzCompiler};

#[test]
fn test_rpz_action_to_rule_string() {
    assert_eq!(RpzAction::NxDomain.to_rule_string(), ".");
    assert_eq!(RpzAction::NoData.to_rule_string(), "*.");
    assert_eq!(RpzAction::Cname("sinkhole.example.com".to_string()).to_rule_string(), "sinkhole.example.com.");
    assert_eq!(RpzAction::Cname("sinkhole.example.com.".to_string()).to_rule_string(), "sinkhole.example.com.");
    assert_eq!(RpzAction::A("192.168.1.1".to_string()).to_rule_string(), "192.168.1.1");
    assert_eq!(RpzAction::Aaaa("::1".to_string()).to_rule_string(), "::1");
}

#[test]
fn test_compile_nxdomain() {
    let mut compiler = RpzCompiler::new("rpz.local");
    compiler.add_rule("bad.com", RpzAction::NxDomain);

    let compiled = compiler.compile();
    assert!(compiled.contains("bad.com IN CNAME ."));
    assert!(compiled.contains("*.bad.com IN CNAME ."));
}

#[test]
fn test_compile_nodata() {
    let mut compiler = RpzCompiler::new("rpz.local");
    compiler.add_rule("drop.com", RpzAction::NoData);

    let compiled = compiler.compile();
    assert!(compiled.contains("drop.com IN CNAME *."));
    assert!(compiled.contains("*.drop.com IN CNAME *."));
}

#[test]
fn test_compile_a_record() {
    let mut compiler = RpzCompiler::new("rpz.local");
    compiler.add_rule("malware.com", RpzAction::A("10.0.0.1".to_string()));

    let compiled = compiler.compile();
    assert!(compiled.contains("malware.com IN A 10.0.0.1"));
    assert!(compiled.contains("*.malware.com IN A 10.0.0.1"));
}

#[test]
fn test_compile_cname_record() {
    let mut compiler = RpzCompiler::new("rpz.local");
    compiler.add_rule("phishing.com", RpzAction::Cname("sinkhole.org".to_string()));

    let compiled = compiler.compile();
    assert!(compiled.contains("phishing.com IN CNAME sinkhole.org."));
    assert!(compiled.contains("*.phishing.com IN CNAME sinkhole.org."));
}

#[test]
fn test_no_wildcards() {
    let mut compiler = RpzCompiler::new("rpz.local")
        .include_wildcards(false);
    compiler.add_rule("bad.com", RpzAction::NxDomain);

    let compiled = compiler.compile();
    assert!(compiled.contains("bad.com IN CNAME ."));
    assert!(!compiled.contains("*.bad.com"));
}

#[test]
fn test_parse_feed_hosts_format() {
    let feed = "\
# This is a comment
127.0.0.1 bad.example.com
0.0.0.0 worse.example.com # another comment
";
    let mut compiler = RpzCompiler::new("rpz.local")
        .include_wildcards(false);
    compiler.parse_feed(feed, RpzAction::NxDomain);

    let compiled = compiler.compile();
    assert!(compiled.contains("bad.example.com IN CNAME ."));
    assert!(compiled.contains("worse.example.com IN CNAME ."));
}

#[test]
fn test_parse_feed_plain_format() {
    let feed = "\
malware1.com
malware2.com.
";
    let mut compiler = RpzCompiler::new("rpz.local")
        .include_wildcards(false);
    compiler.parse_feed(feed, RpzAction::NoData);

    let compiled = compiler.compile();
    assert!(compiled.contains("malware1.com IN CNAME *."));
    assert!(compiled.contains("malware2.com IN CNAME *."));
}

#[test]
fn test_deterministic_output() {
    let mut compiler = RpzCompiler::new("rpz.local");
    compiler.add_rule("z.com", RpzAction::NxDomain);
    compiler.add_rule("a.com", RpzAction::NxDomain);
    compiler.add_rule("m.com", RpzAction::NxDomain);

    let compiled = compiler.compile();
    let lines: Vec<&str> = compiled.lines().filter(|l| l.contains("IN CNAME")).collect();

    assert!(lines[0].starts_with("a.com"));
    assert!(lines[1].starts_with("*.a.com"));
    assert!(lines[2].starts_with("m.com"));
    assert!(lines[3].starts_with("*.m.com"));
    assert!(lines[4].starts_with("z.com"));
    assert!(lines[5].starts_with("*.z.com"));
}
