#![forbid(unsafe_code)]

use hosting_e2e::{block_if_missing, get_temp_dir};
use std::process::Command;
use std::time::Duration;

struct ProcessGuard(std::process::Child);

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_nginx(config_path: &std::path::Path) -> ProcessGuard {
    let child = Command::new("nginx")
        .arg("-c")
        .arg(config_path)
        .arg("-g")
        .arg("daemon off;")
        .spawn()
        .expect("Failed to start Nginx");
    ProcessGuard(child)
}

fn start_php_fpm(config_path: &std::path::Path) -> ProcessGuard {
    let child = Command::new("php-fpm")
        .arg("-y")
        .arg(config_path)
        .arg("-F")
        .spawn()
        .expect("Failed to start PHP-FPM");
    ProcessGuard(child)
}

#[test]
fn test_host_04_nginx_and_virtual_hosts() {
    block_if_missing("nginx");

    let dir = get_temp_dir();
    let nginx_conf = dir.path().join("nginx.conf");
    let logs_dir = dir.path().join("logs");
    let pid_file = dir.path().join("nginx.pid");
    std::fs::create_dir_all(&logs_dir).unwrap();

    let config = format!(
        r#"
        pid {};
        events {{ worker_connections 1024; }}
        http {{
            access_log {}/access.log;
            error_log {}/error.log;
            server {{
                listen 18080;
                server_name test.local;
                location / {{
                    return 200 "OK from Virtual Host";
                }}
            }}
        }}
    "#,
        pid_file.display(),
        logs_dir.display(),
        logs_dir.display()
    );
    std::fs::write(&nginx_conf, config).unwrap();

    let _child = start_nginx(&nginx_conf);
    std::thread::sleep(Duration::from_millis(500));

    let client = reqwest::blocking::Client::builder()
        .resolve(
            "test.local",
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                18080,
            ),
        )
        .build()
        .unwrap();

    let resp = client.get("http://test.local:18080/").send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "OK from Virtual Host");
}

#[test]
fn test_host_08_redirects_and_hsts() {
    block_if_missing("nginx");

    let dir = get_temp_dir();
    let nginx_conf = dir.path().join("nginx.conf");
    let logs_dir = dir.path().join("logs");
    let pid_file = dir.path().join("nginx.pid");
    std::fs::create_dir_all(&logs_dir).unwrap();

    let config = format!(
        r#"
        pid {};
        events {{ worker_connections 1024; }}
        http {{
            access_log {}/access.log;
            error_log {}/error.log;
            server {{
                listen 18081;
                server_name test-redirect.local;
                return 301 http://test-redirect.local:18082/;
            }}
            server {{
                listen 18082;
                server_name test-redirect.local;
                add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
                location / {{
                    return 200 "Target OK";
                }}
            }}
        }}
    "#,
        pid_file.display(),
        logs_dir.display(),
        logs_dir.display()
    );
    std::fs::write(&nginx_conf, config).unwrap();

    let _child = start_nginx(&nginx_conf);
    std::thread::sleep(Duration::from_millis(500));

    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve(
            "test-redirect.local",
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                18081,
            ),
        )
        .resolve(
            "test-redirect.local",
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                18082,
            ),
        )
        .build()
        .unwrap();

    let resp = client
        .get("http://test-redirect.local:18081/")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 301);
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "http://test-redirect.local:18082/"
    );

    let resp_target = client
        .get("http://test-redirect.local:18082/")
        .send()
        .unwrap();
    assert_eq!(resp_target.status(), 200);
    assert!(resp_target
        .headers()
        .contains_key("strict-transport-security"));
    assert_eq!(resp_target.text().unwrap(), "Target OK");
}

