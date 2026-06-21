use clap::Parser;

use crate::project::HttpMethod;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Tui,
    Request { method: HttpMethod, url: String },
}

#[derive(Debug, Parser)]
#[command(name = "rata")]
pub struct Args {
    #[arg(long, short = 'X', default_value = "GET")]
    method: String,
    url: Option<String>,
}

pub fn parse_from<I, T>(args: I) -> anyhow::Result<Command>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::parse_from(args);
    let method = args.method.parse()?;

    Ok(match args.url {
        Some(url) => Command::Request { method, url },
        None => Command::Tui,
    })
}
