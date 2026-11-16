#![forbid(unsafe_code)]

use hosting::managed_site::{
    ManagedEntryKind, ManagedSiteError, ManagedSiteFs, ManagedSiteOperations, ManagedSiteSpec,
    NginxControl, NginxSiteAdapter, StdManagedSiteFs,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Default)]
struct FakeNginx {
    events: Mutex<Vec<&'static str>>,
    fail_candidate: Mutex<bool>,
    active_failures: Mutex<usize>,
    reload_failures: Mutex<usize>,
}

impl FakeNginx {
    fn failing_candidate() -> Self {
        Self {
            fail_candidate: Mutex::new(true),
            ..Self::default()
        }
    }

    fn failing_active(count: usize) -> Self {
        Self {
            active_failures: Mutex::new(count),
            ..Self::default()
        }
    }

    fn failing_reload(count: usize) -> Self {
        Self {
            reload_failures: Mutex::new(count),
            ..Self::default()
        }
    }

    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }
}

impl NginxControl for FakeNginx {
    fn validate_candidate(&self, config_path: &Path) -> Result<(), ManagedSiteError> {
        assert!(config_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("test-"));
        self.events.lock().unwrap().push("candidate");
        let mut fail = self.fail_candidate.lock().unwrap();
        if *fail {
            *fail = false;
            Err(ManagedSiteError::CandidateRejected)
        } else {
            Ok(())
        }
    }

    fn validate_active(&self, required_include: &str) -> Result<(), ManagedSiteError> {
        assert!(required_include.ends_with("/sites-enabled/*.conf;"));
        self.events.lock().unwrap().push("active");
        let mut failures = self.active_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            Err(ManagedSiteError::ActiveConfigRejected)
        } else {
            Ok(())
        }
    }

    fn reload(&self) -> Result<(), ManagedSiteError> {
        self.events.lock().unwrap().push("reload");
        let mut failures = self.reload_failures.lock().unwrap();
        if *failures > 0 {
            *failures -= 1;
            Err(ManagedSiteError::ReloadFailed)
        } else {
            Ok(())
        }
    }
}

struct FailTestConfigWrite {
    inner: StdManagedSiteFs,
}

