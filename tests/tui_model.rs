use std::fs;

use rata::RataProject;
use rata::tui::{Theme, build_model};
use tempfile::tempdir;

#[test]
fn builds_dark_postman_style_model_from_openapi_project() {
    let tmp = tempdir().unwrap();
    let rata_dir = tmp.path().join(".rata");
    fs::create_dir_all(rata_dir.join("users/{id}/get")).unwrap();
    fs::write(
        rata_dir.join("openapi.yaml"),
        r#"
openapi: 3.0.3
info:
  title: Example API
  version: 1.0.0
servers:
  - url: https://api.example.com
paths:
  /users/{id}:
    get:
      tags: [users]
      operationId: getUser
      summary: Get user by ID
      responses:
        "200":
          description: User found
  /users:
    post:
      tags: [users]
      operationId: createUser
      summary: Create user
      responses:
        "201":
          description: User created
"#,
    )
    .unwrap();
    fs::write(
        rata_dir.join("users/{id}/get/success.yaml"),
        "response:\n  status: 200\n",
    )
    .unwrap();
    let project = RataProject::discover(tmp.path()).unwrap().unwrap();

    let model = build_model(Some(&project));

    assert_eq!(model.theme, Theme::Dark);
    assert_eq!(model.collections_title, "Collections");

    assert_eq!(
        model.selected_request_url,
        "https://api.example.com/users/{id}"
    );
    assert_eq!(model.examples, vec!["success.yaml".to_string()]);
}
