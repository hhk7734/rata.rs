use std::io::Read;

use rata::{Command, RataProject, parse_from};

fn main() -> anyhow::Result<()> {
    match parse_from(std::env::args_os())? {
        Command::Tui => {
            let cwd = std::env::current_dir()?;
            let project = RataProject::discover(cwd)?;
            rata::tui::run(project.as_ref())
        }
        Command::Request { method, url } => {
            run_request(method, &url)?;
            Ok(())
        }
    }
}

fn run_request(method: rata::HttpMethod, url: &str) -> anyhow::Result<()> {
    let project = RataProject::discover(std::env::current_dir()?)?;
    if let Some(project) = &project {
        match project.match_url(method, url)? {
            Some(matched) => eprintln!(
                "Matched OpenAPI: {} {} - {}",
                method.label(),
                matched.operation.path,
                matched.operation.summary
            ),
            None => eprintln!(
                "No matching OpenAPI operation found for {} {}",
                method.label(),
                url
            ),
        }
    }

    let mut variables = std::collections::HashMap::new();
    if let Some(project) = &project {
        variables = project.variables();
        if !variables.contains_key("baseUrl") {
            if let Some(server) = project.server_url() {
                variables.insert("baseUrl".to_string(), server.trim_end_matches('/').to_string());
            }
        }
    }
    let final_url = rata::render(url, &variables);

    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("rata/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut request = client.request(method.reqwest(), &final_url);
    if let Some(project) = &project {
        for (k, v) in project.global_headers() {
            let final_value = rata::render(&v, &variables);
            request = request.header(k, final_value);
        }
    }
    let mut response = request.send()?;
    println!("HTTP {}", response.status());
    for (name, value) in response.headers() {
        println!("{}: {}", name, value.to_str().unwrap_or("<binary>"));
    }
    println!();

    let mut body = String::new();
    response.read_to_string(&mut body)?;
    print!("{body}");

    Ok(())
}