impl ManagedSiteFs for FailTestConfigWrite {
    fn entry_kind(&self, path: &Path) -> Result<Option<ManagedEntryKind>, ManagedSiteError> {
        self.inner.entry_kind(path)
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), ManagedSiteError> {
        self.inner.ensure_directory(path)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, ManagedSiteError> {
        self.inner.read_link(path)
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ManagedSiteError> {
        self.inner.read_file(path)
    }

    fn write_new_file(&self, path: &Path, content: &[u8]) -> Result<(), ManagedSiteError> {
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("test-"))
        {
            Err(ManagedSiteError::Io)
        } else {
            self.inner.write_new_file(path, content)
        }
    }

    fn create_symlink(&self, target: &Path, link: &Path) -> Result<(), ManagedSiteError> {
        self.inner.create_symlink(target, link)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), ManagedSiteError> {
        self.inner.rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> Result<(), ManagedSiteError> {
        self.inner.remove_file(path)
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, ManagedSiteError> {
        self.inner.list_directory(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, ManagedSiteError> {
        self.inner.canonicalize(path)
    }
}

struct Fixture {
    _temp: TempDir,
    managed_root: PathBuf,
    document_root: PathBuf,
    site_id: Uuid,
    adapter: NginxSiteAdapter,
}

fn fixture(nginx: Arc<FakeNginx>) -> Fixture {
    fixture_with_fs(nginx, Arc::new(StdManagedSiteFs))
}

fn fixture_with_fs(nginx: Arc<FakeNginx>, filesystem: Arc<dyn ManagedSiteFs>) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let managed_root = temp.path().join("nginx");
    let document_root_base = temp.path().join("site-roots");
    let document_root = document_root_base.join("example.test");
    fs::create_dir_all(&document_root).unwrap();
    let adapter = NginxSiteAdapter::with_components(
        managed_root.clone(),
        document_root_base,
        filesystem,
        nginx,
    );
    Fixture {
        _temp: temp,
        managed_root,
        document_root,
        site_id: Uuid::new_v4(),
        adapter,
    }
}

fn static_spec(root: &Path) -> ManagedSiteSpec {
    ManagedSiteSpec::Static {
        document_root: root.to_string_lossy().into_owned(),
    }
}

fn active_link(root: &Path) -> PathBuf {
    root.join("sites-enabled/example.test.conf")
}

fn active_target(root: &Path) -> PathBuf {
    fs::read_link(active_link(root)).unwrap()
}

fn read_active_config(root: &Path) -> Vec<u8> {
    fs::read(active_target(root)).unwrap()
}

fn apply(fixture: &Fixture) -> Result<(), ManagedSiteError> {
    fixture.adapter.apply_site(
        fixture.site_id,
        "example.test",
        &static_spec(&fixture.document_root),
    )
}

#[test]
fn apply_stages_and_validates_before_atomic_promotion_and_reload() {
    let nginx = Arc::new(FakeNginx::default());
    let fixture = fixture(nginx.clone());

    apply(&fixture).unwrap();

    let target = active_target(&fixture.managed_root);
    assert!(target.starts_with(fixture.managed_root.join("sites-available")));
    let config = String::from_utf8(read_active_config(&fixture.managed_root)).unwrap();
    assert!(config.contains("FusionPanel managed site:"));
    assert!(config.contains(&format!("root {};", fixture.document_root.display())));
    assert_eq!(nginx.events(), vec!["candidate", "active", "reload"]);
}

#[test]
fn reverse_proxy_renders_the_typed_upstream_without_config_text_interpolation() {
    let nginx = Arc::new(FakeNginx::default());
    let fixture = fixture(nginx.clone());
    let spec = ManagedSiteSpec::ReverseProxy {
        upstream_url: "http://unresolvable.invalid".to_string(),
    };

    fixture
        .adapter
        .apply_site(fixture.site_id, "example.test", &spec)
        .unwrap();

    let config = String::from_utf8(read_active_config(&fixture.managed_root)).unwrap();
    assert!(config.contains("proxy_pass http://unresolvable.invalid;"));
    assert_eq!(nginx.events(), vec!["candidate", "active", "reload"]);
}

#[test]
fn apply_rejects_bad_upstream_and_static_root_without_writing_configs() {
    let nginx = Arc::new(FakeNginx::default());
    let fixture = fixture(nginx.clone());
    let invalid_upstream = ManagedSiteSpec::ReverseProxy {
        upstream_url: "http://backend.test; include /etc/passwd".to_string(),
    };
    assert_eq!(
        fixture
            .adapter
            .apply_site(fixture.site_id, "example.test", &invalid_upstream),
        Err(ManagedSiteError::InvalidInput)
    );
    let invalid_root = ManagedSiteSpec::Static {
        document_root: "/srv/fusionpanel/sites/other.test".to_string(),
    };
    assert_eq!(
        fixture
            .adapter
            .apply_site(fixture.site_id, "example.test", &invalid_root),
        Err(ManagedSiteError::InvalidInput)
    );
    assert!(nginx.events().is_empty());
    assert!(!fixture.managed_root.exists());
}

#[test]
fn candidate_test_failure_preserves_prior_link_and_config() {
    let initial = fixture(Arc::new(FakeNginx::default()));
    apply(&initial).unwrap();
    let prior_target = active_target(&initial.managed_root);
    let prior_config = read_active_config(&initial.managed_root);

    let adapter = NginxSiteAdapter::with_components(
        initial.managed_root.clone(),
        initial._temp.path().join("site-roots"),
        Arc::new(StdManagedSiteFs),
        Arc::new(FakeNginx::failing_candidate()),
    );
    let result = adapter.apply_site(
        initial.site_id,
        "example.test",
        &static_spec(&initial.document_root),
    );

    assert_eq!(result, Err(ManagedSiteError::CandidateRejected));
    assert_eq!(active_target(&initial.managed_root), prior_target);
    assert_eq!(read_active_config(&initial.managed_root), prior_config);
}

#[test]
fn active_test_failure_restores_exact_previous_link_and_config() {
    let initial = fixture(Arc::new(FakeNginx::default()));
    apply(&initial).unwrap();
    let prior_target = active_target(&initial.managed_root);
    let prior_config = read_active_config(&initial.managed_root);
    let nginx = Arc::new(FakeNginx::failing_active(1));
    let adapter = NginxSiteAdapter::with_components(
        initial.managed_root.clone(),
        initial._temp.path().join("site-roots"),
        Arc::new(StdManagedSiteFs),
        nginx.clone(),
    );

    let result = adapter.apply_site(
        initial.site_id,
        "example.test",
        &static_spec(&initial.document_root),
    );

    assert_eq!(result, Err(ManagedSiteError::ActiveConfigRejected));
    assert_eq!(active_target(&initial.managed_root), prior_target);
    assert_eq!(read_active_config(&initial.managed_root), prior_config);
    assert_eq!(nginx.events(), vec!["candidate", "active"]);
}

#[test]
fn reload_failure_restores_old_link_and_reloads_the_previous_config() {
    let initial = fixture(Arc::new(FakeNginx::default()));
    apply(&initial).unwrap();
    let prior_target = active_target(&initial.managed_root);
    let prior_config = read_active_config(&initial.managed_root);
    let nginx = Arc::new(FakeNginx::failing_reload(1));
    let adapter = NginxSiteAdapter::with_components(
        initial.managed_root.clone(),
        initial._temp.path().join("site-roots"),
        Arc::new(StdManagedSiteFs),
        nginx.clone(),
    );

    assert_eq!(
        adapter.apply_site(
            initial.site_id,
            "example.test",
            &static_spec(&initial.document_root)
        ),
        Err(ManagedSiteError::ReloadFailed)
    );
    assert_eq!(active_target(&initial.managed_root), prior_target);
    assert_eq!(read_active_config(&initial.managed_root), prior_config);
    assert_eq!(
        nginx.events(),
        vec!["candidate", "active", "reload", "active", "reload"]
    );
}

#[test]
fn staging_write_failure_removes_candidate_and_keeps_old_site_active() {
    let initial = fixture(Arc::new(FakeNginx::default()));
    apply(&initial).unwrap();
    let prior_target = active_target(&initial.managed_root);
    let prior_config = read_active_config(&initial.managed_root);
    let adapter = NginxSiteAdapter::with_components(
        initial.managed_root.clone(),
        initial._temp.path().join("site-roots"),
        Arc::new(FailTestConfigWrite {
            inner: StdManagedSiteFs,
        }),
        Arc::new(FakeNginx::default()),
    );

    assert_eq!(
        adapter.apply_site(
            initial.site_id,
            "example.test",
            &static_spec(&initial.document_root)
        ),
        Err(ManagedSiteError::Io)
    );
    assert_eq!(active_target(&initial.managed_root), prior_target);
    assert_eq!(read_active_config(&initial.managed_root), prior_config);
    let staging = fs::read_dir(initial.managed_root.join("staging")).unwrap();
    assert_eq!(staging.count(), 0);
}

#[test]
fn delete_reload_failure_restores_exact_previous_link_and_config() {
    let initial = fixture(Arc::new(FakeNginx::default()));
    apply(&initial).unwrap();
    let prior_target = active_target(&initial.managed_root);
    let prior_config = read_active_config(&initial.managed_root);
    let nginx = Arc::new(FakeNginx::failing_reload(1));
    let adapter = NginxSiteAdapter::with_components(
        initial.managed_root.clone(),
        initial._temp.path().join("site-roots"),
        Arc::new(StdManagedSiteFs),
        nginx.clone(),
    );

    assert_eq!(
        adapter.delete_site(initial.site_id, "example.test"),
        Err(ManagedSiteError::ReloadFailed)
    );
    assert_eq!(active_target(&initial.managed_root), prior_target);
    assert_eq!(read_active_config(&initial.managed_root), prior_config);
    assert_eq!(
        nginx.events(),
        vec!["candidate", "active", "reload", "active", "reload"]
    );
}

#[test]
fn delete_success_removes_only_its_managed_link_after_validation_and_reload() {
    let nginx = Arc::new(FakeNginx::default());
    let fixture = fixture(nginx.clone());
    apply(&fixture).unwrap();
    let target = active_target(&fixture.managed_root);

    fixture
        .adapter
        .delete_site(fixture.site_id, "example.test")
        .unwrap();

    assert!(!active_link(&fixture.managed_root).exists());
    assert!(!target.exists());
    assert_eq!(&nginx.events()[3..], &["candidate", "active", "reload"]);
}

#[cfg(unix)]
#[test]
fn symlinked_document_root_is_rejected() {
    let nginx = Arc::new(FakeNginx::default());
    let fixture = fixture(nginx.clone());
    let outside = fixture._temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let site_root = fixture._temp.path().join("site-roots/example.test");
    fs::remove_dir(&site_root).unwrap();
    std::os::unix::fs::symlink(&outside, &site_root).unwrap();

    assert_eq!(apply(&fixture), Err(ManagedSiteError::UnsafePath));
    assert!(nginx.events().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_managed_root_is_rejected_without_touching_its_target() {
    let temp = tempfile::tempdir().unwrap();
    let actual = temp.path().join("real-nginx");
    fs::create_dir(&actual).unwrap();
    let root_link = temp.path().join("nginx");
    std::os::unix::fs::symlink(&actual, &root_link).unwrap();
    let document_root_base = temp.path().join("site-roots");
    let document_root = document_root_base.join("example.test");
    fs::create_dir_all(&document_root).unwrap();
    let adapter = NginxSiteAdapter::with_components(
        root_link,
        document_root_base,
        Arc::new(StdManagedSiteFs),
        Arc::new(FakeNginx::default()),
    );

    assert_eq!(
        adapter.apply_site(Uuid::new_v4(), "example.test", &static_spec(&document_root)),
        Err(ManagedSiteError::UnsafePath)
    );
    assert_eq!(fs::read_dir(actual).unwrap().count(), 0);
}

#[test]
fn missing_managed_parent_is_not_created_outside_the_fusionpanel_directory() {
    let temp = tempfile::tempdir().unwrap();
    let missing_parent = temp.path().join("nginx-parent-must-already-exist");
    let managed_root = missing_parent.join("fusionpanel");
    let document_root_base = temp.path().join("site-roots");
    let adapter = NginxSiteAdapter::with_components(
        managed_root,
        document_root_base,
        Arc::new(StdManagedSiteFs),
        Arc::new(FakeNginx::default()),
    );

    assert_eq!(
        adapter.apply_site(
            Uuid::new_v4(),
            "example.test",
            &ManagedSiteSpec::ReverseProxy {
                upstream_url: "http://127.0.0.1:8080".to_string(),
            }
        ),
        Err(ManagedSiteError::UnsafePath)
    );
    assert!(!missing_parent.exists());
}
