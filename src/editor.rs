use crate::*;
use ratatui::widgets::Wrap;

pub(super) fn edit(
    terminal: &mut impl ui::Terminal,
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
            match terminal.edit_config(home) {
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
                        "{err}\n\nConfiguration not reloaded.\n{}",
                        launch::config_path(home).display()
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
        let event = terminal.read()?;
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
            KeyCode::F(1) => {
                bar_state.reset();
                help::show(terminal, help::Context::Configuration, config)?;
            }
            KeyCode::Enter => edit_requested = true,
            _ => {}
        }
    }
}

pub(super) fn launch_editor(
    terminal: &mut DefaultTerminal,
    home: &Path,
) -> io::Result<launch::Config> {
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| if cfg!(windows) { "notepad" } else { "vi" }.into());
    let words = editor_words(&editor)
        .filter(|words| !words.is_empty())
        .ok_or_else(|| io::Error::other("Invalid VISUAL / EDITOR command."))?;
    suspend_terminal()?;
    let status = Command::new(&words[0])
        .args(&words[1..])
        .arg(launch::config_path(home))
        .status();
    *terminal = init_terminal()?;
    let status = status?;
    if !status.success() {
        return Err(io::Error::other(format!("Editor exited with {status}")));
    }
    launch::Config::load(home)
}

#[cfg(not(windows))]
fn editor_words(editor: &str) -> Option<Vec<String>> {
    shell_words::split(editor).ok()
}

#[cfg(windows)]
fn editor_words(editor: &str) -> Option<Vec<String>> {
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
    let wide: Vec<u16> = editor.encode_utf16().chain(Some(0)).collect();
    let mut count = 0;
    // SAFETY: wide is NUL-terminated; Windows returns count valid strings in
    // one allocation, released with LocalFree after copying them.
    unsafe {
        let args = CommandLineToArgvW(wide.as_ptr(), &mut count);
        if args.is_null() {
            return None;
        }
        let words = std::slice::from_raw_parts(args, count as usize)
            .iter()
            .map(|&arg| {
                let mut len = 0;
                while *arg.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(arg, len))
            })
            .collect();
        LocalFree(args.cast());
        Some(words)
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn editor_preserves_windows_paths_and_arguments() {
        assert_eq!(
            super::editor_words(r#""C:\Program Files\Editor\edit.exe" --wait"#).unwrap(),
            [r"C:\Program Files\Editor\edit.exe", "--wait"]
        );
    }
}
