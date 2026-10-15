mod bar;
mod bindings;
mod editor;
mod help;
mod launch;
mod new_remote;
#[cfg(all(test, unix))]
mod preset_tests;
mod search;
mod shortcut;

use crossterm::{
    cursor::Show,
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    style::{Attribute, ResetColor, SetAttribute},
};
use ratatui::{
    DefaultTerminal,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState},
};
use std::{
    collections::HashMap,
    env, fs,
    io::{self, Write},
    path::Path,
    process::Command,
};

// Catppuccin Mocha: https://catppuccin.com/palette/
mod theme {
    use ratatui::style::Color;
    pub const BASE: Color = Color::Rgb(30, 30, 46);
    pub const TEXT: Color = Color::Rgb(205, 214, 244);
    pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);
    pub const SURFACE: Color = Color::Rgb(49, 50, 68);
    pub const SELECTION: Color = Color::Rgb(69, 71, 90);
    pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
    pub const USERNAME: Color = Color::Rgb(249, 226, 175);
    pub const PEACH: Color = Color::Rgb(250, 179, 135);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    host: String,
    port: Option<String>,
}
impl Target {
    fn label(&self) -> String {
        match &self.port {
            Some(p) => format!("{}:{p}", self.host),
            None => self.host.clone(),
        }
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.-@[]:".contains(c))
}
fn target(raw: &str, user: &str) -> Option<Target> {
    let raw = raw.strip_prefix(&format!("{user}@")).unwrap_or(raw);
    let (host, port) = match raw.rsplit_once(':') {
        Some((h, p))
            if !p.is_empty()
                && p.chars().all(|c| c.is_ascii_digit())
                && (!h.contains(':') || h.ends_with(']')) =>
        {
            (h, Some(p.to_owned()))
        }
        _ => (raw, None),
    };
    if !valid_host(host) {
        return None;
    }
    Some(Target {
        host: host.into(),
        port,
    })
}
fn history_target(line: &str, user: &str) -> Option<Target> {
    if let Some(hint) = launch::history_target_hint(line) {
        return target(hint, user);
    }
    let words = launch::history_words(line)?;
    if let [program, path] = words.as_slice()
        && program == "mc"
        && let Some(host) = path
            .strip_prefix("/sh://")
            .and_then(|p| p.strip_suffix('/'))
    {
        return target(host, user);
    }
    history_connection(line, user).map(|(target, _, _)| target)
}

fn history_connection(line: &str, user: &str) -> Option<(Target, Vec<String>, String)> {
    let words = launch::history_words(line)?;
    let mut it = words.iter().map(String::as_str);
    let cmd = it.next()?;
    if !matches!(cmd, "ssh" | "ssh-copy-id" | "mosh") {
        return None;
    }
    let mut port = None;
    let mut login = None;
    let host = loop {
        let word = it.next()?;
        if word == "--" {
            break it.next()?;
        }
        if word == "-p" {
            port = Some(it.next()?.to_owned());
            continue;
        }
        if word.starts_with("-p") && word.len() > 2 {
            port = Some(word[2..].to_owned());
            continue;
        }
        if word == "-l" {
            login = Some(it.next()?);
            continue;
        }
        if word == "--ssh" || word.starts_with("--ssh=") {
            let value = word.strip_prefix("--ssh=").or_else(|| it.next())?;
            port = history_target(&format!("{value} placeholder"), user).and_then(|t| t.port);
            continue;
        }
        if matches!(
            word,
            "-i" | "-F"
                | "-o"
                | "-J"
                | "-L"
                | "-R"
                | "-D"
                | "-S"
                | "-b"
                | "-c"
                | "-E"
                | "-e"
                | "-m"
                | "-O"
                | "-W"
                | "-w"
        ) {
            it.next()?;
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        break word;
    };
    let mut t = target(host, user)?;
    if let Some(p) = port {
        p.parse::<u16>().ok().filter(|p| *p > 0)?;
        t.port = Some(p);
    }
    if let Some(login) = login {
        t = target(&format!("{login}@{}", t.label()), user)?;
    }
    Some((t, it.map(str::to_owned).collect(), cmd.to_owned()))
}
fn discover(home: &Path, user: &str) -> Vec<Target> {
    let mut entries = HashMap::new();
    let mut rank = 0usize;
    let mut add = |t: Target| {
        rank += 1;
        entries.insert(t.label(), (rank, t));
    };
    let config = fs::read_to_string(home.join(".ssh/config")).unwrap_or_default();
    let mut hosts: Vec<String> = Vec::new();
    let mut login = user.to_owned();
    let flush = |hosts: &[String], login: &str, add: &mut dyn FnMut(Target)| {
        for host in hosts {
            if host.contains(['*', '?', '!']) {
                continue;
            }
            if let Some(t) = target(&format!("{login}@{host}"), user) {
                add(t);
            }
            if login != "root"
                && let Some(t) = target(&format!("root@{host}"), user)
            {
                add(t);
            }
        }
    };
    for line in config.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.len() < 2 || words[0].starts_with('#') {
            continue;
        }
        if words[0].eq_ignore_ascii_case("host") {
            flush(&hosts, &login, &mut add);
            hosts = words[1..]
                .iter()
                .take_while(|w| !w.starts_with('#'))
                .map(|w| w.to_string())
                .collect();
            login = user.into();
        } else if words[0].eq_ignore_ascii_case("user") {
            login = words[1].into();
        }
    }
    flush(&hosts, &login, &mut add);
    for line in fs::read_to_string(home.join(".config/mc/hotlist"))
        .unwrap_or_default()
        .lines()
    {
        if let Ok(words) = shell_words::split(line)
            && words.first().map(String::as_str) == Some("ENTRY")
            && let Some(url) = words.windows(2).find(|w| w[0] == "URL").map(|w| &w[1])
            && let Some(rest) = url.strip_prefix("/sh://")
            && let Some(t) = target(rest.split('/').next().unwrap_or(""), user)
        {
            add(t);
        }
    }
    for line in fs::read_to_string(home.join(".bash_history"))
        .unwrap_or_default()
        .lines()
    {
        if let Some(t) = history_target(line, user) {
            add(t);
        }
    }
    let mut values: Vec<_> = entries.into_values().collect();
    values.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
    values.into_iter().map(|(_, t)| t).collect()
}
fn erase_filter_word(query: &mut String) {
    let trimmed = query.trim_end();
    let end = trimmed
        .rfind(char::is_whitespace)
        .map_or(0, |i| i + trimmed[i..].chars().next().unwrap().len_utf8());
    query.truncate(end);
}

struct Selection {
    target: Target,
    profile: usize,
    setup: Option<usize>,
    setup_command: Option<(String, Vec<String>)>,
    new: bool,
    configure_only: bool,
    return_to_menu: bool,
    connection_command: Option<(String, Vec<String>)>,
}

#[derive(Default)]
struct MenuState {
    query: String,
    selection: usize,
    profiles: Vec<usize>,
    notice: Option<String>,
}

fn menu(
    terminal: &mut DefaultTerminal,
    targets: &[Target],
    config: &mut launch::Config,
    state: &mut MenuState,
    home: &Path,
) -> io::Result<Option<Selection>> {
    let MenuState {
        query,
        selection,
        profiles,
        notice,
    } = state;
    let mut bar_state = bar::State::default();
    let host_config = search::host_config(home, &env::var("USER").unwrap_or_default());
    loop {
        let footer_height = 1;
        let visible = search::matching(targets, query, &host_config);
        *selection = (*selection).min(visible.len().saturating_sub(1));
        let profile_index = visible
            .get(*selection)
            .map_or(config.default_index(), |i| profiles[*i]);
        terminal.draw(|frame| {
            frame.render_widget(
                Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
                frame.area(),
            );
            let areas = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(footer_height),
            ])
            .split(frame.area());
            let header = Line::from(vec![
                Span::styled(
                    " SSH MENU ",
                    Style::default()
                        .fg(theme::BASE)
                        .bg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}/{} ", visible.len(), targets.len()),
                    Style::default().fg(theme::SUBTEXT),
                ),
                Span::styled(format!(" / {query}"), Style::default().fg(theme::LAVENDER)),
            ]);
            frame.render_widget(
                Paragraph::new(header).style(Style::default().bg(theme::SURFACE)),
                areas[0],
            );
            let rows: Vec<_> = visible
                .iter()
                .map(|i| {
                    let profile = &config.profiles[profiles[*i]];
                    let mosh = profile.program() == "mosh";
                    let label = targets[*i].label();
                    let (username, host) = label.rsplit_once('@').unwrap_or(("", &label));
                    Row::new(vec![
                        Cell::from(profile.name.clone()).style(Style::default().fg(if mosh {
                            theme::PEACH
                        } else {
                            theme::SUBTEXT
                        })),
                        Cell::from(Line::from(username.to_owned()).alignment(Alignment::Right))
                            .style(
                                Style::default()
                                    .fg(theme::USERNAME)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        Cell::from(host.to_owned()),
                    ])
                })
                .collect();
            if rows.is_empty() {
                frame.render_widget(
                    Paragraph::new(
                        " No matches · type to search hosts from ~/.ssh/config or history",
                    )
                    .style(Style::default().fg(theme::SUBTEXT)),
                    areas[1],
                );
            } else {
                let profile_width = config
                    .profiles
                    .iter()
                    .map(|profile| Line::from(profile.name.as_str()).width())
                    .max()
                    .unwrap_or(0)
                    .min(usize::from(areas[1].width.saturating_sub(4) / 3))
                    as u16;
                let username_width = targets
                    .iter()
                    .filter_map(|target| target.host.rsplit_once('@').map(|(user, _)| user))
                    .map(|user| Line::from(user).width())
                    .max()
                    .unwrap_or(0)
                    .min(usize::from(areas[1].width.saturating_sub(4) / 4))
                    as u16;
                let table = Table::new(
                    rows,
                    [
                        Constraint::Length(profile_width),
                        Constraint::Length(username_width),
                        Constraint::Min(1),
                    ],
                )
                .column_spacing(1)
                .highlight_symbol("› ")
                .row_highlight_style(
                    Style::default()
                        .bg(theme::SELECTION)
                        .add_modifier(Modifier::BOLD),
                );
                let mut state = TableState::default().with_selected(Some(*selection));
                frame.render_stateful_widget(table, areas[1], &mut state);
            }
            let keys = &config.keybindings;
            bar_state.draw(
                frame,
                areas[2],
                config,
                "menu",
                vec![
                    bar::Item::profiles(config, profile_index),
                    bar::Item::action(keys, "menu", "connect", "Open", 2),
                    bar::Item::action(keys, "menu", "connect_and_return", "Return", 4),
                    bar::Item::action(keys, "menu", "connection_settings", "Edit", 5),
                    bar::Item::action(keys, "menu", "configure_remote", "Setup", 6),
                    bar::Item::action(keys, "menu", "new_remote", "New", 8),
                    bar::Item::action(keys, "menu", "edit_config", "Config", 7),
                    bar::Item::action(keys, "menu", "help", "Help", 0),
                    bar::Item::action(keys, "menu", "quit", "Quit", 1),
                ],
            );
            if let Some(i) = visible.get(*selection)
                && let Some(block) = host_config.get(&targets[*i].host)
            {
                search::hint(frame, areas[1], block);
            }
            if let Some(message) = notice.as_deref() {
                // Results must remain visible even where ordinary host hints are hidden.
                let rect = ratatui::layout::Rect::new(
                    areas[1].x,
                    areas[1].bottom() - areas[1].height.min(2),
                    areas[1].width,
                    areas[1].height.min(2),
                );
                frame.render_widget(ratatui::widgets::Clear, rect);
                frame.render_widget(
                    Paragraph::new(message)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .style(Style::default().fg(theme::USERNAME).bg(theme::SURFACE)),
                    rect,
                );
            }
        })?;
        let event = event::read()?;
        if matches!(event, Event::FocusLost) {
            bar_state.reset();
        }
        if let Event::Key(key) = event {
            if bar_state.event(key, &config.keybindings, "menu") {
                continue;
            }
            if key.kind == KeyEventKind::Release {
                continue;
            }
            notice.take();
            if key.kind == KeyEventKind::Press
                && let Some(profile) = config
                    .profiles
                    .iter()
                    .position(|p| p.key.as_ref().is_some_and(|shortcut| shortcut.matches(key)))
            {
                if let Some(i) = visible.get(*selection) {
                    if !config.profiles[profile].inputs.is_empty() {
                        bar_state.reset();
                        if let Some(result) =
                            new_remote::connect(terminal, config, &targets[*i], profile, false)?
                        {
                            return Ok(Some(result));
                        }
                        continue;
                    }
                    return Ok(Some(Selection {
                        target: targets[*i].clone(),
                        profile,
                        setup: None,
                        setup_command: None,
                        new: false,
                        configure_only: false,
                        return_to_menu: key.modifiers.contains(KeyModifiers::ALT)
                            && key.code == KeyCode::Enter,
                        connection_command: None,
                    }));
                }
                continue;
            }
            let repeated = key.kind == KeyEventKind::Repeat;
            let Some(key) = config.keybindings.translate("menu", key) else {
                continue;
            };
            if repeated
                && !matches!(
                    key.code,
                    KeyCode::Char(_)
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Backspace
                        | KeyCode::Delete
                )
            {
                continue;
            }
            match key.code {
                KeyCode::F(1) => {
                    bar_state.reset();
                    help::show(terminal, help::Context::Hosts, config)?;
                }
                KeyCode::F(9) => {
                    bar_state.reset();
                    editor::edit(terminal, home, config, profiles)?;
                }
                KeyCode::F(4) => {
                    bar_state.reset();
                    if let Some(mut selection) = new_remote::dialog(
                        terminal,
                        config,
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            None
                        } else {
                            visible.get(*selection).map(|i| &targets[*i])
                        },
                    )? {
                        if !selection.configure_only
                            && !config.profiles[selection.profile].inputs.is_empty()
                        {
                            let Some(connection) = new_remote::connect(
                                terminal,
                                config,
                                &selection.target,
                                selection.profile,
                                selection.setup.is_some(),
                            )?
                            else {
                                continue;
                            };
                            selection.target = connection.target;
                            selection.profile = connection.profile;
                            selection.connection_command = connection.connection_command;
                            selection.return_to_menu |= connection.return_to_menu;
                        }
                        return Ok(Some(selection));
                    }
                }
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                // Legacy terminals encode Ctrl-Backspace as Ctrl-H (0x08).
                KeyCode::Backspace | KeyCode::Delete | KeyCode::Char('h')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    erase_filter_word(query);
                    *selection = 0;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    query.push(c);
                    *selection = 0;
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    query.pop();
                    *selection = 0;
                }
                KeyCode::Up => *selection = selection.saturating_sub(1),
                KeyCode::Down => *selection = selection.saturating_add(1),
                KeyCode::PageUp => {
                    *selection = selection.saturating_sub(
                        terminal
                            .size()?
                            .height
                            .saturating_sub(footer_height + 1)
                            .max(1) as usize,
                    )
                }
                KeyCode::PageDown => {
                    *selection = selection.saturating_add(
                        terminal
                            .size()?
                            .height
                            .saturating_sub(footer_height + 1)
                            .max(1) as usize,
                    )
                }
                KeyCode::Left => {
                    if let Some(i) = visible.get(*selection) {
                        profiles[*i] =
                            (profiles[*i] + config.profiles.len() - 1) % config.profiles.len();
                    }
                }
                KeyCode::Right => {
                    if let Some(i) = visible.get(*selection) {
                        profiles[*i] = (profiles[*i] + 1) % config.profiles.len();
                    }
                }
                KeyCode::F(8) => {
                    if let Some(i) = visible.get(*selection) {
                        profiles[*i] = config.toggle_transport(profiles[*i]);
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    bar_state.reset();
                    if let Some(i) = visible.get(*selection)
                        && let Some(result) = new_remote::connect(
                            terminal,
                            config,
                            &targets[*i],
                            profile_index,
                            false,
                        )?
                    {
                        return Ok(Some(result));
                    }
                }
                KeyCode::Enter => {
                    if let Some(i) = visible.get(*selection) {
                        if !config.profiles[profile_index].inputs.is_empty() {
                            bar_state.reset();
                            if let Some(mut result) = new_remote::connect(
                                terminal,
                                config,
                                &targets[*i],
                                profile_index,
                                false,
                            )? {
                                result.return_to_menu |= key.modifiers.contains(KeyModifiers::ALT);
                                return Ok(Some(result));
                            }
                            continue;
                        }
                        return Ok(Some(Selection {
                            target: targets[*i].clone(),
                            profile: profile_index,
                            setup: None,
                            setup_command: None,
                            new: false,
                            configure_only: false,
                            return_to_menu: key.modifiers.contains(KeyModifiers::ALT)
                                && key.code == KeyCode::Enter,
                            connection_command: None,
                        }));
                    }
                }
                _ => {}
            }
        }
    }
}
fn init_terminal() -> io::Result<DefaultTerminal> {
    let terminal = ratatui::try_init()?;
    execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        ),
        event::EnableFocusChange
    )?;
    Ok(terminal)
}

