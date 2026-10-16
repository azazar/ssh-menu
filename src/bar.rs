use crate::*;

#[derive(Clone)]
pub(super) struct Item {
    prefix: String,
    key: String,
    label: String,
    priority: u8,
    selected: bool,
    alternatives: Vec<String>,
    action: Option<String>,
    available: bool,
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
            available: true,
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
        let left = plain_hint(&config.keybindings, "menu", "left");
        let right = plain_hint(&config.keybindings, "menu", "right");
        if count < 2 || (left == "—" && right == "—") {
            let mut item = Self::new("—", "", 3);
            item.action = Some("profiles".into());
            return item;
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
        item.action = Some("profiles".into());
        item
    }
    pub fn available(mut self, available: bool) -> Self {
        self.available = available;
        self
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
    let keys = [previous, next]
        .iter()
        .filter_map(|action| {
            let keys = bindings.key_labels(scope, action);
            keys.iter()
                .find(|key| matches!(key.as_str(), "←" | "→" | "↑" | "↓"))
                .or_else(|| keys.iter().find(|key| group(key).is_empty()))
                .cloned()
        })
        .collect::<Vec<_>>();
    if keys.is_empty() {
        "—".into()
    } else {
        let separator = if keys
            .iter()
            .all(|key| matches!(key.as_str(), "←" | "→" | "↑" | "↓"))
        {
            ""
        } else {
            "/"
        };
        keys.join(separator)
    }
}

pub(super) fn plain_hint(bindings: &bindings::Bindings, scope: &str, action: &str) -> String {
    bindings
        .key_labels(scope, action)
        .into_iter()
        .find(|key| group(key).is_empty())
        .unwrap_or_else(|| "—".into())
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
        .filter(|i| i.available && i.key != "—")
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
    if key.contains("Shift+") {
        flags |= KeyModifiers::SHIFT;
    }
    flags
}

pub(super) struct State {
    held: Vec<crossterm::event::ModifierKeyCode>,
    inferred: KeyModifiers,
    latched: KeyModifiers,
    modifier_events: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            inferred: KeyModifiers::NONE,
            latched: KeyModifiers::NONE,
            modifier_events: false,
        }
    }
}
impl State {
    fn modifier_flag(modifier: crossterm::event::ModifierKeyCode) -> KeyModifiers {
        use crossterm::event::ModifierKeyCode::*;
        match modifier {
            LeftAlt | RightAlt => KeyModifiers::ALT,
            LeftControl | RightControl => KeyModifiers::CONTROL,
            LeftShift | RightShift => KeyModifiers::SHIFT,
            LeftSuper | RightSuper => KeyModifiers::SUPER,
            LeftHyper | RightHyper => KeyModifiers::HYPER,
            LeftMeta | RightMeta => KeyModifiers::META,
            IsoLevel3Shift | IsoLevel5Shift => KeyModifiers::NONE,
        }
    }
    fn held_modifiers(&self) -> KeyModifiers {
        self.held
            .iter()
            .fold(KeyModifiers::NONE, |flags, modifier| {
                flags | Self::modifier_flag(*modifier)
            })
    }
    pub fn event(
        &mut self,
        key: crossterm::event::KeyEvent,
        bindings: &bindings::Bindings,
        scope: &str,
    ) -> bool {
        if let KeyCode::Modifier(modifier) = key.code {
            self.modifier_events = true;
            self.latched = KeyModifiers::NONE;
            if key.kind == KeyEventKind::Release {
                self.held.retain(|m| *m != modifier);
            } else if !self.held.contains(&modifier) {
                self.held.push(modifier);
            }
            self.inferred = key.modifiers & !self.held_modifiers() & !Self::modifier_flag(modifier);
            return true;
        }
        let reveal = bindings
            .translate(scope, key)
            .and_then(|event| match event.code {
                KeyCode::F(21) => Some(KeyModifiers::ALT),
                KeyCode::F(22) => Some(KeyModifiers::CONTROL | KeyModifiers::ALT),
                KeyCode::F(23) => Some(KeyModifiers::CONTROL),
                KeyCode::F(24) => Some(KeyModifiers::SHIFT),
                _ => None,
            });
        if let Some(reveal) = reveal {
            if key.kind == KeyEventKind::Press {
                self.held.clear();
                self.inferred = KeyModifiers::NONE;
                self.latched = if self.latched == reveal {
                    KeyModifiers::NONE
                } else {
                    reveal
                };
            }
            // Repeats and releases must neither toggle a fallback nor reach an
            // action handler as its internal F21--F24 identifier.
            return true;
        }
        if key.kind != KeyEventKind::Release {
            self.latched = KeyModifiers::NONE;
        }
        if self.latched.is_empty() && self.modifier_events {
            self.held.retain(|modifier| {
                let flag = Self::modifier_flag(*modifier);
                flag.is_empty() || key.modifiers.contains(flag)
            });
            // A scope switch can miss modifier presses. Ordinary key events
            // still carry the current mask, including repeats and releases.
            self.inferred = key.modifiers & !self.held_modifiers();
        }
        false
    }
    pub fn reset(&mut self) {
        // Reset held keys across focus/scope changes while remembering that
        // this terminal has actually demonstrated modifier-release support.
        *self = Self {
            modifier_events: self.modifier_events,
            ..Self::default()
        };
    }
    fn modifiers(&self) -> KeyModifiers {
        self.latched | self.inferred | self.held_modifiers()
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
        use crossterm::event::ModifierKeyCode::{IsoLevel3Shift, IsoLevel5Shift};
        let level3 = self.held.contains(&IsoLevel3Shift);
        let level5 = self.held.contains(&IsoLevel5Shift);
        if active.is_empty() && !level3 && !level5 {
            items.retain_mut(|item| {
                if !item.available {
                    return false;
                }
                if !item.alternatives.is_empty() {
                    let Some(key) = item.alternatives.iter().find(|key| group(key).is_empty())
                    else {
                        return false;
                    };
                    item.key = key.clone();
                }
                group(&item.key).is_empty()
            });
        } else {
            let label = [
                (KeyModifiers::CONTROL, "Ctrl"),
                (KeyModifiers::ALT, "Alt"),
                (KeyModifiers::SHIFT, "Shift"),
                (KeyModifiers::SUPER, "Super"),
                (KeyModifiers::HYPER, "Hyper"),
                (KeyModifiers::META, "Meta"),
            ]
            .into_iter()
            .filter_map(|(flag, name)| active.contains(flag).then_some(name))
            .chain(level3.then_some("Level3"))
            .chain(level5.then_some("Level5"))
            .collect::<Vec<_>>()
            .join("+");
            if Line::from(format!(" {label} ")).width() > usize::from(area.width) {
                // Never show a bare modified shortcut when its modifier
                // heading cannot fit; it would advertise a different key.
                draw(frame, area, Vec::new());
                return;
            }
            let mut revealed = vec![Item::new("", label, 0).selected()];
            let profiles_available = !items
                .iter()
                .any(|item| item.action.as_deref() == Some("profiles") && !item.available);
            let cycle_available = profiles_available && config.profiles.len() > 1;
            for (action, keys) in config.keybindings.actions(scope) {
                if action.starts_with("reveal_")
                    || level3
                    || level5
                    || items
                        .iter()
                        .any(|item| item.action.as_deref() == Some(&action) && !item.available)
                    || (scope == "menu"
                        && !cycle_available
                        && matches!(action.as_str(), "left" | "right"))
                {
                    continue;
                }
                let keys: Vec<_> = keys
                    .into_iter()
                    .filter(|key| group(key) == active)
                    .map(|key| {
                        key.replace("Ctrl+", "")
                            .replace("Alt+", "")
                            .replace("Shift+", "")
                    })
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
                    "configure_remote" => "Setup",
                    "new_remote" => "New",
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
            if scope == "menu" && profiles_available && !level3 && !level5 {
                for profile in &config.profiles {
                    if let Some(key) = &profile.key {
                        let key = key.to_string();
                        if group(&key) == active {
                            revealed.push(Item::new(
                                key.replace("Ctrl+", "")
                                    .replace("Alt+", "")
                                    .replace("Shift+", ""),
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
        if scope == "menu" {
            // Sort the displayed keys after selecting the active modifier group.
            // Keep non-function controls in their existing positions.
            let number = |item: &Item| {
                item.key
                    .split('/')
                    .next()?
                    .strip_prefix('F')?
                    .parse::<u8>()
                    .ok()
            };
            let mut function_keys: Vec<_> = items
                .iter()
                .filter_map(|item| number(item).map(|key| (key, item.clone())))
                .collect();
            function_keys.sort_by_key(|(key, _)| *key);
            let mut sorted = function_keys.into_iter();
            for item in &mut items {
                if number(item).is_some() {
                    *item = sorted.next().unwrap().1;
                }
            }
        }
        draw(frame, area, items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, ModifierKeyCode::*};

    fn configuration(overrides: &str) -> launch::Config {
        launch::Config::parse(&format!("default: SSH\nprofiles: [{{name: SSH, key: Alt+S, command: 'ssh %h'}}, {{name: Special, key: Ctrl+Alt+X, command: 'ssh %h'}}]\n{overrides}")).unwrap()
    }

    fn rendered(
        state: &State,
        config: &launch::Config,
        scope: &str,
        width: u16,
        items: Vec<Item>,
    ) -> String {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| state.draw(frame, frame.area(), config, scope, items.clone()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .trim()
            .to_owned()
    }

    fn controls(config: &launch::Config, scope: &str) -> Vec<Item> {
        let exit = match scope {
            "menu" => "quit",
            "help" => "close",
            _ => "cancel",
        };
        vec![
            Item::action(&config.keybindings, scope, "help", "Help", 0),
            Item::action(&config.keybindings, scope, exit, "Back", 1),
            Item::action(
                &config.keybindings,
                scope,
                if scope == "editor" { "edit" } else { "connect" },
                "Open",
                2,
            ),
        ]
    }

    #[test]
    fn menu_function_keys_follow_numeric_order_in_each_modifier_group() {
        for modifiers in ["", "Shift+", "Ctrl+", "Alt+"] {
            let config = configuration(&format!(
                "keybindings:\n  menu:\n    help: [{modifiers}F12]\n    edit_config: [{modifiers}F2]\n    configure_remote: [{modifiers}F10]\n    refresh: [{modifiers}F3]\n"
            ));
            let mut state = State::default();
            if let Some(modifier) = match modifiers {
                "Shift+" => Some(LeftShift),
                "Ctrl+" => Some(LeftControl),
                "Alt+" => Some(LeftAlt),
                _ => None,
            } {
                state.event(
                    KeyEvent::new(KeyCode::Modifier(modifier), KeyModifiers::NONE),
                    &config.keybindings,
                    "menu",
                );
            }
            let items = [
                ("connect", "Open"),
                ("help", "Help"),
                ("configure_remote", "Setup"),
                ("edit_config", "Config"),
                ("refresh", "Refresh"),
                ("quit", "Quit"),
            ]
            .into_iter()
            .map(|(action, label)| Item::action(&config.keybindings, "menu", action, label, 1))
            .collect();
            let text = rendered(&state, &config, "menu", 240, items);
            let positions: Vec<_> = ["F2 Config", "F3 Refresh", "F10 Setup", "F12 Help"]
                .map(|label| {
                    text.find(label)
                        .unwrap_or_else(|| panic!("{label}: {text}"))
                })
                .into();
            assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
            if modifiers.is_empty() {
                assert!(text.starts_with("↵ Open"), "{text}");
                assert!(text.ends_with("Esc Quit"), "{text}");
            }
        }
    }

    #[test]
    fn rendered_bars_follow_complete_modifier_transitions_in_each_scope() {
        let config = configuration("");
        for scope in ["menu", "dialog", "help", "editor"] {
            let mut state = State::default();
            let normal = rendered(&state, &config, scope, 160, controls(&config, scope));
            assert!(
                normal.contains("F1 Help") && normal.contains("Esc Back"),
                "{scope}: {normal}"
            );
            for (modifier, kind, mask, heading) in [
                (LeftAlt, KeyEventKind::Press, KeyModifiers::ALT, "Alt"),
                (RightAlt, KeyEventKind::Press, KeyModifiers::ALT, "Alt"),
                (RightAlt, KeyEventKind::Repeat, KeyModifiers::ALT, "Alt"),
                (
                    LeftControl,
                    KeyEventKind::Press,
                    KeyModifiers::ALT | KeyModifiers::CONTROL,
                    "Ctrl+Alt",
                ),
                (
                    LeftAlt,
                    KeyEventKind::Release,
                    KeyModifiers::ALT | KeyModifiers::CONTROL,
                    "Ctrl+Alt",
                ),
                (
                    RightAlt,
                    KeyEventKind::Release,
                    KeyModifiers::CONTROL,
                    "Ctrl",
                ),
                (
                    LeftShift,
                    KeyEventKind::Press,
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                    "Ctrl+Shift",
                ),
                (
                    LeftControl,
                    KeyEventKind::Release,
                    KeyModifiers::SHIFT,
                    "Shift",
                ),
            ] {
                assert!(state.event(
                    KeyEvent::new_with_kind(KeyCode::Modifier(modifier), mask, kind),
                    &config.keybindings,
                    scope
                ));
                let text = rendered(&state, &config, scope, 160, controls(&config, scope));
                assert!(text.starts_with(&format!("{heading} ")), "{scope}: {text}");
                assert!(
                    !text.contains("F1 Help") && !text.contains("Esc Back"),
                    "{scope}: {text}"
                );
                assert_eq!(
                    text.contains("S SSH"),
                    scope == "menu" && heading == "Alt",
                    "{scope}: {text}"
                );
                assert_eq!(
                    text.contains("X Special"),
                    scope == "menu" && heading == "Ctrl+Alt",
                    "{scope}: {text}"
                );
            }
            state.event(
                KeyEvent::new_with_kind(
                    KeyCode::Modifier(LeftShift),
                    KeyModifiers::NONE,
                    KeyEventKind::Release,
                ),
                &config.keybindings,
                scope,
            );
            assert_eq!(
                rendered(&state, &config, scope, 160, controls(&config, scope)),
                normal
            );

            // Recover current modifiers after entering a new scope or missing
            // a modifier press, and recover stale state on repeated input.
            state.reset();
            state.event(
                KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::CONTROL, KeyEventKind::Repeat),
                &config.keybindings,
                scope,
            );
            assert!(
                rendered(&state, &config, scope, 160, controls(&config, scope))
                    .starts_with("Ctrl ")
            );
            state.event(
                KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat),
                &config.keybindings,
                scope,
            );
            assert_eq!(
                rendered(&state, &config, scope, 160, controls(&config, scope)),
                normal
            );
            state.event(
                KeyEvent::new(KeyCode::Modifier(LeftAlt), KeyModifiers::ALT),
                &config.keybindings,
                scope,
            );
            state.reset(); // Focus loss and scope transitions use this same reset.
            assert_eq!(
                rendered(&state, &config, scope, 160, controls(&config, scope)),
                normal
            );
        }
    }

    #[test]
    fn fallback_remaps_toggle_once_and_keep_context_labels() {
        for scope in ["menu", "dialog", "help", "editor"] {
            let config = configuration(&format!(
                "keybindings: {{{scope}: {{reveal_alt: [F2], help: [Alt+F3]}}}}"
            ));
            let mut state = State::default();
            let items = vec![Item::action(
                &config.keybindings,
                scope,
                "help",
                "Context help",
                0,
            )];
            let fallback = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
            assert!(state.event(fallback, &config.keybindings, scope));
            let revealed = rendered(&state, &config, scope, 160, items.clone());
            assert!(revealed.contains("F3 Context help"));
            for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
                assert!(state.event(
                    KeyEvent::new_with_kind(KeyCode::F(2), KeyModifiers::NONE, kind),
                    &config.keybindings,
                    scope
                ));
                assert_eq!(
                    rendered(&state, &config, scope, 160, items.clone()),
                    revealed
                );
            }
            assert!(state.event(fallback, &config.keybindings, scope));
            assert_eq!(rendered(&state, &config, scope, 160, items.clone()), "");
            assert!(state.event(fallback, &config.keybindings, scope));
            state.event(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                &config.keybindings,
                scope,
            );
            assert_eq!(rendered(&state, &config, scope, 160, items), "");
        }
    }

    #[test]
    fn legacy_key_masks_do_not_leave_a_modifier_menu_stuck() {
        let config = configuration("");
        for scope in ["menu", "dialog", "help", "editor"] {
            let mut state = State::default();
            let normal = rendered(&state, &config, scope, 160, controls(&config, scope));
            for key in [
                KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT),
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT),
                KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
            ] {
                state.event(key, &config.keybindings, scope);
                assert_eq!(
                    rendered(&state, &config, scope, 160, controls(&config, scope)),
                    normal
                );
            }
            state.event(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::ALT | KeyModifiers::SHIFT),
                &config.keybindings,
                scope,
            );
            assert!(
                rendered(&state, &config, scope, 160, controls(&config, scope)).starts_with("Alt ")
            );
            state.event(
                KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT),
                &config.keybindings,
                scope,
            );
            assert_eq!(
                rendered(&state, &config, scope, 160, controls(&config, scope)),
                normal
            );
        }
    }

    #[test]
    fn extra_modifiers_never_advertise_or_activate_a_smaller_group() {
        let config = configuration("keybindings: {menu: {help: [Ctrl+Q]}}");
        for (modifier, flag, name) in [
            (LeftShift, KeyModifiers::SHIFT, "Shift"),
            (LeftSuper, KeyModifiers::SUPER, "Super"),
            (LeftHyper, KeyModifiers::HYPER, "Hyper"),
            (LeftMeta, KeyModifiers::META, "Meta"),
        ] {
            let mut state = State::default();
            state.event(
                KeyEvent::new(KeyCode::Modifier(LeftAlt), KeyModifiers::ALT),
                &config.keybindings,
                "menu",
            );
            state.event(
                KeyEvent::new(KeyCode::Modifier(modifier), KeyModifiers::ALT | flag),
                &config.keybindings,
                "menu",
            );
            let text = rendered(&state, &config, "menu", 160, controls(&config, "menu"));
            assert_eq!(text, format!("Alt+{name}  No shortcuts"));
            let event = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::ALT | flag);
            assert!(!config.profiles[0].key.as_ref().unwrap().matches(event));
            assert!(config.keybindings.translate("menu", event).is_none());
            assert!(
                config
                    .keybindings
                    .translate(
                        "menu",
                        KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::CONTROL | flag)
                    )
                    .is_none()
            );
        }
        for modifier in [IsoLevel3Shift, IsoLevel5Shift] {
            let mut state = State::default();
            state.event(
                KeyEvent::new(KeyCode::Modifier(modifier), KeyModifiers::NONE),
                &config.keybindings,
                "menu",
            );
            assert!(
                rendered(&state, &config, "menu", 160, controls(&config, "menu"))
                    .ends_with("No shortcuts")
            );
        }
    }

    #[test]
    fn narrow_and_unavailable_bars_cannot_advertise_unusable_keys() {
        let config =
            configuration("keybindings: {menu: {left: [Ctrl+Alt+Left], right: [Alt+Right]}}");
        let mut state = State::default();
        state.event(
            KeyEvent::new(
                KeyCode::Char('?'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            &config.keybindings,
            "menu",
        );
        for width in 0..40 {
            let text = rendered(&state, &config, "menu", width, controls(&config, "menu"));
            if width < 10 {
                assert!(text.is_empty(), "width {width}: {text}");
            } else {
                assert!(text.starts_with("Ctrl+Alt"), "width {width}: {text}");
            }
        }
        let items = vec![
            Item::profiles(&config, 0).available(false),
            Item::action(&config.keybindings, "menu", "connect", "Open", 1).available(false),
            Item::action(
                &config.keybindings,
                "menu",
                "connect_and_return",
                "Return",
                1,
            )
            .available(false),
            Item::action(
                &config.keybindings,
                "menu",
                "connection_settings",
                "Edit",
                1,
            )
            .available(false),
            Item::action(&config.keybindings, "menu", "help", "Help", 0),
        ];
        for flags in [
            KeyModifiers::NONE,
            KeyModifiers::ALT,
            KeyModifiers::CONTROL,
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ] {
            let state = State {
                latched: flags,
                ..State::default()
            };
            let text = rendered(&state, &config, "menu", 200, items.clone());
            for unavailable in [
                "S SSH",
                "X Special",
                "Open",
                "Return",
                "Edit",
                "Previous",
                "Next",
            ] {
                assert!(!text.contains(unavailable), "{flags:?}: {text}");
            }
        }
        let config =
            configuration("keybindings: {dialog: {previous_field: [F2], next_field: [F3]}}");
        assert_eq!(
            pair(
                &config.keybindings,
                "dialog",
                "previous_field",
                "next_field"
            ),
            "F2/F3"
        );
        let config = launch::Config::parse("default: SSH\nprofiles: [{name: SSH, key: Alt+S, command: 'ssh %h'}]\nkeybindings: {menu: {left: [Alt+Left], right: [Alt+Right]}}\n").unwrap();
        let state = State {
            latched: KeyModifiers::ALT,
            ..State::default()
        };
        let text = rendered(
            &state,
            &config,
            "menu",
            160,
            vec![Item::profiles(&config, 0)],
        );
        assert!(
            text.contains("S SSH"),
            "a single available profile must remain launchable: {text}"
        );
        assert!(
            !text.contains("Previous") && !text.contains("Next"),
            "a single profile cannot cycle: {text}"
        );
    }
    #[test]
    fn bars_show_only_the_exact_modifier_group_in_every_screen() {
        use ratatui::{Terminal, backend::TestBackend};
        let variants = [
            (KeyModifiers::NONE, "F10"),
            (KeyModifiers::SHIFT, "Shift+F11"),
            (KeyModifiers::ALT, "Alt+F12"),
            (KeyModifiers::CONTROL, "Ctrl+F13"),
            (KeyModifiers::CONTROL | KeyModifiers::ALT, "Ctrl+Alt+F14"),
            (KeyModifiers::ALT | KeyModifiers::SHIFT, "Alt+Shift+F15"),
            (
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                "Ctrl+Shift+F16",
            ),
            (
                KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
                "Ctrl+Alt+Shift+F17",
            ),
        ];
        for scope in ["menu", "dialog", "help", "editor"] {
            let config = launch::Config::parse(&format!(
                "default: SSH\nprofiles: [{{name: SSH, key: Alt+S, command: 'ssh %h'}}, {{name: Special, key: Ctrl+Alt+X, command: 'ssh %h'}}]\nkeybindings:\n  {scope}:\n    help: [{}]\n",
                variants.iter().map(|(_, key)| *key).collect::<Vec<_>>().join(", ")
            )).unwrap();
            for (active, expected) in variants {
                let mut terminal = Terminal::new(TestBackend::new(400, 1)).unwrap();
                let state = State {
                    held: Vec::new(),
                    latched: active,
                    ..State::default()
                };
                terminal
                    .draw(|frame| {
                        state.draw(
                            frame,
                            frame.area(),
                            &config,
                            scope,
                            vec![Item::action(&config.keybindings, scope, "help", "Help", 0)],
                        )
                    })
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                if active.is_empty() {
                    assert_eq!(text.trim(), "F10 Help", "{scope}: {text}");
                }
                assert_eq!(
                    text.contains("SSH"),
                    scope == "menu" && active == KeyModifiers::ALT
                );
                assert_eq!(
                    text.contains("Special"),
                    scope == "menu" && active == (KeyModifiers::CONTROL | KeyModifiers::ALT)
                );
                for (_, candidate) in variants {
                    let key = candidate.rsplit('+').next().unwrap();
                    assert_eq!(
                        text.contains(key),
                        candidate == expected,
                        "{scope} {active:?}: {text}"
                    );
                }
            }
        }
        let config = launch::Config::parse("default: SSH\nprofiles: [{name: SSH, command: 'ssh %h'}, {name: Other, command: 'ssh %h'}]\nkeybindings: {menu: {left: [Shift+Left], right: [Right]}, dialog: {previous_field: [Shift+Tab]}}\n").unwrap();
        assert_eq!(pair(&config.keybindings, "menu", "left", "right"), "→");
        assert_eq!(
            pair(
                &config.keybindings,
                "dialog",
                "previous_field",
                "next_field"
            ),
            "↓"
        );
        let item = Item::profiles(&config, 0);
        assert_eq!(item.key, "→");
        assert!(item.prefix.is_empty());
        assert_eq!(item.label, "Other");
    }

    #[test]
    fn shift_release_and_fallback_restore_the_right_group() {
        let bindings = bindings::Bindings::default();
        let mut state = State::default();
        for modifier in [LeftShift, RightShift, LeftControl] {
            state.event(
                KeyEvent::new(KeyCode::Modifier(modifier), KeyModifiers::NONE),
                &bindings,
                "dialog",
            );
        }
        assert_eq!(
            state.modifiers(),
            KeyModifiers::SHIFT | KeyModifiers::CONTROL
        );
        for (modifier, expected) in [
            (LeftShift, KeyModifiers::SHIFT | KeyModifiers::CONTROL),
            (LeftControl, KeyModifiers::SHIFT),
            (RightShift, KeyModifiers::NONE),
        ] {
            state.event(
                KeyEvent::new_with_kind(
                    KeyCode::Modifier(modifier),
                    KeyModifiers::NONE,
                    KeyEventKind::Release,
                ),
                &bindings,
                "dialog",
            );
            assert_eq!(state.modifiers(), expected);
        }
        let reveal = KeyEvent::new(KeyCode::F(1), KeyModifiers::SHIFT);
        assert!(state.event(reveal, &bindings, "dialog"));
        assert_eq!(state.modifiers(), KeyModifiers::SHIFT);
        assert!(state.event(reveal, &bindings, "dialog"));
        assert!(state.modifiers().is_empty());
        state.event(
            KeyEvent::new(KeyCode::Modifier(LeftShift), KeyModifiers::SHIFT),
            &bindings,
            "dialog",
        );
        state.event(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &bindings,
            "dialog",
        );
        assert!(
            state.modifiers().is_empty(),
            "stale Shift state survived an unmodified key"
        );
    }

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
