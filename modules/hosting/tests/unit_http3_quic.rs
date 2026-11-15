#![forbid(unsafe_code)]

use hosting::http3_quic::{Http3QuicConfig, UdpListenerGuard};

#[test]
fn test_default_config_generation() {
    let mut guard = UdpListenerGuard::new();
    let config = Http3QuicConfig::default();

    let generated = config.generate_directives(&mut guard);
    assert!(generated.contains("listen 443 quic reuseport;\n"));
    assert!(generated.contains("listen [::]:443 quic reuseport;\n"));
    assert!(generated.contains("add_header Alt-Svc 'h3=\":443\"; ma=86400' always;\n"));
    assert!(generated.contains("ssl_early_data on;\n"));
}

#[test]
fn test_reuseport_guard_prevents_duplicates() {
    let mut guard = UdpListenerGuard::new();
    let config1 = Http3QuicConfig::default();
    let config2 = Http3QuicConfig::default();

    let gen1 = config1.generate_directives(&mut guard);
    assert!(gen1.contains("listen 443 quic reuseport;\n"));
    assert!(gen1.contains("listen [::]:443 quic reuseport;\n"));

    let gen2 = config2.generate_directives(&mut guard);
    // The second time for the same port, reuseport should be omitted
    assert!(gen2.contains("listen 443 quic;\n"));
    assert!(gen2.contains("listen [::]:443 quic;\n"));
    assert!(!gen2.contains("reuseport"));
}

#[test]
fn test_custom_config() {
    let mut guard = UdpListenerGuard::new();
    let config = Http3QuicConfig::new()
        .port(8443)
        .ipv6(false)
        .alt_svc_max_age(3600)
        .ssl_early_data(false);

    let generated = config.generate_directives(&mut guard);
    assert!(generated.contains("listen 8443 quic reuseport;\n"));
    assert!(!generated.contains("listen [::]:8443"));
    assert!(generated.contains("add_header Alt-Svc 'h3=\":8443\"; ma=3600' always;\n"));
    assert!(!generated.contains("ssl_early_data on;\n"));
}

#[test]
fn test_guard_release() {
    let mut guard = UdpListenerGuard::new();

    assert!(guard.reserve_reuseport(false, 443)); // First time works
    assert!(!guard.reserve_reuseport(false, 443)); // Second time fails

    guard.release_reuseport(false, 443);
    assert!(guard.reserve_reuseport(false, 443)); // Should work again after release
}

#[test]
fn test_guard_clear() {
    let mut guard = UdpListenerGuard::new();

    guard.reserve_reuseport(false, 443);
    guard.reserve_reuseport(true, 443);

    guard.clear();

    assert!(guard.reserve_reuseport(false, 443));
    assert!(guard.reserve_reuseport(true, 443));
}
