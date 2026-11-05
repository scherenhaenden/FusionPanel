#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::scanner::ImageScanner;
use crate::schema::{AppTemplate, TemplateError};

pub struct TempDir {
    pub path: PathBuf,
    cleanup_on_drop: bool,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub struct CatalogInstaller;

const PROJECT_NAME_FILE: &str = ".compose-project";

fn create_secure_install_dir(base: &Path, app_name: &str) -> Result<TempDir, String> {
    let path = base.join(format!("{app_name}_active"));
    fs::create_dir(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("An installation already exists at {}", path.display())
        } else {
            format!("Failed to create install directory: {e}")
        }
    })?;
    if let Err(error) = fs::set_permissions(&path, fs::Permissions::from_mode(0o700)) {
        let _ = fs::remove_dir(&path);
        return Err(format!(
            "Failed to set install directory permissions: {error}"
        ));
    }

    Ok(TempDir {
        path,
        cleanup_on_drop: true,
    })
}

fn generate_compose_project_name() -> Result<String, String> {
    let mut random = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut random))
        .map_err(|e| format!("Failed to generate Compose project identity: {e}"))?;

    let mut suffix = String::with_capacity(random.len() * 2);
    for byte in random {
        suffix.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        suffix.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    Ok(format!("fp-{suffix}"))
}

fn write_compose_project_name(install_dir: &Path, project_name: &str) -> Result<(), String> {
    let path = install_dir.join(PROJECT_NAME_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("Failed to store Compose project identity: {e}"))?;
    file.write_all(project_name.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Failed to persist Compose project identity: {e}"))
}

fn validate_app_name(app_name: &str) -> Result<(), String> {
    let valid = !app_name.is_empty()
        && app_name.len() <= 63
        && app_name
            .as_bytes()
            .first()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && app_name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
        });
    if valid {
        Ok(())
    } else {
        Err("App name must be 1-63 lowercase letters, digits, hyphens, or underscores and start with a letter or digit".to_string())
    }
}

fn compose_down(compose_file_path: &Path, app_name: &str) -> Result<(), String> {
    let output = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file_path
                .to_str()
                .ok_or_else(|| "Compose file path is not valid UTF-8".to_string())?,
            "-p",
            app_name,
            "down",
        ])
        .output()
        .map_err(|e| format!("Failed to execute docker compose down: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "docker compose down failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn compose_output_is_healthy(json_lines: &str) -> bool {
    let mut count = 0;
    for line in json_lines.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(container) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        let state = container
            .get("State")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let health = container
            .get("Health")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !state.eq_ignore_ascii_case("running")
            || (!health.is_empty() && !health.eq_ignore_ascii_case("healthy"))
        {
            return false;
        }
        count += 1;
    }
    count > 0
}

fn validate_install_dir(app_name: &str, install_dir: &Path) -> Result<(PathBuf, String), String> {
    validate_app_name(app_name)?;
    let expected_name = format!("{app_name}_active");
    if install_dir.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return Err("Installation path does not match the app name".to_string());
    }

    let dir_metadata = fs::symlink_metadata(install_dir)
        .map_err(|e| format!("Failed to inspect installation directory: {e}"))?;
    if !dir_metadata.file_type().is_dir() {
        return Err("Installation path must be a directory, not a symlink".to_string());
    }

    let compose_file_path = install_dir.join("docker-compose.yml");
    let file_metadata = fs::symlink_metadata(&compose_file_path)
        .map_err(|e| format!("Installation directory or compose file missing: {e}"))?;
    if !file_metadata.file_type().is_file() {
        return Err("Compose file must be a regular file, not a symlink".to_string());
    }

    let project_name_path = install_dir.join(PROJECT_NAME_FILE);
    let project_metadata = fs::symlink_metadata(&project_name_path)
        .map_err(|e| format!("Compose project identity is missing: {e}"))?;
    if !project_metadata.file_type().is_file() {
        return Err("Compose project identity must be a regular file, not a symlink".to_string());
    }
    let mut project_name = String::new();
    File::open(&project_name_path)
        .and_then(|mut file| file.read_to_string(&mut project_name))
        .map_err(|e| format!("Failed to read Compose project identity: {e}"))?;
    if project_name.trim() != project_name || project_name.is_empty() {
        return Err("Compose project identity is malformed".to_string());
    }
    validate_app_name(&project_name)?;

    Ok((compose_file_path, project_name))
}

