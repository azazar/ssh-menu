use crate::*;
use ratatui::widgets::Wrap;

pub(super) fn edit(
    terminal: &mut DefaultTerminal,
    home: &Path,
    config: &mut launch::Config,
    profiles: &mut [usize],
) -> io::Result<()> {
    let path = home.join(".ssh").join(launch::CONFIG_NAME);
    let mut message = format!(
        "{}\n\nEdit with $VISUAL, $EDITOR, or vi. Help explains the configuration format.\n\nSaving a valid configuration reloads it immediately. Invalid edits stay on disk for correction; the current configuration remains active.",
        path.display()
    );
    let mut bar_state = bar::State::default();
    loop {
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
            KeyCode::Enter | KeyCode::F(4) => {
                bar_state.reset();
                if !home.join(".ssh").is_dir() {
                    message =
                        "~/.ssh does not exist. Create it before editing configuration.".into();
                    continue;
                }
                let editor = env::var("VISUAL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| env::var("EDITOR").ok())
                    .unwrap_or_else(|| "vi".into());
                let words = match shell_words::split(&editor) {
                    Ok(words) if !words.is_empty() => words,
                    _ => {
                        message = "Invalid VISUAL / EDITOR command.".into();
                        continue;
                    }
                };
                suspend_terminal()?;
                let status = Command::new(&words[0])
                    .args(&words[1..])
                    .arg(&path)
                    .status();
                *terminal = init_terminal()?;
                let loaded = match status {
                    Ok(status) if status.success() => launch::Config::load(home),
                    Ok(status) => Err(io::Error::other(format!("Editor exited with {status}"))),
                    Err(err) => Err(err),
                };
                match loaded {
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
                            "{err}\n\nConfiguration not reloaded.\nEdit to fix, open help, or cancel to keep the current configuration."
                        )
                    }
                }
            }
            _ => {}
        }
    }
}
