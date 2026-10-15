use crate::*;
use ratatui::{Frame, layout::Alignment, widgets::Wrap};

#[derive(Clone, Copy)]
pub(super) enum Context {
    Hosts,
    Setup,
    Connection,
    Configuration,
}
impl Context {
    fn topics(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Hosts => &[
                ("Guide", include_str!("../docs/usage.md")),
                ("Profiles", include_str!("../docs/configuration.md")),
                ("Bindings", include_str!("../docs/keybindings.md")),
            ],
            Self::Connection => &[("Connection", include_str!("../docs/connection.md"))],
            Self::Configuration => &[
                ("Profiles", include_str!("../docs/configuration.md")),
                ("Bindings", include_str!("../docs/keybindings.md")),
                ("Setup", include_str!("../docs/setup.md")),
            ],
            Self::Setup => &[("Setup", include_str!("../docs/setup.md"))],
        }
    }
}

#[derive(Clone)]
struct HelpStyle;
impl tui_markdown::StyleSheet for HelpStyle {
    fn heading(&self, _level: u8) -> Style {
        Style::default()
            .fg(theme::LAVENDER)
            .add_modifier(Modifier::BOLD)
    }
    fn heading_marker(&self, _level: u8) -> &str {
        ""
    }
    fn code(&self) -> Style {
        Style::default().fg(theme::PEACH).bg(theme::SURFACE)
    }
    fn link(&self) -> Style {
        Style::default()
            .fg(theme::LAVENDER)
            .add_modifier(Modifier::UNDERLINED)
    }
    fn code_block_fence(&self) -> &str {
        ""
    }
}

struct Help {
    bar_state: bar::State,
    controls: Vec<(String, &'static str, u8)>,
    topics: Vec<(&'static str, &'static str)>,
    reference: String,
    topic: usize,
    offsets: Vec<usize>,
    page_height: usize,
    max_offset: usize,
}
impl Help {
    fn new(context: Context, bindings: &bindings::Bindings) -> Self {
        let mut topics = context.topics().to_vec();
        topics.push(("Keys", ""));
        let scope = match context {
            Context::Hosts => "menu",
            Context::Setup | Context::Connection => "dialog",
            Context::Configuration => "editor",
        };
        let count = topics.len();
        Self {
            bar_state: bar::State::default(),
            controls: vec![
                (
                    bar::pair(bindings, "help", "previous_topic", "next_topic"),
                    "Topic",
                    3,
                ),
                (bar::pair(bindings, "help", "up", "down"), "Scroll", 2),
                (bar::plain_hint(bindings, "help", "page_down"), "Page", 4),
                (bar::plain_hint(bindings, "help", "help"), "Back", 1),
                (bar::plain_hint(bindings, "help", "close"), "Close", 0),
            ],
            topics,
            reference: bindings.reference(scope),
            topic: 0,
            offsets: vec![0; count],
            page_height: 1,
            max_offset: 0,
        }
    }
    fn draw(&mut self, frame: &mut Frame, config: Option<&launch::Config>) {
        frame.render_widget(
            Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
            frame.area(),
        );
        let areas = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());
        let version = concat!("v", env!("CARGO_PKG_VERSION"));
        let header = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(version.len() as u16 + 1),
        ])
        .split(areas[0]);
        let mut tabs = vec![Span::styled(
            " HELP ",
            Style::default()
                .fg(theme::BASE)
                .bg(theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        )];
        if header[0].width < 65 {
            tabs.push(Span::raw(format!(
                " {}/{} {}",
                self.topic + 1,
                self.topics.len(),
                self.topics[self.topic].0
            )));
        } else {
            for (i, (title, _)) in self.topics.iter().enumerate() {
                tabs.push(Span::styled(
                    format!(" {title} "),
                    if i == self.topic {
                        Style::default()
                            .fg(theme::BASE)
                            .bg(theme::LAVENDER)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::SUBTEXT)
                    },
                ));
            }
        }
        frame.render_widget(Paragraph::new(Line::from(tabs)), header[0]);
        frame.render_widget(
            Paragraph::new(version)
                .alignment(Alignment::Right)
                .style(Style::default().fg(theme::SUBTEXT)),
            header[1],
        );
        let options = tui_markdown::Options::new(HelpStyle).table_width(areas[1].width.max(1));
        let text = tui_markdown::from_str_with_options(
            if self.topics[self.topic].0 == "Keys" {
                &self.reference
            } else {
                self.topics[self.topic].1
            },
            &options,
        );
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        let total = paragraph.line_count(areas[1].width);
        self.page_height = usize::from(areas[1].height).max(1);
        self.max_offset = total
            .saturating_sub(self.page_height)
            .min(usize::from(u16::MAX));
        let offset = &mut self.offsets[self.topic];
        *offset = (*offset).min(self.max_offset);
        frame.render_widget(paragraph.scroll((*offset as u16, 0)), areas[1]);
        let position = format!(
            " {} · {}–{}/{}",
            self.topics[self.topic].0,
            (*offset + 1).min(total),
            (*offset + self.page_height).min(total),
            total
        );
        let mut items: Vec<_> = self
            .controls
            .iter()
            .map(|(key, label, priority)| bar::Item::new(key, *label, *priority))
            .collect();
        items.push(bar::Item::new("", position.trim(), 5));
        if let Some(config) = config {
            self.bar_state.draw(frame, areas[2], config, "help", items);
        } else {
            bar::draw(frame, areas[2], items);
        }
    }
}

