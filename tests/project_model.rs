use std::fs;

use rata::{HttpMethod, RataProject};
use tempfile::tempdir;

#[test]
fn discovers_openapi_collections_matches_urls_and_finds_examples() {
    let tmp = tempdir().unwrap();
    let rata_dir = tmp.path().join(".rata");
    fs::create_dir_all(rata_dir.join("examples/users/{id}/get")).unwrap();
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
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: User found
    delete:
      tags: [users]
      operationId: deleteUser
      summary: Delete user
      responses:
        "204":
          description: User deleted
  /users:
    get:
      tags: [users]
      operationId: listUsers
      summary: List users
      responses:
        "200":
          description: Users listed
    post:
      tags: [users]
      operationId: createUser
      summary: Create user
      responses:
        "201":
          description: User created
  /organizations:
    get:
      tags: [organizations]
      operationId: listOrganizations
      summary: List organizations
      responses:
        "200":
          description: Organizations listed
"#,
    )
    .unwrap();
    fs::write(
        rata_dir.join("examples/users/{id}/get/success.yaml"),
        r#"params:
  id: "42"
response:
  status: 200
"#,
    )
    .unwrap();

    let project = RataProject::discover(tmp.path()).unwrap().unwrap();

    assert_eq!(project.openapi_path(), rata_dir.join("openapi.yaml"));
    assert_eq!(project.collections()[0].name, "users");
    assert_eq!(project.collections()[0].operations.len(), 4);
    assert_eq!(
        project.collections()[0].operations[0].method,
        HttpMethod::Get
    );
    assert_eq!(
        project.collections()[0].operations[0].summary,
        "Get user by ID"
    );
    assert_eq!(project.collections()[1].name, "organizations");

    let matched = project
        .match_url(HttpMethod::Get, "https://api.example.com/users/42")
        .unwrap()
        .unwrap();
    assert_eq!(matched.operation.path, "/users/{id}");
    assert_eq!(
        matched.path_params,
        vec![("id".to_string(), "42".to_string())]
    );

    let examples = project.examples_for(&matched.operation).unwrap();
    assert_eq!(examples.len(), 1);
    assert_eq!(examples[0].name, "success.yaml");
    assert_eq!(
        examples[0].path,
        rata_dir.join("examples/users/{id}/get/success.yaml")
    );
}
