#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::schema::{AppTemplate, TemplateError};

pub struct TempDir {
    pub path: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub struct CatalogInstaller;

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn create_secure_temp_dir(base: &Path) -> Result<TempDir, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir_name = format!("app_install_{}_{}", now, counter);
    let path = base.join(dir_name);

    fs::create_dir(&path).map_err(|e| format!("Failed to create temp dir: {}", e))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to set temp dir permissions: {}", e))?;

    Ok(TempDir { path })
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
        if template.unsupported_hardware.iter().any(|hw| hw == host_hardware) {
            return Err(format!("Hardware '{}' is not supported by this template", host_hardware));
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
                    let pwd = Self::generate_password(16).map_err(|e| format!("Failed to generate password: {}", e))?;
                    final_values.insert(var_name.clone(), pwd.clone());
                    allocated.insert(var_name.clone(), pwd);
                }
            }
        }
        
        // Include ownership as a variable if it's referenced in the template
        if !final_values.contains_key("OWNER") {
            final_values.insert("OWNER".to_string(), owner.to_string());
        }

        let rendered_compose = template.render(&final_values).map_err(|e: TemplateError| e.to_string())?;
        
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
        let temp_dir = create_secure_temp_dir(base_dir)?;
        let compose_file_path = temp_dir.path.join("docker-compose.yml");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&compose_file_path)
            .map_err(|e| format!("Failed to open compose file for writing: {}", e))?;
        file.write_all(compose_content.as_bytes())
            .map_err(|e| format!("Failed to write compose file: {}", e))?;

        let output = Command::new("docker")
            .args(&["compose", "-f", compose_file_path.to_str().unwrap(), "-p", app_name, "up", "-d"])
            .output()
            .map_err(|e| format!("Failed to execute docker compose: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            // Compensation
            let _ = Command::new("docker")
                .args(&["compose", "-f", compose_file_path.to_str().unwrap(), "-p", app_name, "down", "-v"])
                .output();
            return Err(format!("Docker compose up failed: {}", err));
        }

        // Installation succeeded, we don't want to drop the temp dir anymore as it is now the active install directory.
        // We will "leak" the path out of the TempDir construct. 
        // In a real application, we might move this to a permanent location, but for here, we can just consume it into a persistent path.
        // However, a better pattern is to return the path and let the caller manage it, or provide a way to 'disarm' the Drop.
        // Since we have a strict read-only system and test constraints, we'll disarm it by moving the path string and manually clearing the TempDir's path to prevent deletion, but that requires `Option<PathBuf>` which is complicated.
        // Let's modify TempDir to allow disarming or just rename it.
        // For simplicity and since tests manage their own directories, let's just rename it to a permanent name.
        let permanent_path = base_dir.join(format!("{}_active", app_name));
        if permanent_path.exists() {
             fs::remove_dir_all(&permanent_path).unwrap_or(());
        }
        fs::rename(&temp_dir.path, &permanent_path)
             .map_err(|e| format!("Failed to persist installation directory: {}", e))?;
        
        // Disable dropping of the original dir, it's already moved
        // temp_dir will try to remove a non-existent dir which is fine since we ignore the result.
        
        Ok(permanent_path)
    }

    pub fn check_health(&self, app_name: &str, install_dir: &Path) -> Result<bool, String> {
        let compose_file_path = install_dir.join("docker-compose.yml");
        if !compose_file_path.exists() {
             return Err("Installation directory or compose file missing".to_string());
        }
        
        let output = Command::new("docker")
            .args(&["compose", "-f", compose_file_path.to_str().unwrap(), "-p", app_name, "ps", "--format", "json"])
            .output()
            .map_err(|e| format!("Failed to execute docker compose ps: {}", e))?;

        if !output.status.success() {
            return Ok(false);
        }

        let json = String::from_utf8_lossy(&output.stdout);
        // Basic health check: ensuring the output is valid and not empty. A real implementation would parse the JSON and check container states.
        if json.trim().is_empty() || json.contains("\"State\":\"exited\"") || json.contains("\"State\":\"dead\"") {
             return Ok(false);
        }

        Ok(true)
    }

    pub fn delete(&self, app_name: &str, install_dir: &Path) -> Result<(), String> {
        let compose_file_path = install_dir.join("docker-compose.yml");
        if compose_file_path.exists() {
             let output = Command::new("docker")
                .args(&["compose", "-f", compose_file_path.to_str().unwrap(), "-p", app_name, "down", "-v"])
                .output()
                .map_err(|e| format!("Failed to execute docker compose down: {}", e))?;

            if !output.status.success() {
                 let err = String::from_utf8_lossy(&output.stderr);
                 return Err(format!("Failed to delete app: {}", err));
            }
        }
        
        fs::remove_dir_all(install_dir).map_err(|e| format!("Failed to remove install dir: {}", e))?;
        Ok(())
    }

    pub fn render_compose(template: &str, config: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        let mut keys: Vec<&String> = config.keys().collect();
        // Sort keys by length descending to prevent partial prefix replacement
        keys.sort_by(|a, b| b.len().cmp(&a.len()));

        for k in keys {
            let v = config.get(k).unwrap();
            let var_1 = format!("${{{}}}", k);
            let var_2 = format!("${}", k);
            result = result.replace(&var_1, v);
            result = result.replace(&var_2, v);
        }
        result
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