#[test]
fn test_host_01_php_sites() {
    block_if_missing("nginx");
    block_if_missing("php-fpm");

    let dir = get_temp_dir();
    let nginx_conf = dir.path().join("nginx.conf");
    let fpm_conf = dir.path().join("php-fpm.conf");
    let logs_dir = dir.path().join("logs");
    let pid_file_nginx = dir.path().join("nginx.pid");
    let pid_file_fpm = dir.path().join("php-fpm.pid");
    let socket_file = dir.path().join("php-fpm.sock");
    let web_root = dir.path().join("html");

    std::fs::create_dir_all(&logs_dir).unwrap();
    std::fs::create_dir_all(&web_root).unwrap();

    std::fs::write(web_root.join("index.php"), "<?php echo 'PHP is alive'; ?>").unwrap();

    let fpm_config = format!(
        r#"
[global]
pid = {}
error_log = {}/fpm-error.log
daemonize = no

[www]
listen = {}
pm = static
pm.max_children = 2
"#,
        pid_file_fpm.display(),
        logs_dir.display(),
        socket_file.display()
    );
    std::fs::write(&fpm_conf, fpm_config).unwrap();

    let nginx_config = format!(
        r#"
        pid {};
        events {{ worker_connections 1024; }}
        http {{
            access_log {}/access.log;
            error_log {}/error.log;
            server {{
                listen 18083;
                server_name php.local;
                root {};
                index index.php;

                location / {{
                    try_files $uri $uri/ =404;
                }}

                location ~ \.php$ {{
                    fastcgi_pass unix:{};
                    fastcgi_index index.php;
                    include fastcgi_params;
                    fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
                }}
            }}
        }}
    "#,
        pid_file_nginx.display(),
        logs_dir.display(),
        logs_dir.display(),
        web_root.display(),
        socket_file.display()
    );
    std::fs::write(&nginx_conf, nginx_config).unwrap();

    let fastcgi_params = r#"
fastcgi_param  QUERY_STRING       $query_string;
fastcgi_param  REQUEST_METHOD     $request_method;
fastcgi_param  CONTENT_TYPE       $content_type;
fastcgi_param  CONTENT_LENGTH     $content_length;
fastcgi_param  SCRIPT_NAME        $fastcgi_script_name;
fastcgi_param  REQUEST_URI        $request_uri;
fastcgi_param  DOCUMENT_URI       $document_uri;
fastcgi_param  DOCUMENT_ROOT      $document_root;
fastcgi_param  SERVER_PROTOCOL    $server_protocol;
fastcgi_param  REQUEST_SCHEME     $scheme;
fastcgi_param  HTTPS              $https if_not_empty;
fastcgi_param  GATEWAY_INTERFACE  CGI/1.1;
fastcgi_param  SERVER_SOFTWARE    nginx/$nginx_version;
fastcgi_param  REMOTE_ADDR        $remote_addr;
fastcgi_param  REMOTE_PORT        $remote_port;
fastcgi_param  SERVER_ADDR        $server_addr;
fastcgi_param  SERVER_PORT        $server_port;
fastcgi_param  SERVER_NAME        $server_name;
"#;
    std::fs::write(dir.path().join("fastcgi_params"), fastcgi_params).unwrap();

    let _child_fpm = start_php_fpm(&fpm_conf);
    std::thread::sleep(Duration::from_millis(500));

    let _child_nginx = start_nginx(&nginx_conf);
    std::thread::sleep(Duration::from_millis(500));

    let client = reqwest::blocking::Client::builder()
        .resolve(
            "php.local",
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                18083,
            ),
        )
        .build()
        .unwrap();

    let resp = client.get("http://php.local:18083/").send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "PHP is alive");
}

// Proxy tests:
#[test]
fn test_host_03_nodejs_python_hosting() {
    block_if_missing("nginx");
    block_if_missing("node");

    let dir = get_temp_dir();
    let nginx_conf = dir.path().join("nginx.conf");
    let logs_dir = dir.path().join("logs");
    let pid_file = dir.path().join("nginx.pid");
    let node_app = dir.path().join("app.js");
    std::fs::create_dir_all(&logs_dir).unwrap();

    std::fs::write(
        &node_app,
        r#"
        const http = require('http');
        const server = http.createServer((req, res) => {
            res.writeHead(200, { 'Content-Type': 'text/plain' });
            res.end('Proxy OK from Node\n');
        });
        server.listen(18085, '127.0.0.1');
    "#,
    )
    .unwrap();

    let child_node = Command::new("node")
        .arg(&node_app)
        .spawn()
        .expect("Failed to start node");
    let _node_guard = ProcessGuard(child_node);

    let config = format!(
        r#"
        pid {};
        events {{ worker_connections 1024; }}
        http {{
            access_log {}/access.log;
            error_log {}/error.log;
            server {{
                listen 18084;
                server_name node.local;
                location / {{
                    proxy_pass http://127.0.0.1:18085;
                }}
            }}
        }}
    "#,
        pid_file.display(),
        logs_dir.display(),
        logs_dir.display()
    );
    std::fs::write(&nginx_conf, config).unwrap();

    let _child_nginx = start_nginx(&nginx_conf);
    std::thread::sleep(Duration::from_millis(500));

    let client = reqwest::blocking::Client::builder()
        .resolve(
            "node.local",
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                18084,
            ),
        )
        .build()
        .unwrap();

    let resp = client.get("http://node.local:18084/").send().unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().unwrap(), "Proxy OK from Node\n");
}

#[test]
fn test_host_02_static_sites() {
    block_if_missing("nginx");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_05_per_site_php_fpm_pools() {
    block_if_missing("nginx");
    block_if_missing("php-fpm");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_06_lets_encrypt_tls() {
    block_if_missing("certbot");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_07_wildcard_certificates() {
    block_if_missing("certbot");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_09_fastcgi_cache() {
    block_if_missing("nginx");
    block_if_missing("php-fpm");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_10_redis_cache() {
    block_if_missing("redis-server");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_11_file_manager() {
    block_if_missing("nginx");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_12_cron_jobs() {
    block_if_missing("cron");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_13_disk_quotas() {
    block_if_missing("setquota");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_14_bandwidth_limits() {
    block_if_missing("tc");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_15_atomic_provisioning_with_compensation() {
    block_if_missing("nginx");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_16_image_optimization() {
    block_if_missing("optipng");
    panic!("BLOCKED: missing logic");
}

#[test]
fn test_host_17_cdn_integration() {
    block_if_missing("nginx");
    panic!("BLOCKED: missing logic");
}
