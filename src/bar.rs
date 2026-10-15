use crate::*;

pub(super) struct Item {
    prefix: String,
    key: String,
    label: String,
    priority: u8,
    selected: bool,
    alternatives: Vec<String>,
    action: Option<String>,
}
impl Item {
    pub fn new(key: impl Into<String>, label: impl Into<String>, priority: u8) -> Self {
        Self {
            prefix: String::new(),
            key: key.into(),
            label: label.into(),
            priority,
            selected: false,
            alternatives: Vec::new(),
            action: None,
        }
    }
    pub fn action(
        bindings: &bindings::Bindings,
        scope: &str,
        action: &str,
        label: &str,
        priority: u8,
    ) -> Self {
        let mut item = Self::new(bindings.hint(scope, action), label, priority);
        item.alternatives = bindings.key_labels(scope, action);
        item.action = Some(action.to_owned());
        item
    }
    pub fn pair(
        bindings: &bindings::Bindings,
        scope: &str,
        previous: &str,
        next: &str,
        label: &str,
        priority: u8,
    ) -> Self {
        Self::new(pair(bindings, scope, previous, next), label, priority)
    }
    pub fn profiles(config: &launch::Config, current: usize) -> Self {
        let count = config.profiles.len();
        let left = config.keybindings.hint("menu", "left");
        let right = config.keybindings.hint("menu", "right");
        if count < 2 || (left == "—" && right == "—") {
            return Self::new("—", "", 3);
        }
        let mut item = Self::new(
            format!(
                "{} {}",
                if left == "—" { "" } else { &left },
                if right == "—" { "" } else { &right }
            )
            .trim(),
            if right == "—" {
                ""
            } else {
                &config.profiles[(current + 1) % count].name
            },
            3,
        );
        if left != "—" {
            item.prefix = format!("{} ", config.profiles[(current + count - 1) % count].name);
        }
        item
    }
    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }
}

pub(super) fn pair(
    bindings: &bindings::Bindings,
    scope: &str,
    previous: &str,
    next: &str,
) -> String {
    [previous, next]
        .iter()
        .filter_map(|action| {
            let keys = bindings.key_labels(scope, action);
            keys.iter()
                .find(|key| matches!(key.as_str(), "←" | "→" | "↑" | "↓"))
                .or(keys.first())
                .cloned()
        })
        .collect::<Vec<_>>()
        .join("")
}

fn compact(key: &str) -> String {
    if key == "Ctrl+Alt" {
        return key.to_owned();
    }
    key.replace("Ctrl+", "C-")
        .replace("Alt+", "A-")
        .replace("Shift+", "S-")
        .replace("Enter", "↵")
        .replace("Backspace", "Bksp")
        .replace("Delete", "Del")
}

// Keep the visual order, but retain the most useful actions first on narrow terminals.
pub(super) fn line(items: Vec<Item>, width: u16) -> Line<'static> {
    let mut candidates: Vec<_> = items
        .into_iter()
        .filter(|i| i.key != "—")
        .enumerate()
        .map(|(index, mut item)| {
            item.key = compact(&item.key);
            let text = format!(
                " {}{}{}{} ",
                item.prefix,
                item.key,
                if item.key.is_empty() { "" } else { " " },
                item.label
            );
            (index, item, Line::from(text).width())
        })
        .collect();
    candidates.sort_by_key(|(index, item, _)| (item.priority, *index));
    let mut remaining = usize::from(width);
    candidates.retain(|(_, _, size)| {
        if *size > remaining {
            false
        } else {
            remaining -= size;
            true
        }
    });
    candidates.sort_by_key(|(index, _, _)| *index);
    let mut spans = Vec::new();
    for (_, item, _) in candidates {
        let background = if item.selected {
            theme::LAVENDER
        } else {
            theme::SURFACE
        };
        let foreground = if item.selected {
            theme::BASE
        } else {
            theme::TEXT
        };
        spans.push(Span::styled(
            format!(" {}", item.prefix),
            Style::default().bg(background).fg(foreground),
        ));
        spans.push(Span::styled(
            item.key.clone(),
            Style::default()
                .bg(background)
                .fg(if item.selected {
                    theme::BASE
                } else {
                    theme::LAVENDER
                })
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(
                "{}{} ",
                if item.key.is_empty() { "" } else { " " },
                item.label
            ),
            Style::default().bg(background).fg(foreground),
        ));
    }
    Line::from(spans)
}

pub(super) fn draw(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, items: Vec<Item>) {
    frame.render_widget(
        Paragraph::new(line(items, area.width)).style(Style::default().bg(theme::SURFACE)),
        area,
    );
}

fn group(key: &str) -> KeyModifiers {
    let mut flags = KeyModifiers::NONE;
    if key.contains("Ctrl+") {
        flags |= KeyModifiers::CONTROL;
    }
    if key.contains("Alt+") {
        flags |= KeyModifiers::ALT;
    }
    flags
}

