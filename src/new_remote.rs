use crate::*;
use ratatui::layout::Rect;

fn parse_remote(text: &str) -> Option<Target> {
    let text = text.trim();
    if text.starts_with('@') {
        return None;
    }
    let parsed = target(text, "")?;
    if parsed
        .port
        .as_ref()
        .is_some_and(|p| p.parse::<u16>().map_or(true, |p| p == 0))
    {
        return None;
    }
    let host = parsed
        .host
        .rsplit_once('@')
        .map_or(parsed.host.as_str(), |(_, h)| h);
    if host.is_empty()
        || host.starts_with('-')
        || parsed.host.starts_with('@')
        || parsed.host.matches('@').count() > 1
        || (host.contains(':') && !(host.starts_with('[') && host.ends_with(']')))
    {
        return None;
    }
    Some(parsed)
}

pub(super) fn dialog(
    terminal: &mut DefaultTerminal,
    config: &launch::Config,
) -> io::Result<Option<Selection>> {
    form(terminal, config, None, false)
}

pub(super) fn connect(
    terminal: &mut DefaultTerminal,
    config: &launch::Config,
    target: &Target,
    profile: usize,
    fixed_remote: bool,
) -> io::Result<Option<Selection>> {
    form(terminal, config, Some((target, profile)), fixed_remote)
}

