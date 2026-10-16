//! Build-time configuration composition, also exercised by the application tests.
use serde_yaml_ng::Value;

pub const SHARED: &str = include_str!("../docs/config.example.yml");
pub const WINDOWS: &str = include_str!("../docs/config.windows.yml");
pub const WINDOWS_OPTIONAL: &str = include_str!("../docs/config.optional.windows.yml");
pub const OPTIONAL: &str = include_str!("../docs/config.optional.yml");

fn merge(base: &mut Value, overlay: Value, named: bool) {
    match (base, overlay) {
        (Value::Mapping(base), Value::Mapping(overlay)) => {
            for (key, value) in overlay {
                if value.is_null() {
                    base.remove(&key);
                    continue;
                }
                let named = matches!(key.as_str(), Some("profiles" | "configuration_scripts"));
                match base.get_mut(&key) {
                    Some(existing) => merge(existing, value, named),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (Value::Sequence(base), Value::Sequence(overlay)) if named => {
            for value in overlay {
                let name = value.get("name").and_then(Value::as_str);
                if let Some(existing) = base.iter_mut().find(|entry| {
                    name.is_some() && entry.get("name").and_then(Value::as_str) == name
                }) {
                    merge(existing, value, false);
                } else {
                    base.push(value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

pub fn compose(parts: &[&str]) -> Result<String, serde_yaml_ng::Error> {
    let mut config = Value::Mapping(Default::default());
    for part in parts {
        merge(&mut config, serde_yaml_ng::from_str(part)?, false);
    }
    serde_yaml_ng::to_string(&config)
}

pub fn defaults(windows: bool, optional: bool) -> Result<String, serde_yaml_ng::Error> {
    let mut parts = vec![SHARED];
    if windows {
        parts.push(WINDOWS);
    }
    if optional {
        parts.push(OPTIONAL);
        if windows {
            parts.push(WINDOWS_OPTIONAL);
        }
    }
    compose(&parts)
}

fn optional_examples(windows: bool) -> Result<String, serde_yaml_ng::Error> {
    if windows {
        compose(&[OPTIONAL, WINDOWS_OPTIONAL])
    } else {
        Ok(OPTIONAL.into())
    }
}

pub fn user_config(windows: bool) -> Result<String, serde_yaml_ng::Error> {
    let mut config = format!(
        "# Shui config: F9 edits and reloads. Existing files are preserved on upgrade.\n# Reference: https://github.com/azazar/shui/blob/master/docs/configuration.md\n{}\n# Optional examples: uncomment desired entries and merge into the sections above.\n# Do not add a second profiles or configuration_scripts section.\n",
        defaults(windows, false)?
    );
    let examples = optional_examples(windows)?;
    for line in examples.lines() {
        config.push_str("# ");
        config.push_str(line);
        config.push('\n');
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Target, launch::Config};

    #[test]
    fn user_config_preserves_tested_examples_as_inactive_comments() {
        for windows in [false, true] {
            let text = user_config(windows).unwrap();
            let active: Value = serde_yaml_ng::from_str(&text).unwrap();
            let expected: Value =
                serde_yaml_ng::from_str(&defaults(windows, false).unwrap()).unwrap();
            assert_eq!(active, expected);
            let examples = optional_examples(windows).unwrap();
            let commented = examples
                .lines()
                .map(|line| format!("# {line}\n"))
                .collect::<String>();
            assert!(text.ends_with(&commented));
            let restored = text[text.len() - commented.len()..]
                .lines()
                .map(|line| format!("{}\n", line.strip_prefix("# ").unwrap()))
                .collect::<String>();
            // Git may check out source YAML with CRLF on Windows.
            // Rendering deliberately uses LF; all line contents must survive.
            assert_eq!(restored, examples.replace("\r\n", "\n"));
            Config::parse(&compose(&[&defaults(windows, false).unwrap(), &restored]).unwrap())
                .unwrap();
        }
        assert_eq!(
            crate::launch::DEFAULT_CONFIG,
            user_config(cfg!(windows)).unwrap()
        );
    }

    #[test]
    fn every_platform_and_optional_composition_validates_and_renders() {
        for windows in [false, true] {
            for optional in [false, true] {
                let config = Config::parse(&defaults(windows, optional).unwrap()).unwrap();
                for name in ["PuTTY", "Plink"] {
                    assert_eq!(config.profiles.iter().any(|p| p.name == name), windows);
                }
                assert_eq!(config.profiles.iter().any(|p| p.name == "Work"), optional);
                for profile in config.profiles.iter().chain(&config.configuration_scripts) {
                    let mut profile = profile.clone();
                    // Supply generated pick values without inspecting the host's keys.
                    for input in &mut profile.inputs {
                        if input.command.is_some()
                            && input.kind == crate::launch::InputType::PickString
                        {
                            input.options = vec!["fixture".into()];
                            input.default = Some("fixture".into());
                        }
                    }
                    let mut inputs = profile.input_defaults();
                    for input in &profile.inputs {
                        if input.required && inputs.get(&input.id).is_none_or(String::is_empty) {
                            inputs.insert(input.id.clone(), "o'hara@example.org".into());
                        }
                    }
                    for port in [None, Some("2222".into())] {
                        profile
                            .command_with_inputs(
                                &Target {
                                    host: "alice@host".into(),
                                    port,
                                },
                                &inputs,
                            )
                            .unwrap_or_else(|e| panic!("{}: {e}", profile.name));
                    }
                }
            }
        }
        let embedded: Value = serde_yaml_ng::from_str(crate::launch::DEFAULT_CONFIG).unwrap();
        let expected: Value =
            serde_yaml_ng::from_str(&defaults(cfg!(windows), false).unwrap()).unwrap();
        assert_eq!(embedded, expected);
    }

    #[test]
    fn overrides_preserve_unmentioned_fields_and_order_but_replace_lists() {
        let source = compose(&[SHARED, "profiles:\n  - name: SSH\n    command: custom %h\n  - name: Extra\n    command: extra %h\nkeybindings:\n  menu:\n    quit: []\n"]).unwrap();
        let config = Config::parse(&source).unwrap();
        assert_eq!(config.profiles[0].name, "SSH");
        assert_eq!(config.profiles[0].command, "custom %h");
        assert!(config.profiles[0].available_command.is_some());
        assert!(config.profiles[0].key.is_some());
        assert_eq!(config.profiles.last().unwrap().name, "Extra");
        let merged = compose(&[
            "keybindings: {menu: {quit: [Esc], help: [F1]}}",
            "keybindings: {menu: {quit: []}}",
        ])
        .unwrap();
        let value: Value = serde_yaml_ng::from_str(&merged).unwrap();
        assert_eq!(
            value["keybindings"]["menu"]["quit"],
            Value::Sequence(vec![])
        );
        assert_eq!(value["keybindings"]["menu"]["help"][0].as_str(), Some("F1"));
    }
}
