//! The terminal boundary shared by all interactive screens.
use crate::*;
use ratatui::{Frame, layout::Size};

pub(crate) trait Terminal {
    fn draw(&mut self, draw: impl FnOnce(&mut Frame)) -> io::Result<()>;
    fn size(&self) -> io::Result<Size>;
    fn read(&mut self) -> io::Result<Event>;
    fn edit_config(&mut self, home: &Path) -> io::Result<launch::Config>;
}

impl Terminal for DefaultTerminal {
    fn draw(&mut self, draw: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.draw(draw).map(|_| ())
    }

    fn size(&self) -> io::Result<Size> {
        self.size()
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }

    fn edit_config(&mut self, home: &Path) -> io::Result<launch::Config> {
        editor::launch_editor(self, home)
    }
}