pub(super) struct State {
    held: Vec<crossterm::event::ModifierKeyCode>,
    latched: KeyModifiers,
}
impl Default for State {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            latched: KeyModifiers::NONE,
        }
    }
}
impl State {
    pub fn event(
        &mut self,
        key: crossterm::event::KeyEvent,
        bindings: &bindings::Bindings,
        scope: &str,
    ) -> bool {
        if let KeyCode::Modifier(modifier) = key.code {
            self.latched = KeyModifiers::NONE;
            if key.kind == KeyEventKind::Release {
                self.held.retain(|m| *m != modifier);
            } else if !self.held.contains(&modifier) {
                self.held.push(modifier);
            }
            return true;
        }
        if key.kind == KeyEventKind::Press {
            use crossterm::event::ModifierKeyCode::*;
            self.held.retain(|modifier| match modifier {
                LeftAlt | RightAlt => key.modifiers.contains(KeyModifiers::ALT),
                LeftControl | RightControl => key.modifiers.contains(KeyModifiers::CONTROL),
                _ => true,
            });
            let reveal = bindings
                .translate(scope, key)
                .and_then(|event| match event.code {
                    KeyCode::F(21) => Some(KeyModifiers::ALT),
                    KeyCode::F(22) => Some(KeyModifiers::CONTROL | KeyModifiers::ALT),
                    KeyCode::F(23) => Some(KeyModifiers::CONTROL),
                    _ => None,
                });
            if let Some(reveal) = reveal {
                self.latched = if self.latched == reveal {
                    KeyModifiers::NONE
                } else {
                    reveal
                };
                return true;
            }
            self.latched = KeyModifiers::NONE;
        }
        false
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    fn modifiers(&self) -> KeyModifiers {
        use crossterm::event::ModifierKeyCode::*;
        self.held.iter().fold(self.latched, |flags, key| {
            flags
                | match key {
                    LeftAlt | RightAlt => KeyModifiers::ALT,
                    LeftControl | RightControl => KeyModifiers::CONTROL,
                    _ => KeyModifiers::NONE,
                }
        })
    }
    pub fn draw(
        &self,
        frame: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        config: &launch::Config,
        scope: &str,
        mut items: Vec<Item>,
    ) {
        let active = self.modifiers();
        if active.is_empty() {
            items.retain_mut(|item| {
                if !item.alternatives.is_empty() {
                    let Some(key) = item.alternatives.iter().find(|key| group(key).is_empty())
                    else {
                        return false;
                    };
                    item.key = key.clone();
                }
                group(&item.key).is_empty()
            });
            for (action, name) in [
                ("reveal_ctrl", "Ctrl"),
                ("reveal_alt", "Alt"),
                ("reveal_ctrl_alt", "Ctrl+Alt"),
            ] {
                let key = config.keybindings.hint(scope, action);
                if key != "—" {
                    items.push(Item::new(
                        if key.ends_with("+?") {
                            name.to_owned()
                        } else {
                            key
                        },
                        "?",
                        3,
                    ));
                }
            }
        } else {
            let label = match active {
                KeyModifiers::ALT => "Alt",
                KeyModifiers::CONTROL => "Ctrl",
                _ => "Ctrl+Alt",
            };
            let mut revealed = vec![Item::new("", label, 0).selected()];
            for (action, keys) in config.keybindings.actions(scope) {
                if action.starts_with("reveal_") {
                    continue;
                }
                let keys: Vec<_> = keys
                    .into_iter()
                    .filter(|key| group(key) == active)
                    .map(|key| key.replace("Ctrl+", "").replace("Alt+", ""))
                    .collect();
                if keys.is_empty() {
                    continue;
                }
                let label = match action.as_str() {
                    "connect" => "Open",
                    "connect_and_return" => "Return",
                    "connection_settings" => "Edit",
                    "erase_character" => "Erase",
                    "erase_word" => "Word",
                    "quit" => "Quit",
                    "cancel" => "Cancel",
                    "help" => "Help",
                    "close" => "Close",
                    "new_remote" => "Setup",
                    "edit_config" => "Config",
                    "edit" => "Edit",
                    "toggle_transport" => "SSH/Mosh",
                    "next_field" => "Next field",
                    "previous_field" => "Previous field",
                    "next_topic" => "Next topic",
                    "previous_topic" => "Previous topic",
                    "left" => "Previous",
                    "right" => "Next",
                    "up" => "Up",
                    "down" => "Down",
                    "page_up" => "Page up",
                    "page_down" => "Page down",
                    "home" => "Top",
                    "end" => "End",
                    other => other,
                };
                let label = items
                    .iter()
                    .find(|item| item.action.as_deref() == Some(action.as_str()))
                    .map_or(label, |item| item.label.as_str());
                revealed.push(Item::new(keys.join("/"), label, 2));
            }
            if scope == "menu" {
                for profile in &config.profiles {
                    if let Some(key) = &profile.key {
                        let key = key.to_string();
                        if group(&key) == active {
                            revealed.push(Item::new(
                                key.replace("Ctrl+", "").replace("Alt+", ""),
                                &profile.name,
                                1,
                            ));
                        }
                    }
                }
            }
            if revealed.len() == 1 {
                revealed.push(Item::new("", "No shortcuts", 1));
            }
            items = revealed;
        }
        draw(frame, area, items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, ModifierKeyCode::*};
    #[test]
    fn modifiers_track_both_sides_and_release_independently() {
        let bindings = bindings::Bindings::default();
        let mut state = State::default();
        for modifier in [LeftAlt, RightAlt, LeftControl] {
            assert!(state.event(
                KeyEvent::new(KeyCode::Modifier(modifier), KeyModifiers::NONE),
                &bindings,
                "menu"
            ));
        }
        assert_eq!(state.modifiers(), KeyModifiers::ALT | KeyModifiers::CONTROL);
        for modifier in [LeftAlt, LeftControl] {
            state.event(
                KeyEvent::new_with_kind(
                    KeyCode::Modifier(modifier),
                    KeyModifiers::NONE,
                    KeyEventKind::Release,
                ),
                &bindings,
                "menu",
            );
        }
        assert_eq!(state.modifiers(), KeyModifiers::ALT);
        state.reset();
        assert!(state.modifiers().is_empty());
        let reveal = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::ALT | KeyModifiers::SHIFT);
        assert!(state.event(reveal, &bindings, "menu"));
        assert_eq!(state.modifiers(), KeyModifiers::ALT);
        assert!(state.event(reveal, &bindings, "menu"));
        assert!(state.modifiers().is_empty());
    }
}