fn form(
    terminal: &mut DefaultTerminal,
    config: &launch::Config,
    connection: Option<(&Target, usize)>,
    fixed_remote: bool,
) -> io::Result<Option<Selection>> {
    let connecting = connection.is_some();
    let mut remote = connection.map_or_else(String::new, |(t, _)| t.label());
    let mut setup = connection.map_or(0, |(_, p)| p + 1);
    let mut focus = 0usize;
    let available: Vec<_> = if connecting {
        (0..config.profiles.len()).collect()
    } else {
        config
            .configuration_scripts
            .iter()
            .enumerate()
            .filter(|(_, script)| script.is_available())
            .map(|(i, _)| i)
            .collect()
    };
    let mut scripts: Vec<_> = available
        .iter()
        .map(|i| {
            if connecting {
                config.profiles[*i].clone()
            } else {
                config.configuration_scripts[*i].clone()
            }
        })
        .collect();
    let mut values: Vec<_> = scripts.iter().map(|p| p.input_defaults()).collect();
    let mut loaded = vec![false; scripts.len()];
    let mut generation_error = String::new();
    let mut error = String::new();
    loop {
        if let Some(i) = setup.checked_sub(1)
            && !loaded[i]
            && generation_error.is_empty()
        {
            let resolved: io::Result<Vec<_>> = scripts[i]
                .inputs
                .iter()
                .map(|input| input.resolve())
                .collect();
            match resolved {
                Ok(inputs) => {
                    scripts[i].inputs = inputs;
                    values[i] = scripts[i].input_defaults();
                    loaded[i] = true;
                }
                Err(err) => generation_error = err.to_string(),
            }
        }
        let script = setup.checked_sub(1).map(|i| &scripts[i]);
        let inputs = script.map_or(&[][..], |s| s.inputs.as_slice());
        let field_count = inputs.len() + 2;
        focus = focus.min(field_count - 1);
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
                area,
            );
            let width = area.width.min(90);
            let height = (inputs.len().saturating_add(8)).min(usize::from(area.height)) as u16;
            let rect = Rect::new(
                area.x + (area.width - width) / 2,
                area.y + (area.height - height) / 2,
                width,
                height,
            );
            let areas = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(rect);
            frame.render_widget(
                Paragraph::new(if connecting {
                    " CONNECT "
                } else {
                    " NEW REMOTE "
                })
                .style(
                    Style::default()
                        .fg(theme::BASE)
                        .bg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
                areas[0],
            );
            let mut fields = vec![
                format!("Remote  {remote}"),
                format!(
                    "{}   ← {} →",
                    if connecting { "Profile" } else { "Setup" },
                    script.map_or("None", |s| s.name.as_str())
                ),
            ];
            for input in inputs {
                let value = &values[setup - 1][&input.id];
                let label = format!(
                    "{}{}",
                    input.label(),
                    if input.required { " *" } else { "" }
                );
                fields.push(if input.kind == launch::InputType::PickString {
                    format!("{label}  ← {value} →")
                } else {
                    format!("{label}  {value}")
                });
            }
            // A stateful list scrolls the focused field into view on short terminals.
            let items: Vec<_> = fields
                .into_iter()
                .enumerate()
                .map(|(i, field)| {
                    let text = if i == focus {
                        let width = usize::from(areas[1].width.saturating_sub(3));
                        let chars: Vec<_> = field.chars().collect();
                        if chars.len() > width {
                            chars[chars.len() - width..].iter().collect()
                        } else {
                            field
                        }
                    } else {
                        field
                    };
                    ListItem::new(text)
                })
                .collect();
            let list = List::new(items)
                .style(Style::default().bg(theme::SURFACE))
                .highlight_symbol("› ")
                .highlight_style(Style::default().bg(theme::SELECTION));
            frame.render_stateful_widget(
                list,
                areas[1],
                &mut ListState::default().with_selected(Some(focus)),
            );
            frame.render_widget(
                Paragraph::new(if !generation_error.is_empty() {
                    &generation_error
                } else if error.is_empty() {
                    if fixed_remote {
                        " Remote fixed for setup · * required"
                    } else {
                        " user@host[:port] · * required"
                    }
                } else {
                    &error
                })
                .style(Style::default().fg(
                    if error.is_empty() && generation_error.is_empty() {
                        theme::SUBTEXT
                    } else {
                        theme::PEACH
                    },
                )),
                areas[2],
            );
            frame.render_widget(
                Paragraph::new(format!(
                    " {} HELP · {} FIELD · {} CONNECT · {} RETURN · {} CANCEL",
                    config.keybindings.hint("dialog", "help"),
                    config.keybindings.hint("dialog", "next_field"),
                    config.keybindings.hint("dialog", "connect"),
                    config.keybindings.hint("dialog", "connect_and_return"),
                    config.keybindings.hint("dialog", "cancel")
                )),
                areas[3],
            );
        })?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(key) = config.keybindings.translate("dialog", key) else {
            continue;
        };
        match key.code {
            KeyCode::F(1) => crate::help::show(
                terminal,
                if connecting {
                    crate::help::Context::Connection
                } else {
                    crate::help::Context::NewRemote
                },
                &config.keybindings,
            )?,
            KeyCode::Esc => return Ok(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(None);
            }
            KeyCode::Tab | KeyCode::Down => focus = (focus + 1) % field_count,
            KeyCode::BackTab | KeyCode::Up => focus = (focus + field_count - 1) % field_count,
            KeyCode::Left | KeyCode::Right => {
                let next = |current: usize, len: usize| {
                    if key.code == KeyCode::Right {
                        (current + 1) % len
                    } else {
                        (current + len - 1) % len
                    }
                };
                if focus < 2 {
                    setup = if connecting {
                        next(setup - 1, scripts.len()) + 1
                    } else {
                        next(setup, scripts.len() + 1)
                    };
                    generation_error.clear();
                } else {
                    let input = &inputs[focus - 2];
                    if input.kind == launch::InputType::PickString && !input.options.is_empty() {
                        let value = values[setup - 1].get_mut(&input.id).unwrap();
                        let index = input.options.iter().position(|v| v == value).unwrap_or(0);
                        *value = input.options[next(index, input.options.len())].clone();
                    }
                }
                error.clear();
            }
            KeyCode::Enter => {
                if !generation_error.is_empty() {
                    continue;
                }
                let Some(target) = parse_remote(&remote) else {
                    error = " Enter a valid host and optional port (1–65535)".into();
                    focus = 0;
                    continue;
                };
                let selected_values = setup
                    .checked_sub(1)
                    .map_or_else(HashMap::new, |i| values[i].clone());
                let mut setup_command = None;
                if let Some(script) = script {
                    if let Some(index) = inputs
                        .iter()
                        .position(|i| i.required && selected_values[&i.id].trim().is_empty())
                    {
                        error = format!(" {} is required", inputs[index].label());
                        focus = index + 2;
                        continue;
                    }
                    match script.command_with_inputs(&target, &selected_values) {
                        Ok(command) => setup_command = Some(command),
                        Err(err) => {
                            error = format!(" {err}");
                            continue;
                        }
                    }
                }
                return Ok(Some(Selection {
                    target,
                    profile: if connecting {
                        setup - 1
                    } else {
                        config.default_index()
                    },
                    setup: if connecting {
                        None
                    } else {
                        setup.checked_sub(1).map(|i| available[i])
                    },
                    connection_command: if connecting {
                        setup_command.clone()
                    } else {
                        None
                    },
                    setup_command: if connecting { None } else { setup_command },
                    new: !connecting,
                    return_to_menu: key.modifiers.contains(KeyModifiers::ALT),
                }));
            }
            _ => {
                let editable = if focus == 0 && !fixed_remote {
                    Some(&mut remote)
                } else if focus >= 2 && inputs[focus - 2].kind == launch::InputType::PromptString {
                    values[setup - 1].get_mut(&inputs[focus - 2].id)
                } else {
                    None
                };
                if let Some(value) = editable {
                    match key.code {
                        KeyCode::Backspace | KeyCode::Delete | KeyCode::Char('h')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            crate::erase_filter_word(value)
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            value.pop();
                        }
                        KeyCode::Char(c)
                            if !c.is_control()
                                && !key
                                    .modifiers
                                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            value.push(c)
                        }
                        _ => continue,
                    }
                    error.clear();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_remote_before_running_scripts() {
        for valid in ["host", "alice@host:2222", "user@[::1]:22"] {
            assert!(parse_remote(valid).is_some(), "{valid}");
        }
        for invalid in [
            "",
            "-oProxyCommand=x",
            "host;touch x",
            "@host",
            "user@",
            "host:0",
            "host:65536",
            "host:abc",
            "a@b@c",
        ] {
            assert!(parse_remote(invalid).is_none(), "{invalid}");
        }
    }
}
