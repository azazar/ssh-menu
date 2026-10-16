use std::{ffi::OsString, io, path::PathBuf};

pub const HELP: &str = "Shui (Shell UI) — searchable SSH connections and remote tools

Usage: shui [QUERY]
       shui --dump
       shui --check-config [FILE]
       shui --config-path

  QUERY                 Start with a host or search phrase (quote multiple words)
  --dump                List discovered hosts without running configuration commands
  --check-config [FILE] Validate configuration without executing its commands
  --config-path         Print the active configuration path
  -h, --help            Show this help
  -V, --version         Show the version

Config: $XDG_CONFIG_HOME/shui/config.yml (default: ~/.config/shui/config.yml)
Hosts: ~/.ssh/config and included files, ~/.bash_history, and MC hotlist
In the menu: Enter connects; Alt+Enter returns afterward; F1 shows help.
Use -- before a query that starts with a dash.";

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Help,
    Version,
    Dump,
    ConfigPath,
    CheckConfig(Option<PathBuf>),
    Menu(String),
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> io::Result<Action> {
    let args: Vec<_> = args.into_iter().collect();
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments; use --help");
    match args.as_slice() {
        [] => Ok(Action::Menu(String::new())),
        [arg] if arg == "--help" || arg == "-h" => Ok(Action::Help),
        [arg] if arg == "--version" || arg == "-V" => Ok(Action::Version),
        [arg] if arg == "--dump" => Ok(Action::Dump),
        [arg] if arg == "--config-path" => Ok(Action::ConfigPath),
        [arg] if arg == "--check-config" => Ok(Action::CheckConfig(None)),
        [arg, path] if arg == "--check-config" => Ok(Action::CheckConfig(Some(path.into()))),
        [arg, query] if arg == "--" => query
            .to_str()
            .filter(|s| !s.chars().any(char::is_control))
            .map(str::to_owned)
            .map(Action::Menu)
            .ok_or_else(invalid),
        [query] => query
            .to_str()
            .filter(|s| !s.starts_with('-') && !s.chars().any(char::is_control))
            .map(str::to_owned)
            .map(Action::Menu)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_search_and_read_only_commands_and_rejects_ambiguous_options() {
        let parse = |args: &[&str]| super::parse(args.iter().map(OsString::from));
        assert_eq!(
            parse(&["prod database"]).unwrap(),
            Action::Menu("prod database".into())
        );
        assert_eq!(
            parse(&["--", "-prod"]).unwrap(),
            Action::Menu("-prod".into())
        );
        assert_eq!(
            parse(&["--check-config", "custom.yml"]).unwrap(),
            Action::CheckConfig(Some("custom.yml".into()))
        );
        assert!(parse(&["--unknown"]).is_err());
        assert!(parse(&["--dump", "--version"]).is_err());
        assert!(parse(&["prod\nother"]).is_err());
    }
}
