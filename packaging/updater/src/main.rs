#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;
use updater::{run_update, UpdateConfig};

fn print_usage(program_name: &str) {
    eprintln!(
        "Usage: {} <binary_url> <signature_url> <public_key_hex> <target_path> <health_check_url> <health_check_timeout_secs>",
        program_name
    );
}

fn parse_hex_key(hex_str: &str) -> Result<[u8; 32], String> {
    if hex_str.len() != 64 {
        return Err("Public key must be exactly 64 hexadecimal characters (32 bytes)".to_string());
    }

    let mut key = [0u8; 32];
    for i in 0..32 {
        let byte_str = &hex_str[i * 2..i * 2 + 2];
        key[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| format!("Invalid hexadecimal character in public key: {}", byte_str))?;
    }
    Ok(key)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 7 {
        print_usage(&args[0]);
        exit(1);
    }

    let binary_url = args[1].clone();
    let signature_url = args[2].clone();

    let public_key = match parse_hex_key(&args[3]) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Error parsing public key: {}", e);
            exit(1);
        }
    };

    let target_path = PathBuf::from(&args[4]);
    let health_check_url = args[5].clone();

    let timeout_secs = match args[6].parse::<u64>() {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error parsing health check timeout seconds");
            exit(1);
        }
    };
    let health_check_timeout = Duration::from_secs(timeout_secs);

    let config = UpdateConfig {
        binary_url,
        signature_url,
        public_key,
        target_path,
        health_check_url,
        health_check_timeout,
    };

    match run_update(&config) {
        Ok(child) => {
            println!(
                "Update successful. Candidate started with PID {}",
                child.id()
            );

            // In a real scenario we might daemonize or detach, but for this updater wrapper,
            // if we just exit, the child might get orphaned or we might want to wait.
            // If the updater is meant to replace itself or just spawn the new candidate and exit,
            // we will just exit with success. The `run_update` function spawns the child.
            // If we drop the child, it keeps running. But wait, `run_update` returns `Child`
            // and if we drop `child` in Rust, does it kill it? No, in Rust dropping a `Child`
            // does NOT kill the process, it just closes the handle.
            exit(0);
        }
        Err(e) => {
            eprintln!("Update failed: {}", e);
            exit(1);
        }
    }
}
