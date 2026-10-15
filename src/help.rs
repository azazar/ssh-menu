use crate::*;
use ratatui::{Frame, widgets::Wrap};

const TOPICS: &[(&str, &str)] = &[
    ("Guide", include_str!("../docs/usage.md")),
    ("Profiles", include_str!("../docs/configuration.md")),
    ("Install", include_str!("../docs/installation.md")),
    ("Setup", include_str!("../docs/setup.md")),
    ("Develop", include_str!("../docs/development.md")),
    ("Index", include_str!("../docs/README.md")),
];

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
    topic: usize,
    offsets: Vec<usize>,
    page_height: usize,
    max_offset: usize,
}
impl Help {
    fn new(topic: usize) -> Self {
        Self {
            topic: topic.min(TOPICS.len() - 1),
            offsets: vec![0; TOPICS.len()],
            page_height: 1,
            max_offset: 0,
        }
    }
    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Block::default().style(Style::default().fg(theme::TEXT).bg(theme::BASE)),
            frame.area(),
        );
        let areas = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(frame.area());
        let mut tabs = vec![Span::styled(
            " HELP ",
            Style::default()
                .fg(theme::BASE)
                .bg(theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        )];
        if frame.area().width < 65 {
            tabs.push(Span::raw(format!(
                " {}/{} {}",
                self.topic + 1,
                TOPICS.len(),
                TOPICS[self.topic].0
            )));
        } else {
            for (i, (title, _)) in TOPICS.iter().enumerate() {
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
        frame.render_widget(Paragraph::new(Line::from(tabs)), areas[0]);
        let options = tui_markdown::Options::new(HelpStyle).table_width(areas[1].width.max(1));
        let text = tui_markdown::from_str_with_options(TOPICS[self.topic].1, &options);
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
            TOPICS[self.topic].0,
            (*offset + 1).min(total),
            (*offset + self.page_height).min(total),
            total
        );
        let keys = if frame.area().width < 65 {
            " ←→ Topic · ↑↓ Scroll · F1 Close"
        } else {
            " ←→/Tab TOPIC · ↑↓/Pg SCROLL · Home/End · F1/Esc CLOSE"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(position, Style::default().fg(theme::SUBTEXT)),
                Line::styled(keys, Style::default().fg(theme::LAVENDER)),
            ]),
            areas[2],
        );
    }
}

pub(super) fn show(terminal: &mut DefaultTerminal, topic: usize) -> io::Result<()> {
    let mut help = Help::new(topic);
    loop {
        terminal.draw(|frame| help.draw(frame))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('q') => return Ok(()),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::Right | KeyCode::Tab => help.topic = (help.topic + 1) % TOPICS.len(),
            KeyCode::Left | KeyCode::BackTab => {
                help.topic = (help.topic + TOPICS.len() - 1) % TOPICS.len()
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
            for topic in 0..TOPICS.len() {
                let mut help = Help::new(topic);
                help.offsets[topic] = usize::MAX;
                terminal.draw(|frame| help.draw(frame)).unwrap();
                assert!(help.offsets[topic] <= help.max_offset);
            }
        }
    }
}
