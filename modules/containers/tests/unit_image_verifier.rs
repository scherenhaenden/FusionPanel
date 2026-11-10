#![forbid(unsafe_code)]

use containers::image_verifier::{ImagePolicyVerifier, VerifyError};

#[test]
fn test_valid_digest_image() {
    let verifier =
        ImagePolicyVerifier::new(true, vec!["ghcr.io".to_string(), "docker.io".to_string()]);

    // Valid digest pin on docker.io
    assert_eq!(
        verifier.verify("docker.io/library/nginx@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Ok(())
    );

    // Valid digest pin on ghcr.io
    assert_eq!(
        verifier.verify("ghcr.io/org/repo@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Ok(())
    );

    // With tag AND digest
    assert_eq!(
        verifier.verify("ghcr.io/org/repo:latest@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Ok(())
    );
}

#[test]
fn test_implicit_registry() {
    let verifier = ImagePolicyVerifier::new(true, vec!["docker.io".to_string()]);

    // Implicit docker.io with library
    assert_eq!(
        verifier.verify(
            "nginx@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"
        ),
        Ok(())
    );

    // Implicit docker.io with namespace
    assert_eq!(
        verifier.verify(
            "bitnami/nginx@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"
        ),
        Ok(())
    );
}

#[test]
fn test_registry_with_port() {
    let verifier = ImagePolicyVerifier::new(true, vec!["localhost:5000".to_string()]);

    assert_eq!(
        verifier.verify("localhost:5000/my-app@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Ok(())
    );
}

#[test]
fn test_floating_tag_rejection() {
    let verifier = ImagePolicyVerifier::new(true, vec!["docker.io".to_string()]);

    // Missing digest
    assert_eq!(
        verifier.verify("docker.io/library/nginx:latest"),
        Err(VerifyError::FloatingTagNotAllowed)
    );

    // Missing tag and digest
    assert_eq!(
        verifier.verify("nginx"),
        Err(VerifyError::FloatingTagNotAllowed)
    );
}

#[test]
fn test_digest_pinning_not_required() {
    let verifier = ImagePolicyVerifier::new(false, vec!["docker.io".to_string()]);

    assert_eq!(verifier.verify("docker.io/library/nginx:latest"), Ok(()));

    assert_eq!(verifier.verify("nginx"), Ok(()));
}

#[test]
fn test_registry_allowlist_matching() {
    let verifier = ImagePolicyVerifier::new(true, vec!["ghcr.io".to_string()]);

    // docker.io is not in the allowlist
    assert_eq!(
        verifier.verify("docker.io/library/nginx@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Err(VerifyError::RegistryNotAllowed("docker.io".to_string()))
    );

    // implicit docker.io is not in the allowlist
    assert_eq!(
        verifier.verify(
            "nginx@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"
        ),
        Err(VerifyError::RegistryNotAllowed("docker.io".to_string()))
    );

    // empty allowlist means all allowed
    let any_verifier = ImagePolicyVerifier::new(true, vec![]);
    assert_eq!(
        any_verifier.verify("evil-registry.com/hacker/malware@sha256:45b235646eb836603a1168434ce3c37b313efbd05e1974ef4b790d939c368d44"),
        Ok(())
    );
}

#[test]
fn test_illegal_character_handling() {
    let verifier = ImagePolicyVerifier::new(false, vec![]);

    let illegal_strings = vec![
        "nginx:latest\n",
        "nginx:latest;",
        "nginx:latest&echo 1",
        "nginx:latest|cat",
        "nginx:latest`ls`",
        "nginx:latest$HOME",
        "nginx:latest\\",
        "nginx:latest<",
        "nginx:latest>",
        "nginx:latest ",
        "nginx:latest\"",
        "nginx:latest'",
        "nginx:latest\0",
        "nginx:latest\t",
        "nginx:latest\r",
    ];

    for img in illegal_strings {
        assert_eq!(
            verifier.verify(img),
            Err(VerifyError::IllegalCharacters),
            "Failed to reject illegal string: {}",
            img
        );
    }
}

#[test]
fn test_malformed_reference() {
    let verifier = ImagePolicyVerifier::new(true, vec![]);

    // Invalid digest format
    assert_eq!(
        verifier.verify("nginx@md5:123"),
        Err(VerifyError::MalformedReference)
    );
}