fn scan_compose_images(compose_file_path: &Path, project_name: &str) -> Result<(), String> {
    let compose_file = compose_file_path
        .to_str()
        .ok_or_else(|| "Compose file path is not valid UTF-8".to_string())?;
    let output = Command::new("docker")
        .args([
            "compose",
            "-f",
            compose_file,
            "-p",
            project_name,
            "config",
            "--images",
        ])
        .output()
        .map_err(|e| format!("Failed to resolve Compose images: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Failed to resolve Compose images: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let images: std::collections::BTreeSet<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|image| !image.is_empty())
        .map(str::to_owned)
        .collect();
    if images.is_empty() {
        return Err("Compose configuration contains no resolvable images".to_string());
    }

    for image in images {
        let reports = ImageScanner::scan_image(&image)
            .map_err(|e| format!("Image scan failed for {image}: {e}"))?;
        ImageScanner::gate_deployment(&reports)
            .map_err(|e| format!("Image {image} failed the vulnerability gate: {e}"))?;
    }
    Ok(())
}

impl CatalogInstaller {
    pub fn new() -> Self {
        Self
    }

    pub fn deploy_template(
        &self,
        template: &AppTemplate,
        app_name: &str,
        user_values: &HashMap<String, String>,
        base_dir: &Path,
        host_hardware: &str,
        used_ports: &[u16],
        owner: &str,
    ) -> Result<(PathBuf, HashMap<String, String>), String> {
        if template
            .unsupported_hardware
            .iter()
            .any(|hw| hw == host_hardware)
        {
            return Err(format!(
                "Hardware '{}' is not supported by this template",
                host_hardware
            ));
        }

        let mut final_values = user_values.clone();

        // Allocate missing ports and secrets based on naming conventions
        let mut allocated = HashMap::new();

        for (var_name, _var_def) in &template.variables {
            if !final_values.contains_key::<String>(var_name) {
                if var_name.contains("PORT") {
                    if let Some(p) = Self::allocate_port(10000, 20000, used_ports) {
                        let p_str = p.to_string();
                        final_values.insert(var_name.clone(), p_str.clone());
                        allocated.insert(var_name.clone(), p_str);
                    } else {
                        return Err(format!("Failed to allocate port for {}", var_name));
                    }
                } else if var_name.contains("PASSWORD") || var_name.contains("SECRET") {
                    let pwd = Self::generate_password(16)
                        .map_err(|e| format!("Failed to generate password: {}", e))?;
                    final_values.insert(var_name.clone(), pwd.clone());
                    allocated.insert(var_name.clone(), pwd);
                }
            }
        }

        // Include ownership as a variable if it's referenced in the template
        if !final_values.contains_key("OWNER") {
            final_values.insert("OWNER".to_string(), owner.to_string());
        }

        let rendered_compose = template
            .render(&final_values)
            .map_err(|e: TemplateError| e.to_string())?;

        match self.install(app_name, &rendered_compose, base_dir) {
            Ok(path) => {
                // Ownership and allocations succeeded and are compensated on failure (by dropping)
                Ok((path, allocated))
            }
            Err(e) => {
                // Compensation: The temporary directory and its docker-compose run are torn down
                // by the `install` method. Any ports or secrets we "allocated" in memory are discarded.
                // So we just return the error.
                Err(e)
            }
        }
    }

    pub fn install(
        &self,
        app_name: &str,
        compose_content: &str,
        base_dir: &Path,
    ) -> Result<PathBuf, String> {
        validate_app_name(app_name)?;
        let mut install_dir = create_secure_install_dir(base_dir, app_name)?;
        let compose_file_path = install_dir.path.join("docker-compose.yml");
        let project_name = generate_compose_project_name()?;
        write_compose_project_name(&install_dir.path, &project_name)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&compose_file_path)
            .map_err(|e| format!("Failed to open compose file for writing: {}", e))?;
        file.write_all(compose_content.as_bytes())
            .map_err(|e| format!("Failed to write compose file: {}", e))?;

        let compose_file = compose_file_path
            .to_str()
            .ok_or_else(|| "Compose file path is not valid UTF-8".to_string())?;
        scan_compose_images(&compose_file_path, &project_name)?;
        let output = Command::new("docker")
            .args([
                "compose",
                "-f",
                compose_file,
                "-p",
                &project_name,
                "up",
                "-d",
                "--wait",
                "--wait-timeout",
                "60",
            ])
            .output()
            .map_err(|e| format!("Failed to execute docker compose: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if let Err(cleanup_error) = compose_down(&compose_file_path, &project_name) {
                install_dir.cleanup_on_drop = false;
                return Err(format!(
                    "Docker compose up failed: {}; cleanup also failed: {}. Compose file retained at {}",
                    err.trim(),
                    cleanup_error,
                    install_dir.path.display()
                ));
            }
            return Err(format!("Docker compose up failed: {}", err.trim()));
        }

        let health = self.check_health(app_name, &install_dir.path);
        let is_healthy = match health {
            Ok(is_healthy) => is_healthy,
            Err(health_error) => {
                if let Err(cleanup_error) = compose_down(&compose_file_path, &project_name) {
                    install_dir.cleanup_on_drop = false;
                    return Err(format!(
                        "Compose health check failed: {health_error}; cleanup also failed: {cleanup_error}. Compose file retained at {}",
                        install_dir.path.display()
                    ));
                }
                return Err(format!(
                    "Compose health check failed: {health_error}; deployment was rolled back"
                ));
            }
        };
        if !is_healthy {
            if let Err(cleanup_error) = compose_down(&compose_file_path, &project_name) {
                install_dir.cleanup_on_drop = false;
                return Err(format!(
                    "Compose project did not become healthy; cleanup failed: {cleanup_error}. Compose file retained at {}",
                    install_dir.path.display()
                ));
            }
            return Err(
                "Compose project did not become healthy; deployment was rolled back".to_string(),
            );
        }

        install_dir.cleanup_on_drop = false;
        Ok(install_dir.path.clone())
    }

    pub fn check_health(&self, app_name: &str, install_dir: &Path) -> Result<bool, String> {
        let (compose_file_path, project_name) = validate_install_dir(app_name, install_dir)?;
        let output = Command::new("docker")
            .args([
                "compose",
                "-f",
                compose_file_path
                    .to_str()
                    .ok_or_else(|| "Compose file path is not valid UTF-8".to_string())?,
                "-p",
                &project_name,
                "ps",
                "--all",
                "--format",
                "json",
            ])
            .output()
            .map_err(|e| format!("Failed to execute docker compose ps: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "docker compose ps failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let json = String::from_utf8_lossy(&output.stdout);
        Ok(compose_output_is_healthy(&json))
    }

    pub fn delete(&self, app_name: &str, install_dir: &Path) -> Result<(), String> {
        let (compose_file_path, project_name) = validate_install_dir(app_name, install_dir)?;
        compose_down(&compose_file_path, &project_name)
            .map_err(|e| format!("Failed to delete app: {e}"))?;
        fs::remove_dir_all(install_dir)
            .map_err(|e| format!("Failed to remove install dir: {}", e))?;
        Ok(())
    }

    pub fn render_compose(
        template: &str,
        config: &HashMap<String, String>,
    ) -> Result<String, TemplateError> {
        crate::schema::render_yaml_template(template, config, true)
    }

    pub fn allocate_port(min: u16, max: u16, used_ports: &[u16]) -> Option<u16> {
        for port in min..=max {
            if !used_ports.contains(&port) {
                return Some(port);
            }
        }
        None
    }

    pub fn generate_password(length: usize) -> Result<String, std::io::Error> {
        let mut file = File::open("/dev/urandom")?;
        let mut buf = vec![0u8; length];
        file.read_exact(&mut buf)?;

        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let password: String = buf
            .into_iter()
            .map(|b| CHARSET[(b as usize) % CHARSET.len()] as char)
            .collect();

        Ok(password)
    }
}

impl Default for CatalogInstaller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compose_output_is_healthy, generate_compose_project_name, validate_app_name,
        CatalogInstaller,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("app-catalog-test-{stamp}"));
        fs::create_dir(&path).expect("test directory should be created");
        path
    }

    #[test]
    fn accepts_safe_compose_project_names() {
        assert!(validate_app_name("app-1_test").is_ok());
        assert!(validate_app_name("1app").is_ok());
    }

    #[test]
    fn generates_distinct_safe_compose_project_names() {
        let first = generate_compose_project_name().unwrap();
        let second = generate_compose_project_name().unwrap();
        assert_ne!(first, second);
        assert!(validate_app_name(&first).is_ok());
        assert!(validate_app_name(&second).is_ok());
    }

    #[test]
    fn rejects_unsafe_compose_project_names() {
        let long_name = "a".repeat(64);
        for app_name in [
            "",
            "../outside",
            "app/name",
            "Uppercase",
            "-leading",
            &long_name,
        ] {
            assert!(
                validate_app_name(app_name).is_err(),
                "accepted {app_name:?}"
            );
        }
    }

    #[test]
    fn health_requires_every_container_to_be_running_and_healthy() {
        assert!(compose_output_is_healthy(
            r#"{"State":"running","Health":"healthy"}
{"State":"running","Health":""}"#
        ));
        assert!(!compose_output_is_healthy(
            r#"{"State":"exited","Health":""}"#
        ));
        assert!(!compose_output_is_healthy(
            r#"{"State":"running","Health":"starting"}"#
        ));
        assert!(!compose_output_is_healthy(""));
        assert!(!compose_output_is_healthy("not json"));
    }

    #[test]
    fn install_rejects_path_traversal_before_creating_anything() {
        let base = test_dir();
        let result = CatalogInstaller::new().install("../outside", "services: {}", &base);
        assert!(result.is_err());
        assert_eq!(fs::read_dir(&base).unwrap().count(), 0);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn install_never_removes_an_existing_installation() {
        let base = test_dir();
        let install_dir = base.join("existing_active");
        fs::create_dir(&install_dir).unwrap();
        let marker = install_dir.join("keep.txt");
        fs::write(&marker, "preserve").unwrap();

        let result = CatalogInstaller::new().install("existing", "services: {}", &base);
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&marker).unwrap(), "preserve");
        fs::remove_dir_all(base).unwrap();
    }
}