pub(super) fn show(
    terminal: &mut DefaultTerminal,
    context: Context,
    config: &launch::Config,
) -> io::Result<()> {
    let bindings = &config.keybindings;
    let mut help = Help::new(context, bindings);
    if matches!(context, Context::Hosts) {
        help.reference
            .push_str("\n## Profile shortcuts\n\n| Profile | Key |\n| --- | --- |\n");
        for profile in &config.profiles {
            if let Some(key) = &profile.key {
                help.reference.push_str(&format!(
                    "| {} | {key} |\n",
                    profile.name.replace('|', "\\|")
                ));
            }
        }
    }
    loop {
        terminal.draw(|frame| help.draw(frame, Some(config)))?;
        let event = event::read()?;
        if matches!(event, Event::FocusLost) {
            help.bar_state.reset();
        }
        let Event::Key(key) = event else {
            continue;
        };
        if help.bar_state.event(key, bindings, "help") {
            continue;
        }
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let repeated = key.kind == KeyEventKind::Repeat;
        let Some(key) = bindings.translate("help", key) else {
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
                    | KeyCode::Home
                    | KeyCode::End
            )
        {
            continue;
        }
        match key.code {
            KeyCode::Esc | KeyCode::F(1) => return Ok(()),
            KeyCode::Char('q') if key.modifiers.is_empty() => return Ok(()),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::Right | KeyCode::Tab => help.topic = (help.topic + 1) % help.topics.len(),
            KeyCode::Left | KeyCode::BackTab => {
                help.topic = (help.topic + help.topics.len() - 1) % help.topics.len()
            }
            KeyCode::Up => help.offsets[help.topic] = help.offsets[help.topic].saturating_sub(1),
            KeyCode::Down => help.offsets[help.topic] = help.offsets[help.topic].saturating_add(1),
            KeyCode::PageUp => {
                help.offsets[help.topic] = help.offsets[help.topic].saturating_sub(help.page_height)
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                help.offsets[help.topic] = help.offsets[help.topic].saturating_add(help.page_height)
            }
            KeyCode::Home => help.offsets[help.topic] = 0,
            KeyCode::End => help.offsets[help.topic] = help.max_offset,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn documents_render_at_small_sizes_and_scroll_is_clamped() {
        for (width, height) in [(1, 1), (20, 6), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for context in [
                Context::Hosts,
                Context::Setup,
                Context::Connection,
                Context::Configuration,
            ] {
                let mut help = Help::new(context, &bindings::Bindings::default());
                for topic in 0..help.topics.len() {
                    help.topic = topic;
                    help.offsets[topic] = usize::MAX;
                    terminal.draw(|frame| help.draw(frame, None)).unwrap();
                    assert!(help.offsets[topic] <= help.max_offset);
                }
            }
        }
    }
}
