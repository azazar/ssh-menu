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
    selected: Option<&Target>,
) -> io::Result<Option<Selection>> {
    form(terminal, config, None, false, selected)
}

pub(super) fn connect(
    terminal: &mut DefaultTerminal,
    config: &launch::Config,
    target: &Target,
    profile: usize,
    fixed_remote: bool,
) -> io::Result<Option<Selection>> {
    form(
        terminal,
        config,
        Some((target, profile)),
        fixed_remote,
        None,
    )
}

fn form(
    terminal: &mut DefaultTerminal,
    config: &launch::Config,
    connection: Option<(&Target, usize)>,
    fixed_remote: bool,
    selected: Option<&Target>,
) -> io::Result<Option<Selection>> {
    let connecting = connection.is_some();
    let mut configuring = selected.is_some();
    let mut remote = connection
        .map(|(t, _)| t.label())
        .or_else(|| selected.map(Target::label))
        .unwrap_or_default();
    let mut alternate_remote = String::new();
    let remote_field = usize::from(!connecting);
    let recipe_field = remote_field + 1;
    let input_start = recipe_field + 1;
    let mut setup = connection.map_or(0, |(_, p)| p + 1);
    let mut focus = if connecting || configuring {
        0
    } else {
        remote_field
    };
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
    let mut bar_state = bar::State::default();
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
        let field_count = inputs.len() + input_start;
        focus = focus.min(field_count - 1);
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
                area,
            );
            let width = area.width.min(90);
            let height = (inputs.len().saturating_add(8 + remote_field))
                .min(usize::from(area.height)) as u16;
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
                    " REMOTE SETUP "
                })
                .style(
                    Style::default()
                        .fg(theme::BASE)
                        .bg(theme::LAVENDER)
                        .add_modifier(Modifier::BOLD),
                ),
                areas[0],
            );
            let mut fields = Vec::new();
            if !connecting {
                fields.push(format!(
                    "Action  ← {} →",
                    if configuring {
                        "Configure host"
                    } else {
                        "Add new"
                    }
                ));
            }
            fields.extend([
                format!("Remote  {remote}"),
                format!(
                    "{}   ← {} →",
                    if connecting { "Profile" } else { "Setup" },
                    script.map_or("None", |s| s.name.as_str())
                ),
            ]);
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
                    if configuring {
                        " Runs setup only · returns to menu · * required"
                    } else if fixed_remote {
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
            let keys = &config.keybindings;
            bar_state.draw(
                frame,
                areas[3],
                config,
                "dialog",
                vec![
                    bar::Item::pair(keys, "dialog", "previous_field", "next_field", "Field", 3),
                    bar::Item::new(
                        format!(
                            "{}{}",
                            keys.hint("dialog", "left"),
                            keys.hint("dialog", "right")
                        ),
                        "Choice",
                        4,
                    ),
                    bar::Item::action(
                        keys,
                        "dialog",
                        "connect",
                        if connecting {
                            "Open"
                        } else if configuring {
                            "Apply"
                        } else {
                            "Add+open"
                        },
                        2,
                    ),
                    bar::Item::action(
                        keys,
                        "dialog",
                        "connect_and_return",
                        if configuring { "Apply" } else { "Return" },
                        5,
                    ),
                    bar::Item::action(keys, "dialog", "help", "Help", 0),
                    bar::Item::action(keys, "dialog", "cancel", "Cancel", 1),
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
        if bar_state.event(key, &config.keybindings, "dialog") {
            continue;
        }
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let repeated = key.kind == KeyEventKind::Repeat;
        let Some(key) = config.keybindings.translate("dialog", key) else {
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
                    | KeyCode::Tab
                    | KeyCode::BackTab
            )
        {
            continue;
        }
        match key.code {
            KeyCode::F(1) => {
                bar_state.reset();
                crate::help::show(
                    terminal,
                    if connecting {
                        crate::help::Context::Connection
                    } else {
                        crate::help::Context::Setup
                    },
                    config,
                )?;
            }
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
                if !connecting && focus == 0 {
                    configuring = !configuring;
                    std::mem::swap(&mut remote, &mut alternate_remote);
                } else if focus <= recipe_field {
                    setup = if connecting {
                        next(setup - 1, scripts.len()) + 1
                    } else {
                        next(setup, scripts.len() + 1)
                    };
                    generation_error.clear();
                } else {
                    let input = &inputs[focus - input_start];
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
                if configuring && setup == 0 {
                    error = " Choose a setup recipe to configure this host".into();
                    focus = recipe_field;
                    continue;
                }
                let Some(target) = parse_remote(&remote) else {
                    error = " Enter a valid host and optional port (1–65535)".into();
                    focus = remote_field;
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
                        focus = index + input_start;
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
                    new: !connecting && !configuring,
                    configure_only: configuring,
                    return_to_menu: key.modifiers.contains(KeyModifiers::ALT),
                }));
            }
            _ => {
                let editable = if focus == remote_field && !fixed_remote {
                    Some(&mut remote)
                } else if focus >= input_start
                    && inputs[focus - input_start].kind == launch::InputType::PromptString
                {
                    values[setup - 1].get_mut(&inputs[focus - input_start].id)
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
