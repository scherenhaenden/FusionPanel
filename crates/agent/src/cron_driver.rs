#![forbid(unsafe_code)]

use fusionpanel_agent_protocol::{
    CronApplyPayload, CronDeletePayload, CronJobSpec, CronRunNowPayload,
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

const DEFAULT_CRON_DIR: &str = "/etc/cron.d";
const DEFAULT_CRON_USER: &str = "www-data";
const RUN_NOW_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT_CHARS: usize = 512;

fn cron_dir() -> PathBuf {
    std::env::var("FUSIONPANEL_CRON_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CRON_DIR))
}

fn cron_user() -> String {
    std::env::var("FUSIONPANEL_CRON_USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CRON_USER.to_string())
}

pub fn render_tenant_crontab(tenant_id: Uuid, user: &str, jobs: &[CronJobSpec]) -> String {
    let mut out = format!(
        "# FusionPanel managed crontab for tenant {tenant_id}\n\
         SHELL=/bin/sh\n\
         PATH=/usr/local/sbin:/usr/local/bin:/sbin:/bin:/usr/sbin:/usr/bin\n"
    );
    for job in jobs {
        if job.enabled {
            out.push_str(&format!(
                "# {} ({})\n{} {} {}\n",
                job.name, job.cron_id, job.schedule, user, job.command
            ));
        } else {
            out.push_str(&format!(
                "# {} ({}) [disabled]\n# {} {} {}\n",
                job.name, job.cron_id, job.schedule, user, job.command
            ));
        }
    }
    out
}

async fn write_tenant_crontab_in(
    dir: &Path,
    tenant_id: Uuid,
    user: &str,
    jobs: &[CronJobSpec],
) -> Result<(), String> {
    let target = dir.join(format!("fusionpanel-{tenant_id}"));
    if jobs.is_empty() {
        match tokio::fs::remove_file(&target).await {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(format!(
                    "failed to remove crontab {}: {err}",
                    target.display()
                ))
            }
        }
    }

    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|err| format!("failed to create cron dir {}: {err}", dir.display()))?;

    let content = render_tenant_crontab(tenant_id, user, jobs);
    let tmp = dir.join(format!(
        ".fusionpanel-{tenant_id}.tmp.{}",
        std::process::id()
    ));
    tokio::fs::write(&tmp, content.as_bytes())
        .await
        .map_err(|err| format!("failed to write temp crontab {}: {err}", tmp.display()))?;
    if let Err(err) = tokio::fs::rename(&tmp, &target).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(format!(
            "failed to rename crontab {} -> {}: {err}",
            tmp.display(),
            target.display()
        ));
    }
    Ok(())
}

pub async fn execute_cron_apply(payload: &CronApplyPayload) -> Result<(), String> {
    let dir = cron_dir();
    let user = cron_user();
    write_tenant_crontab_in(&dir, payload.tenant_id, &user, &payload.jobs).await
}

pub async fn execute_cron_delete(payload: &CronDeletePayload) -> Result<(), String> {
    let dir = cron_dir();
    let user = cron_user();
    write_tenant_crontab_in(&dir, payload.tenant_id, &user, &payload.jobs).await
}

fn truncate_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= MAX_OUTPUT_CHARS {
        return trimmed.to_string();
    }
    let prefix: String = trimmed.chars().take(MAX_OUTPUT_CHARS).collect();
    format!("{prefix}...")
}

pub async fn execute_cron_run_now(payload: &CronRunNowPayload) -> Result<String, String> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&payload.command);

    let output = timeout(RUN_NOW_TIMEOUT, cmd.output())
        .await
        .map_err(|_| {
            format!(
                "cron.run_now timed out after {}s for {}",
                RUN_NOW_TIMEOUT.as_secs(),
                payload.cron_id
            )
        })?
        .map_err(|err| format!("failed to spawn cron command: {err}"))?;

    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr));

    if output.status.success() {
        if stdout.is_empty() {
            Ok(format!(
                "cron.run_now {} exited with status 0",
                payload.cron_id
            ))
        } else {
            Ok(format!(
                "cron.run_now {} exited 0: {stdout}",
                payload.cron_id
            ))
        }
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(format!(
            "cron.run_now {} exited with {code}: {detail}",
            payload.cron_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fusionpanel-cron-test-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn render_tenant_crontab_formats_enabled_and_disabled_jobs() {
        let tenant_id = Uuid::new_v4();
        let enabled_id = Uuid::new_v4();
        let disabled_id = Uuid::new_v4();
        let rendered = render_tenant_crontab(
            tenant_id,
            "www-data",
            &[
                CronJobSpec {
                    cron_id: enabled_id,
                    name: "nightly-cache".to_string(),
                    schedule: "0 3 * * *".to_string(),
                    command: "/usr/bin/true".to_string(),
                    enabled: true,
                },
                CronJobSpec {
                    cron_id: disabled_id,
                    name: "paused-job".to_string(),
                    schedule: "*/5 * * * *".to_string(),
                    command: "/bin/echo paused".to_string(),
                    enabled: false,
                },
            ],
        );

        assert!(rendered.contains(&format!("0 3 * * * www-data /usr/bin/true")));
        assert!(rendered.contains(&format!("# */5 * * * * www-data /bin/echo paused")));
    }

    #[tokio::test]
    async fn apply_and_delete_crontab_files() {
        let dir = unique_temp_dir("files");
        let tenant_id = Uuid::new_v4();
        let cron_id = Uuid::new_v4();
        let jobs = vec![CronJobSpec {
            cron_id,
            name: "heartbeat".to_string(),
            schedule: "* * * * *".to_string(),
            command: "echo ok".to_string(),
            enabled: true,
        }];

        write_tenant_crontab_in(&dir, tenant_id, "www-data", &jobs)
            .await
            .expect("write crontab");
        let target = dir.join(format!("fusionpanel-{tenant_id}"));
        let written = tokio::fs::read_to_string(&target).await.expect("read");
        assert!(written.contains("* * * * * www-data echo ok"));

        write_tenant_crontab_in(&dir, tenant_id, "www-data", &[])
            .await
            .expect("remove crontab");
        assert!(!target.exists());

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn run_now_executes_command_and_captures_output() {
        let ok = execute_cron_run_now(&CronRunNowPayload {
            tenant_id: Uuid::new_v4(),
            cron_id: Uuid::new_v4(),
            command: "echo hello-cron".to_string(),
        })
        .await
        .expect("echo should succeed");
        assert!(ok.contains("hello-cron"));

        let err = execute_cron_run_now(&CronRunNowPayload {
            tenant_id: Uuid::new_v4(),
            cron_id: Uuid::new_v4(),
            command: "echo boom >&2; exit 2".to_string(),
        })
        .await
        .expect_err("non-zero should fail");
        assert!(err.contains("exited with 2"));
        assert!(err.contains("boom"));
    }
}
