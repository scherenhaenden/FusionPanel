#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;

pub fn run(driver_variable: &str, scenario: &str, requirement: &str) {
    let driver = std::env::var_os(driver_variable).unwrap_or_else(|| {
        panic!(
            "RESULT=BLOCKED; missing {driver_variable}; required: {requirement}; configure it with an executable fixture driver and rerun the named test"
        )
    });
    if !Path::new(&driver).is_absolute() {
        panic!(
            "RESULT=BLOCKED; {driver_variable} must name an absolute executable path; required: {requirement}"
        );
    }

    let output = Command::new(driver)
        .env_clear()
        .env("FUSIONPANEL_NODES_TEST_SCENARIO", scenario)
        .output()
        .unwrap_or_else(|_| {
            panic!("RESULT=BLOCKED; could not execute {driver_variable}; required: {requirement}")
        });

    if !output.status.success() {
        panic!(
            "RESULT=FAIL; scenario={scenario}; fixture driver exited with status {}",
            output.status
        );
    }

    let expected = format!("RESULT=PASS;scenario={scenario}");
    let observed = String::from_utf8_lossy(&output.stdout);
    if observed.trim() != expected {
        panic!(
            "RESULT=FAIL; scenario={scenario}; fixture driver must assert the real user outcome and print exactly {expected:?} on success"
        );
    }
}
