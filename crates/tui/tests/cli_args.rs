use rata::{Command, HttpMethod, parse_from};

#[test]
fn no_url_selects_tui_mode() {
    let command = parse_from(["rata"]).unwrap();

    assert_eq!(command, Command::Tui);
}

#[test]
fn url_selects_request_mode_with_get_by_default() {
    let command = parse_from(["rata", "https://api.example.com/users/42"]).unwrap();

    assert_eq!(
        command,
        Command::Request {
            method: HttpMethod::Get,
            url: "https://api.example.com/users/42".to_string(),
        }
    );
}

#[test]
fn method_flag_changes_request_method() {
    let command = parse_from(["rata", "-X", "POST", "https://api.example.com/users"]).unwrap();

    assert_eq!(
        command,
        Command::Request {
            method: HttpMethod::Post,
            url: "https://api.example.com/users".to_string(),
        }
    );
}
