use crate::*;
use ratatui::widgets::Wrap;

pub(super) fn edit(
    terminal: &mut DefaultTerminal,
    home: &Path,
    config: &mut launch::Config,
    profiles: &mut [usize],
) -> io::Result<()> {
    let mut message = String::new();
    let mut edit_requested = true;
    let mut bar_state = bar::State::default();
    loop {
        if edit_requested {
            edit_requested = false;
            bar_state.reset();
            match launch_editor(terminal, home) {
                Ok(updated) => {
                    for index in profiles.iter_mut() {
                        let name = &config.profiles[*index].name;
                        *index = updated
                            .profiles
                            .iter()
                            .position(|p| &p.name == name)
                            .unwrap_or(updated.default_index());
                    }
                    *config = updated;
                    return Ok(());
                }
                Err(err) => {
                    message = format!(
                        "{}\n\n{err}\n\nConfiguration not reloaded.\nEdit to fix, open help, or cancel to keep the current configuration.",
                        home.join(".ssh").join(launch::CONFIG_NAME).display()
                    )
                }
            }
        }
        terminal.draw(|frame| {
            frame.render_widget(
                Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
                frame.area(),
            );
            let areas =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
            frame.render_widget(
                Paragraph::new(format!(" CONFIGURATION\n\n{message}")).wrap(Wrap { trim: false }),
                areas[0],
            );
            let keys = &config.keybindings;
            bar_state.draw(
                frame,
                areas[1],
                config,
                "editor",
                vec![
                    bar::Item::action(keys, "editor", "edit", "Edit", 2),
                    bar::Item::action(keys, "editor", "help", "Help", 0),
                    bar::Item::action(keys, "editor", "cancel", "Cancel", 1),
                ],
            );
        })?;
        let event = event::read()?;
        if matches!(event, Event::FocusLost) {
            bar_state.reset();
        }
        let Event::Key(key) = event else {
            continue;
        };
        if bar_state.event(key, &config.keybindings, "editor") {
            continue;
        }
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(key) = config.keybindings.translate("editor", key) else {
            continue;
        };
        match key.code {
            KeyCode::Esc => return Ok(()),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::F(1) => help::show(terminal, help::Context::Configuration, config)?,
            KeyCode::Enter => edit_requested = true,
            _ => {}
        }
    }
}

fn launch_editor(terminal: &mut DefaultTerminal, home: &Path) -> io::Result<launch::Config> {
    if !home.join(".ssh").is_dir() {
        return Err(io::Error::other(
            "~/.ssh does not exist. Create it before editing configuration.",
        ));
    }
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| "vi".into());
    let words = shell_words::split(&editor)
        .ok()
        .filter(|words| !words.is_empty())
        .ok_or_else(|| io::Error::other("Invalid VISUAL / EDITOR command."))?;
    suspend_terminal()?;
    let status = Command::new(&words[0])
        .args(&words[1..])
        .arg(home.join(".ssh").join(launch::CONFIG_NAME))
        .status();
    *terminal = init_terminal()?;
    let status = status?;
    if !status.success() {
        return Err(io::Error::other(format!("Editor exited with {status}")));
    }
    launch::Config::load(home)
}