fn suspend_terminal() -> io::Result<()> {
    execute!(
        io::stdout(),
        PopKeyboardEnhancementFlags,
        event::DisableFocusChange
    )?;
    let restore = ratatui::try_restore();
    let appearance = execute!(
        io::stdout(),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Show
    );
    restore.and(appearance).and(io::stdout().flush())
}

// exec() skips destructors, so finish all TUI cleanup before handing off the tty.
fn restore_terminal(terminal: DefaultTerminal) -> io::Result<()> {
    let result = suspend_terminal();
    drop(terminal);
    result
}

fn run() -> io::Result<()> {
    let home = env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?;
    let home = Path::new(&home);
    let user = env::var("USER").unwrap_or_default();
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!(
            "ssh-menu [--dump | --help | --version]\nTerminal menu for SSH connections and remote tools. Reads ~/.ssh/config, ~/.bash_history and MC hotlist."
        );
        return Ok(());
    }
    if args == ["--version"] {
        println!("ssh-menu {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !args.is_empty() && args != ["--dump"] {
        return Err(io::Error::other("unknown argument; use --help"));
    }
    let targets = discover(home, &user);
    if args == ["--dump"] {
        for t in targets {
            println!("{}", t.label());
        }
        return Ok(());
    }
    let mut config = launch::Config::load(home)?;
    let history = fs::read_to_string(home.join(".bash_history")).unwrap_or_default();
    let profiles = config.history_profiles(&targets, &history, &user);
    let mut state = MenuState {
        profiles,
        ..MenuState::default()
    };
    loop {
        let mut terminal = init_terminal()?;
        let result = menu(&mut terminal, &targets, &mut config, &mut state, home);
        restore_terminal(terminal)?;
        if let Some(Selection {
            target,
            profile: profile_index,
            setup,
            setup_command,
            new,
            configure_only,
            connection_command,
            return_to_menu,
        }) = result?
        {
            if let Some(index) = setup {
                let script = &config.configuration_scripts[index];
                println!("Running {} for {}", script.name, target.label());
                let (program, args) = setup_command.expect("validated setup command");
                let status = Command::new(program).args(args).status();
                if configure_only {
                    state.notice = Some(match status {
                        Ok(status) if status.success() => {
                            format!("{}: {} completed", target.label(), script.name)
                        }
                        Ok(status) => {
                            format!("{}: {} failed ({status})", target.label(), script.name)
                        }
                        Err(err) => format!("{}: {} failed: {err}", target.label(), script.name),
                    });
                    continue;
                }
                let status = status?;
                if !status.success() {
                    return Err(io::Error::other(format!(
                        "configuration script {} failed ({status}); connection cancelled",
                        script.name
                    )));
                }
            }
            let custom = connection_command.is_some();
            let (program, args) = connection_command
                .unwrap_or_else(|| config.profiles[profile_index].command(&target));
            let history = home.join(".bash_history");
            let line = config.profiles[profile_index].history_line(&target);
            if !custom
                && (new || history.exists())
                && fs::read_to_string(&history)
                    .unwrap_or_default()
                    .lines()
                    .last()
                    != Some(line.as_str())
            {
                let mut options = fs::OpenOptions::new();
                options.append(true).create(new);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&history)?;
                writeln!(file, "{line}")?;
            }
            if return_to_menu {
                Command::new(program).args(args).status()?;
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                return Err(Command::new(program).args(args).exec());
            }
            #[cfg(not(unix))]
            {
                let status = Command::new(program).args(args).status()?;
                if !status.success() {
                    return Err(io::Error::other("connection failed"));
                }
            }
        }
        return Ok(());
    }
}
fn main() {
    if let Err(err) = run() {
        eprintln!("ssh-menu: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_word_deletion() {
        for (input, expected) in [
            ("", ""),
            ("alpha", ""),
            ("alpha beta  ", "alpha "),
            ("alpha привет", "alpha "),
            ("alpha\u{2003}beta", "alpha\u{2003}"),
        ] {
            let mut query = input.to_owned();
            erase_filter_word(&mut query);
            assert_eq!(query, expected);
        }
    }

    #[test]
    fn history_options() {
        let t = history_target("exec ssh -i '/a key' -p 2222 alice@example.org", "alice").unwrap();
        assert_eq!(t.label(), "example.org:2222");
        for delimiter in ["", "-- "] {
            let line = format!("exec ssh -l bob -p2222 {delimiter}host exec bash -l");
            let (target, remote, program) = history_connection(&line, "alice").unwrap();
            assert_eq!(target.label(), "bob@host:2222");
            assert_eq!(remote, ["exec", "bash", "-l"]);
            assert_eq!(program, "ssh");
        }
        assert!(history_target("echo ssh example.org", "alice").is_none());
        assert!(history_target("ssh 'host;touch /tmp/no'", "alice").is_none());
    }
    #[test]
    fn mosh_and_override() {
        let (t, _, program) =
            history_connection("mosh --ssh='ssh -p 2200' bob@host", "alice").unwrap();
        assert_eq!(program, "mosh");
        assert_eq!(t.label(), "bob@host:2200");
    }
    #[test]
    fn filtering() {
        let ts = vec![target("Alice@Host:22", "alice").unwrap()];
        assert_eq!(search::matching(&ts, "HOST", &HashMap::new()), vec![0]);
        assert!(search::matching(&ts, "absent", &HashMap::new()).is_empty());
    }
    #[test]
    fn discovery_recency() {
        let dir = env::temp_dir().join(format!("ssh-menu-test-{}", std::process::id()));
        fs::create_dir_all(dir.join(".ssh")).unwrap();
        fs::create_dir_all(dir.join(".config/mc")).unwrap();
        fs::write(
            dir.join(".ssh/config"),
            "Host alpha beta wild*\n User alice\n",
        )
        .unwrap();
        fs::write(
            dir.join(".config/mc/hotlist"),
            "ENTRY \"server\" URL \"/sh://bob@mc/path\"\n",
        )
        .unwrap();
        fs::write(
            dir.join(".bash_history"),
            "ssh alpha\nmosh beta\nssh alpha\n",
        )
        .unwrap();
        let ts = discover(&dir, "alice");
        assert_eq!(ts[0].label(), "alpha");
        assert_eq!(ts[1].label(), "beta");
        assert!(ts.iter().any(|t| t.label() == "bob@mc"));
        assert!(!ts.iter().any(|t| t.host.contains('*')));
        fs::remove_dir_all(dir).unwrap();
    }
}
