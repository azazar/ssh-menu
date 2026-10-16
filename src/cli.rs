use std::{
    ffi::OsString,
    io::{self, IsTerminal},
    path::PathBuf,
};

pub const HELP: &str = "Shui (Shell UI) — searchable SSH connections and remote tools

Usage: shui [--no-terminal] [QUERY]
       shui --dump
       shui --check-config [FILE]
       shui --config-path

  QUERY                 Start with a host or search phrase (quote multiple words)
  --no-terminal         Require an existing terminal; never open a desktop terminal
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
    Menu { query: String, auto_terminal: bool },
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> io::Result<Action> {
    let args: Vec<_> = args.into_iter().collect();
    let (auto_terminal, args) = if args.first().is_some_and(|arg| arg == "--no-terminal") {
        (false, &args[1..])
    } else {
        (true, args.as_slice())
    };
    let menu = |query| Action::Menu {
        query,
        auto_terminal,
    };
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments; use --help");
    match args {
        [] => Ok(menu(String::new())),
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
            .map(menu)
            .ok_or_else(invalid),
        [query] => query
            .to_str()
            .filter(|s| !s.starts_with('-') && !s.chars().any(char::is_control))
            .map(str::to_owned)
            .map(menu)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

pub fn require_terminal(auto_terminal: bool) -> io::Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    if auto_terminal
        && ["DISPLAY", "WAYLAND_DISPLAY"]
            .iter()
            .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
    {
        return open_terminal();
    }
    #[cfg(not(target_os = "linux"))]
    let _ = auto_terminal;
    Err(io::Error::other(
        "interactive mode requires a terminal; use --dump to list hosts or --help for options",
    ))
}

#[cfg(target_os = "linux")]
fn open_terminal() -> io::Result<()> {
    use std::{
        env,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
    };

    let executable = env::current_exe()?;
    let directory = env::current_dir()?;
    let args: Vec<_> = env::args_os().skip(1).collect();
    let mut failure = None;
    for (launcher, separator) in [
        ("xdg-terminal-exec", "--"),
        ("sensible-terminal", "-e"),
        ("x-terminal-emulator", "-e"),
    ] {
        // Server-based terminals may use their own cwd/environment. Carry the
        // directory and recursion guard in argv, with no user text in shell code.
        let error = Command::new(launcher)
            .arg(separator)
            .args([
                "/bin/sh",
                "-c",
                "cd -- \"$1\" && shift && exec \"$@\"",
                "shui-terminal",
            ])
            .arg(&directory)
            .arg(&executable)
            .arg("--no-terminal")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .exec();
        // Once a launcher starts, its exit status belongs to that attempt. Do
        // not retry a failed session in a second window.
        if error.kind() != io::ErrorKind::NotFound {
            failure = Some(format!("{launcher}: {error}"));
        }
    }
    Err(io::Error::other(failure.map_or_else(
        || "no desktop terminal launcher found; install xdg-terminal-exec, sensible-terminal or x-terminal-emulator, or run Shui in a terminal".to_owned(),
        |error| format!("could not open a desktop terminal: {error}"),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_search_and_read_only_commands_and_rejects_ambiguous_options() {
        let parse = |args: &[&str]| super::parse(args.iter().map(OsString::from));
        assert_eq!(
            parse(&["prod database"]).unwrap(),
            Action::Menu {
                query: "prod database".into(),
                auto_terminal: true
            }
        );
        assert_eq!(
            parse(&["--", "-prod"]).unwrap(),
            Action::Menu {
                query: "-prod".into(),
                auto_terminal: true
            }
        );
        assert_eq!(
            parse(&["--check-config", "custom.yml"]).unwrap(),
            Action::CheckConfig(Some("custom.yml".into()))
        );
        assert!(parse(&["--unknown"]).is_err());
        assert!(parse(&["--dump", "--version"]).is_err());
        assert!(parse(&["prod\nother"]).is_err());
        assert_eq!(
            parse(&["--no-terminal", "--", "-prod"]).unwrap(),
            Action::Menu {
                query: "-prod".into(),
                auto_terminal: false
            }
        );
        assert_eq!(
            parse(&["--", "--no-terminal"]).unwrap(),
            Action::Menu {
                query: "--no-terminal".into(),
                auto_terminal: true
            }
        );
    }
}
