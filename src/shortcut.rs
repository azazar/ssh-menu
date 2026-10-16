use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(try_from = "String")]
pub struct Shortcut {
    letter: char,
    modifiers: KeyModifiers,
}

impl TryFrom<String> for Shortcut {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let invalid = || {
            format!(
                "invalid profile key {value:?}: use Ctrl+letter, Alt+letter, or Ctrl+Alt+letter"
            )
        };
        let normalized = value.trim().to_ascii_lowercase();
        let mut parts: Vec<_> = normalized.split('+').map(str::trim).collect();
        let letter = parts.pop().ok_or_else(invalid)?;
        if letter.len() != 1 || !letter.as_bytes()[0].is_ascii_alphabetic() {
            return Err(invalid());
        }
        let mut modifiers = KeyModifiers::NONE;
        for part in parts {
            let modifier = match part {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                _ => return Err(invalid()),
            };
            if modifiers.contains(modifier) {
                return Err(invalid());
            }
            modifiers.insert(modifier);
        }
        if modifiers.is_empty() {
            return Err(invalid());
        }
        let letter = letter.chars().next().unwrap();
        // Include terminal aliases (Tab, Enter, Backspace) and flow/suspend keys.
        if modifiers.contains(KeyModifiers::CONTROL) && "hijmsz".contains(letter) {
            return Err(format!(
                "profile key {value:?} is reserved by the app or terminal"
            ));
        }
        Ok(Self { letter, modifiers })
    }
}

impl Shortcut {
    pub fn matches(&self, event: KeyEvent) -> bool {
        let KeyCode::Char(letter) = event.code else {
            return false;
        };
        event.modifiers == self.modifiers && letter.to_ascii_lowercase() == self.letter
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            write!(f, "Alt+")?;
        }
        write!(f, "{}", self.letter.to_ascii_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_letters_match_without_intercepting_typing() {
        for (text, modifiers) in [
            ("Alt+M", KeyModifiers::ALT),
            ("Ctrl+P", KeyModifiers::CONTROL),
            ("Ctrl+Q", KeyModifiers::CONTROL),
            ("Ctrl+Alt+Q", KeyModifiers::CONTROL | KeyModifiers::ALT),
            ("Ctrl+Alt+P", KeyModifiers::CONTROL | KeyModifiers::ALT),
        ] {
            let key = Shortcut::try_from(text.to_owned()).unwrap();
            assert!(key.matches(KeyEvent::new(KeyCode::Char(key.letter), modifiers)));
            assert!(key.matches(KeyEvent::new(
                KeyCode::Char(key.letter.to_ascii_uppercase()),
                modifiers
            )));
            assert!(!key.matches(KeyEvent::new(
                KeyCode::Char(key.letter.to_ascii_uppercase()),
                modifiers | KeyModifiers::SHIFT
            )));
            assert!(!key.matches(KeyEvent::new(KeyCode::Char(key.letter), KeyModifiers::NONE)));
            assert!(!key.matches(KeyEvent::new(KeyCode::Enter, modifiers)));
        }
        for text in [
            "M",
            "F1",
            "Ctrl+F1",
            "Alt+Enter",
            "Shift+M",
            "Ctrl+Ctrl+P",
            "Ctrl+",
            "Ctrl+é",
            "Ctrl+H",
            "Ctrl+I",
            "Ctrl+J",
            "Ctrl+M",
            "Ctrl+S",
            "Ctrl+Z",
        ] {
            assert!(Shortcut::try_from(text.to_owned()).is_err(), "{text}");
        }
        assert_eq!(
            Shortcut::try_from("alt+ctrl+p".to_owned()).unwrap(),
            Shortcut::try_from("Ctrl+Alt+P".to_owned()).unwrap()
        );
    }
}
