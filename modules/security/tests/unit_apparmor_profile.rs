#![forbid(unsafe_code)]

use security::apparmor_profile::AppArmorProfile;

#[test]
fn test_apparmor_profile_creation() {
    let profile = AppArmorProfile::new("fusionpanel-nginx", "/usr/sbin/nginx");
    assert_eq!(profile.name, "fusionpanel-nginx");
    assert_eq!(profile.executable_path, "/usr/sbin/nginx");
    assert!(profile.paths.is_empty());
    assert!(profile.network_families.is_empty());
    assert!(profile.disable_core_dumps); // default is true
}

#[test]
fn test_add_path_permission() {
    let mut profile = AppArmorProfile::new("test-app", "/bin/test");
    profile.add_path_permission("/etc/config.json", "r");
    profile.add_path_permission("/var/log/test.log", "w");

    assert_eq!(profile.paths.get("/etc/config.json").unwrap(), "r");
    assert_eq!(profile.paths.get("/var/log/test.log").unwrap(), "w");
}

#[test]
fn test_add_network_family() {
    let mut profile = AppArmorProfile::new("test-app", "/bin/test");
    profile.add_network_family("inet");
    profile.add_network_family("inet6");

    assert_eq!(profile.network_families, vec!["inet", "inet6"]);
}

#[test]
fn test_generate_profile() {
    let mut profile = AppArmorProfile::new("fusionpanel-app", "/usr/bin/app");
    profile.add_network_family("inet");
    profile.add_path_permission("/etc/app/config", "r");
    profile.add_path_permission("/var/log/app/*", "w");

    let expected = "\
abi <abi/3.0>,

include <tunables/global>

profile fusionpanel-app /usr/bin/app {
  include <abstractions/base>

  network inet,

  /etc/app/config r,
  /var/log/app/* w,
}
";

    assert_eq!(profile.generate_profile(), expected);
}

#[test]
fn test_sysctl_hardening_enabled() {
    let mut profile = AppArmorProfile::new("test", "/bin/test");
    profile.set_disable_core_dumps(true);
    assert_eq!(profile.generate_sysctl_hardening(), Some("fs.suid_dumpable = 0\n".to_string()));
}

#[test]
fn test_sysctl_hardening_disabled() {
    let mut profile = AppArmorProfile::new("test", "/bin/test");
    profile.set_disable_core_dumps(false);
    assert_eq!(profile.generate_sysctl_hardening(), None);
}
