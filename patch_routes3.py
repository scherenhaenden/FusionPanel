import re

with open("integration/containers/src/routes.rs", "r") as f:
    content = f.read()

old_validate = """    let suffix = &container_id[expected_prefix.len()..];
    if uuid::Uuid::parse_str(suffix).is_err() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }"""

new_validate = """    let suffix = &container_id[expected_prefix.len()..];
    if uuid::Uuid::parse_str(suffix).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid container ID format".to_string(),
            }),
        ));
    }"""

content = content.replace(old_validate, new_validate)

with open("integration/containers/src/routes.rs", "w") as f:
    f.write(content)
