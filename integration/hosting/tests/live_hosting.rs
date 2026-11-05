use hosting::config::{generate_nginx_config, generate_php_fpm_config};
use hosting::domain::{DomainName, HostingSpec, PhpConfig, SiteConfig, SiteId, TenantId};
use std::convert::TryInto;
use std::fs;
use std::process::Command;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[tokio::test]
async fn test_live_hosting_e2e() {
    let docker_check = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("alpine")
        .arg("echo")
        .arg("hello")
        .output();

    if docker_check.is_err() || !docker_check.unwrap().status.success() {
        panic!("BLOCKED: Docker is not available or cannot run basic containers.");
    }

    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let site_id = SiteId(Uuid::new_v4());
    let tenant_id = TenantId(Uuid::new_v4());
    let domain = DomainName::new("example.com").unwrap();
    let run_as_user: hosting::domain::OsUsername = "testuser".to_string().try_into().unwrap();
    let site = SiteConfig::new(
        site_id,
        tenant_id.clone(),
        domain.clone(),
        format!("/srv/fusionpanel/sites/{}", domain.as_str()),
        HostingSpec::Php(PhpConfig {
            version: "8.2".to_string().try_into().unwrap(),
            pool_name: "example_pool".to_string().try_into().unwrap(),
            memory_limit: "256M".to_string().try_into().unwrap(),
        }),
        port,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        run_as_user.clone(),
        vec![],
    )
    .unwrap();

    let php_config = generate_php_fpm_config(&site).unwrap();
    let nginx_config = generate_nginx_config(&site).unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Nginx config dir
    let nginx_conf_dir = temp_path.join("nginx_conf");
    fs::create_dir_all(&nginx_conf_dir).unwrap();
    let nginx_conf_path = nginx_conf_dir.join("default.conf");
    fs::write(&nginx_conf_path, &nginx_config).unwrap();

    // PHP-FPM config dir
    let php_conf_dir = temp_path.join("php_conf");
    fs::create_dir_all(&php_conf_dir).unwrap();
    let php_conf_path = php_conf_dir.join("pool.conf");
    fs::write(&php_conf_path, &php_config).unwrap();

    // Site root
    let site_root = temp_path.join("site_root");
    fs::create_dir_all(&site_root).unwrap();
    fs::write(
        site_root.join("index.php"),
        "<?php echo 'Hello from PHP-FPM'; ?>",
    )
    .unwrap();

    // Shared run directory for the unix socket
    let run_dir = temp_path.join("run_php");
    fs::create_dir_all(&run_dir).unwrap();
    assert!(Command::new("chmod")
        .arg("777")
        .arg(&run_dir)
        .status()
        .unwrap()
        .success());

    // Create Docker network
    let network_name = format!("test_net_{}", Uuid::new_v4());
    assert!(Command::new("docker")
        .args(["network", "create", &network_name])
        .output()
        .expect("Failed to create network")
        .status
        .success());

    // PHP container script
    let run_as_user: hosting::domain::OsUsername = "testuser".to_string().try_into().unwrap();
    let user_name = run_as_user.as_str().to_string();
    let php_entrypoint = temp_path.join("php_entrypoint.sh");
    fs::write(
        &php_entrypoint,
        format!(
            "#!/bin/sh\n\
        addgroup -S {user} || true\n\
        adduser -S -G {user} {user} || true\n\
        exec php-fpm -F\n",
            user = user_name
        ),
    )
    .unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&php_entrypoint)
        .status()
        .unwrap()
        .success());

    let php_container_name = format!("test_php_{}", Uuid::new_v4());
    let mut php_cmd = Command::new("docker");
    php_cmd.args([
        "run",
        "-d",
        "--rm",
        "--name",
        &php_container_name,
        "--network",
        &network_name,
        "-v",
        &format!(
            "{}:/usr/local/etc/php-fpm.d/zz-pool.conf:ro",
            php_conf_path.display()
        ),
        "-v",
        &format!("{}:/srv/fusionpanel/sites/example.com", site_root.display()),
        "-v",
        &format!("{}:/run/php", run_dir.display()),
        "-v",
        &format!("{}:/entrypoint.sh:ro", php_entrypoint.display()),
        "--entrypoint",
        "/entrypoint.sh",
        "php:8.2-fpm-alpine",
    ]);
    let php_output = php_cmd.output().unwrap();
    assert!(
        php_output.status.success(),
        "Failed to start PHP container: {}",
        String::from_utf8_lossy(&php_output.stderr)
    );

    // Nginx container script
    let nginx_entrypoint = temp_path.join("nginx_entrypoint.sh");
    fs::write(
        &nginx_entrypoint,
        format!(
            "#!/bin/sh\n\
        addgroup -S {user} || true\n\
        adduser nginx {user} || true\n\
        exec nginx -g 'daemon off;'\n",
            user = user_name
        ),
    )
    .unwrap();
    assert!(Command::new("chmod")
        .arg("+x")
        .arg(&nginx_entrypoint)
        .status()
        .unwrap()
        .success());

    let nginx_container_name = format!("test_nginx_{}", Uuid::new_v4());
    let mut nginx_cmd = Command::new("docker");
    nginx_cmd.args([
        "run",
        "-d",
        "--rm",
        "--name",
        &nginx_container_name,
        "--network",
        &network_name,
        "-p",
        &format!("{}:{}", port, port),
        "-v",
        &format!(
            "{}:/etc/nginx/conf.d/default.conf:ro",
            nginx_conf_path.display()
        ),
        "-v",
        &format!(
            "{}:/srv/fusionpanel/sites/example.com:ro",
            site_root.display()
        ),
        "-v",
        &format!("{}:/run/php", run_dir.display()),
        "-v",
        &format!("{}:/entrypoint.sh:ro", nginx_entrypoint.display()),
        "--entrypoint",
        "/entrypoint.sh",
        "nginx:alpine",
    ]);
    let nginx_output = nginx_cmd.output().unwrap();
    assert!(
        nginx_output.status.success(),
        "Failed to start Nginx container: {}",
        String::from_utf8_lossy(&nginx_output.stderr)
    );

    // Wait for services to be ready
    sleep(Duration::from_secs(3)).await;

    // 2. Test End-to-End Request
    let client = reqwest::Client::new();
    let res = client
        .get(&format!("http://localhost:{}/", port))
        .header("Host", "example.com")
        .send()
        .await
        .expect("HTTP Request failed completely (e.g. connection refused)");

    assert_eq!(
        res.status().as_u16(),
        200,
        "Expected HTTP 200 on initial request"
    );
    let body = res.text().await.unwrap();
    assert_eq!(body, "Hello from PHP-FPM", "Expected PHP-FPM output");

    // 3. Test Reloading PHP-FPM
    fs::write(site_root.join("index.php"), "<?php echo 'Reloaded PHP'; ?>").unwrap();

    let kill_res = Command::new("docker")
        .args(["exec", &php_container_name, "kill", "-USR2", "1"])
        .output()
        .unwrap();
    assert!(kill_res.status.success(), "Failed to send USR2 to PHP-FPM");

    sleep(Duration::from_secs(2)).await;

    let res2 = client
        .get(&format!("http://localhost:{}/", port))
        .header("Host", "example.com")
        .send()
        .await
        .expect("HTTP Request failed completely after PHP reload");

    assert_eq!(
        res2.status().as_u16(),
        200,
        "Expected HTTP 200 on reloaded request"
    );
    let body2 = res2.text().await.unwrap();
    assert_eq!(body2, "Reloaded PHP", "Expected updated PHP-FPM output");

    // 1. Test Syntax Error Recovery
    // We update the active config file with a syntax error, run `nginx -t` in the running container
    // and assert it fails (thus simulating error recovery, meaning it wouldn't be applied/reloaded).
    let bad_nginx_conf = "server { invalid syntax; }";
    let bad_conf_path = temp_path.join("bad.conf");
    fs::write(&bad_conf_path, bad_nginx_conf).unwrap();

    // Instead of doing it that way, just test the syntax on a new container or a specific file
    let syntax_check = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!(
                "{}:/etc/nginx/conf.d/default.conf:ro",
                bad_conf_path.display()
            ),
            "nginx:alpine",
            "nginx",
            "-t",
        ])
        .output()
        .unwrap();
    assert!(
        !syntax_check.status.success(),
        "Nginx should fail syntax check for invalid config"
    );

    // Simulate SSL certificate binding
    let ssl_nginx_config = nginx_config.replace(
        &format!("listen {};", port),
        &format!("listen {};\n    listen 443 ssl;\n    ssl_certificate /etc/nginx/cert.pem;\n    ssl_certificate_key /etc/nginx/key.pem;", port),
    );
    let ssl_conf_path = temp_path.join("ssl.conf");
    fs::write(&ssl_conf_path, ssl_nginx_config).unwrap();

    assert!(Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:4096",
            "-keyout",
            &temp_path.join("key.pem").display().to_string(),
            "-out",
            &temp_path.join("cert.pem").display().to_string(),
            "-days",
            "365",
            "-nodes",
            "-subj",
            "/CN=example.com"
        ])
        .output()
        .unwrap()
        .status
        .success());

    let ssl_check = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!(
                "{}:/etc/nginx/conf.d/default.conf:ro",
                ssl_conf_path.display()
            ),
            "-v",
            &format!(
                "{}:/etc/nginx/cert.pem:ro",
                temp_path.join("cert.pem").display()
            ),
            "-v",
            &format!(
                "{}:/etc/nginx/key.pem:ro",
                temp_path.join("key.pem").display()
            ),
            "nginx:alpine",
            "nginx",
            "-t",
        ])
        .output()
        .unwrap();
    assert!(
        ssl_check.status.success(),
        "Nginx should pass syntax check for SSL config: {}",
        String::from_utf8_lossy(&ssl_check.stderr)
    );

    // Cleanup
    let _ = Command::new("docker")
        .args(["rm", "-f", &php_container_name])
        .output();
    let _ = Command::new("docker")
        .args(["rm", "-f", &nginx_container_name])
        .output();
    let _ = Command::new("docker")
        .args(["network", "rm", &network_name])
        .output();
}
