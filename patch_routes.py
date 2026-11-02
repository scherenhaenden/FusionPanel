import re

with open("integration/containers/src/routes.rs", "r") as f:
    content = f.read()

content = content.replace('"/v1/containers/{id}"', '"/v1/containers/:id"')
content = content.replace('"/v1/containers/{id}/start"', '"/v1/containers/:id/start"')
content = content.replace('"/v1/containers/{id}/stop"', '"/v1/containers/:id/stop"')
content = content.replace('"/v1/containers/{id}/restart"', '"/v1/containers/:id/restart"')

old_validate = """fn validate_tenant_access(
    tenant_id: &str,
    container_id: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if !container_id.starts_with(&format!("{}-", tenant_id)) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }
    Ok(())
}"""

new_validate = """fn validate_tenant_access(
    tenant_id: &str,
    container_id: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let expected_prefix = format!("{}-", tenant_id);
    if !container_id.starts_with(&expected_prefix) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }

    let suffix = &container_id[expected_prefix.len()..];
    if uuid::Uuid::parse_str(suffix).is_err() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }

    Ok(())
}"""

content = content.replace(old_validate, new_validate)

with open("integration/containers/src/routes.rs", "w") as f:
    f.write(content)
