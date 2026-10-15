use crate::Target;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
    path::Path,
};

pub const CONFIG_NAME: &str = "ssh-menu-config.yml";
pub const DEFAULT_CONFIG: &str = include_str!("../ssh-menu-config.example.yml");
const HISTORY_TARGET: &str = " # ssh-menu-target=";
const SCRIPT_PREFIX: &str = "exec sh -c \"$(printf '%b' ";
const SCRIPT_SUFFIX: &str = ")\"";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum InputType {
    #[serde(rename = "promptString")]
    PromptString,
    #[serde(rename = "pickString")]
    PickString,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: InputType,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub command: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedInput {
    description: Option<String>,
    default: Option<String>,
    options: Option<Vec<String>>,
}

impl Input {
    pub fn resolve(&self) -> io::Result<Self> {
        let Some(command) = &self.command else {
            return Ok(self.clone());
        };
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(std::process::Stdio::null())
            .output()?;
        if !output.status.success() {
            let message: String = String::from_utf8_lossy(&output.stderr)
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect();
            return Err(invalid(format!(
                "{}: generator failed ({}): {}",
                self.label(),
                output.status,
                message
            )));
        }
        if output.stdout.len() > 65536 {
            return Err(invalid("input generator output exceeds 64 KiB"));
        }
        let generated: GeneratedInput = serde_yaml_ng::from_slice(&output.stdout)
            .map_err(|e| invalid(format!("{}: invalid generated input: {e}", self.label())))?;
        let mut resolved = self.clone();
        resolved.command = None;
        if let Some(value) = generated.description {
            resolved.description = value;
        }
        if let Some(value) = generated.default {
            resolved.default = Some(value);
        }
        if let Some(value) = generated.options {
            resolved.options = value;
        }
        resolved.validate()?;
        Ok(resolved)
    }
    pub fn initial(&self) -> String {
        self.default
            .clone()
            .unwrap_or_else(|| self.options.first().cloned().unwrap_or_default())
    }
    pub fn label(&self) -> &str {
        if self.description.is_empty() {
            &self.id
        } else {
            &self.description
        }
    }
    fn validate(&self) -> io::Result<()> {
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
        {
            return Err(invalid(
                "input id must contain only letters, digits, _ or -",
            ));
        }
        if self.description.chars().any(char::is_control)
            || self
                .default
                .iter()
                .chain(&self.options)
                .any(|s| s.chars().any(char::is_control))
        {
            return Err(invalid(
                "input labels and values must not contain control characters",
            ));
        }
        if self
            .command
            .as_ref()
            .is_some_and(|c| c.trim().is_empty() || c.contains('\0'))
        {
            return Err(invalid("input command must be nonempty and contain no NUL"));
        }
        match self.kind {
            InputType::PickString
                if self.command.is_none()
                    && (self.options.is_empty()
                        || self
                            .default
                            .as_ref()
                            .is_some_and(|v| !self.options.contains(v))) =>
            {
                return Err(invalid(
                    "pickString needs options and its default must be one of them",
                ));
            }
            InputType::PromptString if !self.options.is_empty() => {
                return Err(invalid("options are only supported for pickString"));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub key: Option<crate::shortcut::Shortcut>,
    #[serde(default)]
    pub available_command: Option<String>,
    pub command: String,
    #[serde(default)]
    pub inputs: Vec<Input>,
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
}
impl Profile {
    pub fn is_available(&self) -> bool {
        let Some(command) = &self.available_command else {
            return true;
        };
        std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    fn expand(
        &self,
        text: &str,
        target: &Target,
        stack: &mut Vec<String>,
        short: bool,
        inputs: &HashMap<String, String>,
    ) -> io::Result<String> {
        // Leaf fields (notably scripts) are literal: shell %, $, quotes, etc. are preserved.
        if !short && !text.contains("%{") {
            return Ok(text.into());
        }
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut quote = None;
        while let Some(c) = chars.next() {
            if c == '\\' && quote != Some('\'') {
                result.push(c);
                if let Some(next) = chars.next() {
                    result.push(next);
                }
                continue;
            }
            if matches!(c, '\'' | '"') {
                if quote == Some(c) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(c);
                }
            }
            if c != '%' {
                result.push(c);
                continue;
            }
            let value = match chars.peek().copied() {
                Some('%') => {
                    chars.next();
                    result.push('%');
                    continue;
                }
                Some('h' | 'p' | 't') if short => match chars.next().unwrap() {
                    'h' => target.host.clone(),
                    'p' => target.port.clone().unwrap_or_default(),
                    _ => target.label(),
                },
                Some('{') => {
                    chars.next();
                    let mut key = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => key.push(c),
                            None => return Err(invalid("unclosed %{field} placeholder")),
                        }
                    }
                    if let Some(id) = key.strip_prefix("input:") {
                        inputs
                            .get(id)
                            .ok_or_else(|| invalid(format!("unknown input: {id}")))?
                            .clone()
                    } else {
                        if stack.len() >= 16 || stack.contains(&key) {
                            return Err(invalid(format!(
                                "cyclic or too deeply nested field: {key}"
                            )));
                        }
                        let field = self
                            .fields
                            .get(&key)
                            .ok_or_else(|| invalid(format!("unknown field: {key}")))?;
                        stack.push(key);
                        let value = self.expand(field, target, stack, false, inputs)?;
                        stack.pop();
                        value
                    }
                }
                _ => {
                    result.push('%');
                    continue;
                }
            };
            if quote.is_some() {
                return Err(invalid(
                    "placeholders must be unquoted; they are shell-escaped automatically",
                ));
            }
            result.push_str(&shell_words::quote(&value));
            if result.len() > 65536 {
                return Err(invalid("expanded command exceeds 64 KiB"));
            }
        }
        if short && quote.is_some() {
            return Err(invalid("unclosed quote in command"));
        }
        Ok(result)
    }
    pub fn expanded(&self, target: &Target) -> io::Result<String> {
        self.expand(
            &self.command,
            target,
            &mut Vec::new(),
            true,
            &self.input_defaults(),
        )
    }
    pub fn input_defaults(&self) -> HashMap<String, String> {
        self.inputs
            .iter()
            .map(|i| (i.id.clone(), i.initial()))
            .collect()
    }
    pub fn command_with_inputs(
        &self,
        target: &Target,
        values: &HashMap<String, String>,
    ) -> io::Result<(String, Vec<String>)> {
        for input in &self.inputs {
            let value = values
                .get(&input.id)
                .ok_or_else(|| invalid(format!("missing input: {}", input.id)))?;
            if (input.required && value.trim().is_empty())
                || value.chars().any(char::is_control)
                || (input.kind == InputType::PickString && !input.options.contains(value))
            {
                return Err(invalid(format!("invalid value for {}", input.label())));
            }
        }
        Ok(Self::shell_command(self.expand(
            &self.command,
            target,
            &mut Vec::new(),
            true,
            values,
        )?))
    }
    pub fn program(&self) -> String {
        let words = shell_words::split(&self.command).unwrap_or_default();
        let i = words.iter().position(|s| s == "exec").map_or(0, |i| i + 1);
        words.get(i).cloned().unwrap_or_default()
    }
    pub fn command(&self, target: &Target) -> (String, Vec<String>) {
        let command = self.expanded(target).expect("validated profile");
        Self::shell_command(command)
    }
    fn shell_command(command: String) -> (String, Vec<String>) {
        // Keep scripts on one history line without sending the script through terminal stdin.
        let code = if command.contains(['\n', '\r']) {
            let escaped = command
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            format!(
                "{SCRIPT_PREFIX}{}{SCRIPT_SUFFIX}",
                shell_words::quote(&escaped)
            )
        } else {
            command
        };
        ("sh".into(), vec!["-c".into(), code])
    }
    pub fn history_line(&self, target: &Target) -> String {
        let (program, args) = self.command(target);
        // The target hint lets generic shell commands rediscover the entry without executing them.
        // Profile selection is still inferred from the command; no profile/state ID is stored.
        format!(
            "exec {}{HISTORY_TARGET}{}",
            shell_words::join(std::iter::once(program).chain(args)),
            target.label()
        )
    }
}

pub fn history_target_hint(line: &str) -> Option<&str> {
    line.rsplit_once(HISTORY_TARGET).map(|(_, target)| target)
}

pub fn history_words(line: &str) -> Option<Vec<String>> {
    let mut code = line
        .rsplit_once(HISTORY_TARGET)
        .map_or(line, |(command, _)| command)
        .to_owned();
    for _ in 0..4 {
        let mut words = shell_words::split(&code).ok()?;
        if words.first().is_some_and(|s| s == "exec") {
            words.remove(0);
        }
        if words.len() == 3 && words[0] == "sh" && words[1] == "-c" {
            code = words[2].clone();
            if let Some(encoded) = code
                .strip_prefix(SCRIPT_PREFIX)
                .and_then(|s| s.strip_suffix(SCRIPT_SUFFIX))
            {
                let pieces = shell_words::split(encoded).ok()?;
                if pieces.len() != 1 {
                    return None;
                }
                let mut chars = pieces[0].chars();
                let mut decoded = String::new();
                while let Some(c) = chars.next() {
                    if c != '\\' {
                        decoded.push(c);
                        continue;
                    }
                    decoded.push(match chars.next()? {
                        'n' => '\n',
                        'r' => '\r',
                        '\\' => '\\',
                        _ => return None,
                    });
                }
                code = decoded;
            }
        } else {
            return Some(words);
        }
    }
    None
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub default: String,
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub configuration_scripts: Vec<Profile>,
}
impl Config {
    pub fn parse(text: &str) -> io::Result<Self> {
        let config: Self = serde_yaml_ng::from_str(text).map_err(|e| invalid(e.to_string()))?;
        if config.profiles.is_empty() {
            return Err(invalid("profiles must not be empty"));
        }
        if config.profiles.iter().any(|p| !p.inputs.is_empty()) {
            return Err(invalid(
                "inputs are supported only in configuration_scripts",
            ));
        }
        if config.configuration_scripts.iter().any(|p| p.key.is_some()) {
            return Err(invalid("key is supported only in launch profiles"));
        }
        let mut keys = HashSet::new();
        for profile in &config.profiles {
            if let Some(key) = &profile.key
                && !keys.insert(key)
            {
                return Err(invalid(format!("duplicate profile key: {key}")));
            }
        }
        for group in [&config.profiles, &config.configuration_scripts] {
            let mut names = HashSet::new();
            for profile in group {
                if profile
                    .available_command
                    .as_ref()
                    .is_some_and(|c| c.trim().is_empty() || c.contains('\0'))
                {
                    return Err(invalid(
                        "available_command must be nonempty and contain no NUL",
                    ));
                }
                let mut ids = HashSet::new();
                for input in &profile.inputs {
                    input.validate()?;
                    if !ids.insert(&input.id) {
                        return Err(invalid(format!("duplicate input id: {}", input.id)));
                    }
                }
                if profile.name.trim().is_empty()
                    || profile.name.chars().any(char::is_control)
                    || !names.insert(&profile.name)
                {
                    return Err(invalid(
                        "profile names must be nonempty, unique and contain no control characters",
                    ));
                }
                if profile.command.trim().is_empty()
                    || profile.command.contains('\0')
                    || profile.fields.values().any(|v| v.contains('\0'))
                {
                    return Err(invalid(
                        "command must be nonempty; fields must not contain NUL",
                    ));
                }
                for port in [None, Some("2222".into())] {
                    profile
                        .expanded(&Target {
                            host: "user@host".into(),
                            port,
                        })
                        .map_err(|e| invalid(format!("profile {}: {e}", profile.name)))?;
                }
            }
        }
        if !config.profiles.iter().any(|p| p.name == config.default) {
            return Err(invalid("default must name an existing profile"));
        }
        Ok(config)
    }
    fn available_profiles(mut self) -> io::Result<Self> {
        self.profiles.retain(Profile::is_available);
        let first = self.profiles.first().ok_or_else(|| {
            invalid("no launch profiles are available; check available_command and installed tools")
        })?;
        if !self
            .profiles
            .iter()
            .any(|profile| profile.name == self.default)
        {
            self.default = first.name.clone();
        }
        Ok(self)
    }
    pub fn default_index(&self) -> usize {
        self.profiles
            .iter()
            .position(|p| p.name == self.default)
            .expect("validated default")
    }
    pub fn toggle_transport(&self, current: usize) -> usize {
        let program = if self.profiles[current].program() == "mosh" {
            "ssh"
        } else {
            "mosh"
        };
        self.profiles
            .iter()
            .position(|p| p.program() == program && p.fields.is_empty())
            .unwrap_or(current)
    }
    pub fn history_profiles(&self, targets: &[Target], history: &str, user: &str) -> Vec<usize> {
        let mut selected = vec![self.default_index(); targets.len()];
        let indices: HashMap<_, _> = targets
            .iter()
            .enumerate()
            .map(|(i, t)| (t.label(), i))
            .collect();
        let mut invocations: HashMap<Vec<String>, Option<(usize, usize)>> = HashMap::new();
        for (i, target) in targets.iter().enumerate() {
            for (p, profile) in self.profiles.iter().enumerate() {
                let Some(words) = history_words(&profile.history_line(target)) else {
                    continue;
                };
                invocations
                    .entry(words)
                    .and_modify(|v| {
                        if v.is_some_and(|(host, _)| host != i) {
                            *v = None;
                        }
                    })
                    .or_insert(Some((i, p)));
            }
        }
        for line in history.lines() {
            let Some(words) = history_words(line) else {
                continue;
            };
            if let Some(Some((i, p))) = invocations.get(&words) {
                selected[*i] = *p;
                continue;
            }
            if let Some(hint) = history_target_hint(line) {
                if let Some(i) = indices.get(hint) {
                    selected[*i] = self.default_index();
                }
                continue;
            }
            let Some((target, remote, program)) = crate::history_connection(line, user) else {
                continue;
            };
            let Some(i) = indices.get(&target.label()) else {
                continue;
            };
            if !matches!(program.as_str(), "ssh" | "mosh") {
                continue;
            }
            selected[*i] = self
                .profiles
                .iter()
                .position(|profile| {
                    if profile.program() != program {
                        return false;
                    }
                    if remote.is_empty() {
                        return profile.fields.is_empty()
                            && profile.command.trim_end().ends_with("%h");
                    }
                    // Conventional named remote command supports ordinary manually entered SSH history.
                    profile.fields.get("remote").is_some_and(|field| {
                        profile
                            .expand(field, &target, &mut Vec::new(), false, &HashMap::new())
                            .is_ok_and(|expected| remote.join(" ") == expected)
                    })
                })
                .unwrap_or(self.default_index());
        }
        selected
    }
    pub fn load(home: &Path) -> io::Result<Self> {
        let dir = home.join(".ssh");
        if !dir.is_dir() {
            return Self::parse(DEFAULT_CONFIG)?.available_profiles();
        }
        let path = dir.join(CONFIG_NAME);
        let load = || -> io::Result<Self> {
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    let mut options = fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    match options.open(&path) {
                        Ok(mut file) => {
                            file.write_all(DEFAULT_CONFIG.as_bytes())?;
                            DEFAULT_CONFIG.into()
                        }
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                            fs::read_to_string(&path)?
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => return Err(e),
            };
            Self::parse(&text)?.available_profiles()
        };
        load().map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn target(port: Option<&str>) -> Target {
        Target {
            host: "alice@host".into(),
            port: port.map(str::to_owned),
        }
    }
    fn run(profile: &Profile, target: &Target) -> std::process::Output {
        let (program, args) = profile.command(target);
        Command::new(program).args(args).output().unwrap()
    }
    #[test]
    fn minimal_profiles_and_literal_shell_substitution() {
        let config = Config::parse("default: Test\nprofiles:\n  - name: Test\n    command: printf '%s\\n' /sh://%h %{value}\n    value: \"a b ' \\\" $(printf INJECTED) ; $HOME %h\"\n").unwrap();
        let out = run(&config.profiles[0], &target(None));
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            "/sh://alice@host\na b ' \" $(printf INJECTED) ; $HOME %h\n"
        );
        Config::parse("default: SSH\nprofiles: [{name: SSH, command: 'ssh %h'}, {name: MC, command: 'mc /sh://%h'}]").unwrap();
    }
    #[test]
    fn nested_fields_quote_each_shell_layer_and_preserve_script() {
        let config = Config::parse("default: Test\nprofiles:\n  - name: Test\n    command: sh -c %{remote}\n    remote: sh -c %{script}\n    script: |\n      printf '%s\\n' \"spaces ' quotes\"\n      printf '%s\\n' '${literal%% *}'\n").unwrap();
        let out = run(&config.profiles[0], &target(None));
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            "spaces ' quotes\n${literal%% *}\n"
        );
        let line = config.profiles[0].history_line(&target(None));
        assert!(!line.contains('\n'));
        assert_eq!(config.history_profiles(&[target(None)], &line, "me"), [0]);
    }
    #[test]
    fn optional_ports_use_ordinary_shell_syntax() {
        let config = Config::parse("default: Test\nprofiles:\n  - name: Test\n    command: port=%p; printf '%s\\n' ${port:+-p \"$port\"} -- %h\n").unwrap();
        assert_eq!(
            String::from_utf8(run(&config.profiles[0], &target(None)).stdout).unwrap(),
            "--\nalice@host\n"
        );
        assert_eq!(
            String::from_utf8(run(&config.profiles[0], &target(Some("2222"))).stdout).unwrap(),
            "-p\n2222\n--\nalice@host\n"
        );
    }
    #[test]
    fn zellij_script_and_history_round_trip() {
        let config = Config::parse(DEFAULT_CONFIG).unwrap();
        assert_eq!(
            config.profiles[4].fields["script"],
            include_str!("../scripts/zellij-remote.sh")
        );
        for (index, profile) in config.profiles.iter().enumerate() {
            let target = target(Some("2222"));
            let line = profile.history_line(&target);
            assert!(!line.contains('\n'));
            assert_eq!(
                config.history_profiles(std::slice::from_ref(&target), &line, "me"),
                [index]
            );
            assert_eq!(
                crate::history_target(&line, "me").unwrap().label(),
                target.label()
            );
        }
    }
    #[test]
    fn history_matches_each_entry_and_ignores_unconfigured_commands() {
        let config = Config::parse(DEFAULT_CONFIG).unwrap();
        let targets = vec![target(None), target(Some("2222"))];
        assert_eq!(
            config.history_profiles(
                &targets,
                "mosh alice@host\nssh -p2222 alice@host exec bash -l\n",
                "me"
            ),
            [1, 3]
        );
        let line = config.profiles[2].history_line(&targets[1]);
        assert_eq!(config.history_profiles(&targets, &line, "me"), [0, 2]);
        let line = format!("{line}\nexec sh -c 'unknown' # ssh-menu-target=alice@host:2222");
        assert_eq!(config.history_profiles(&targets, &line, "me"), [0, 0]);
    }
    #[test]
    fn invalid_templates_fail_without_executing() {
        for profile in [
            "{name: X, command: ''}",
            "{name: X, command: 'echo %{missing}'}",
            "{name: X, command: 'echo %{a}', a: '%{b}', b: '%{a}'}",
            "{name: X, command: 'echo %{unclosed'}",
            "{name: X, command: 'echo \"%h\"'}",
            "{name: X, command: 'echo ok', value: [not, a, string]}",
        ] {
            assert!(
                Config::parse(&format!("default: X\nprofiles: [{profile}]\n")).is_err(),
                "{profile}"
            );
        }
    }
    #[test]
    fn inputs_are_literal_across_nested_shell_layers() {
        let config = Config::parse(
            r#"
default: SSH
profiles: [{name: SSH, command: 'ssh %h'}]
configuration_scripts:
  - name: Install
    command: sh -c %{remote}
    remote: sh -c %{script} -- %{input:argument} %{input:mode}
    script: printf '%s\n' "$1" "$2"
    inputs:
      - id: argument
        type: promptString
        required: true
      - id: mode
        type: pickString
        options: [minimal, full]
"#,
        )
        .unwrap();
        let profile = &config.configuration_scripts[0];
        let mut values = profile.input_defaults();
        assert!(profile.command_with_inputs(&target(None), &values).is_err());
        let literal = "spaces ' \" ; $(printf INJECTED) %{input:mode} %h";
        values.insert("argument".into(), literal.into());
        values.insert("mode".into(), "full".into());
        let (program, args) = profile.command_with_inputs(&target(None), &values).unwrap();
        let output = Command::new(program).args(args).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{literal}\nfull\n")
        );
        values.insert("mode".into(), "not-an-option".into());
        assert!(profile.command_with_inputs(&target(None), &values).is_err());
    }

    #[test]
    fn rejects_invalid_input_definitions_and_references() {
        for definition in [
            "{id: x, type: unsupported}",
            "{id: x, type: pickString, options: []}",
            "{id: x, type: pickString, options: [one], default: two}",
            "{id: x, type: promptString, options: [one]}",
            "{id: 'bad:id', type: promptString}",
            "{id: x, type: promptString, default: [invalid]}",
            "{id: x, type: promptString}, {id: x, type: promptString}",
        ] {
            assert!(Config::parse(&format!("default: SSH\nprofiles: [{{name: SSH, command: 'ssh %h'}}]\nconfiguration_scripts: [{{name: Setup, command: 'echo ok', inputs: [{definition}]}}]")).is_err(), "{definition}");
        }
        for command in [
            "echo %{input:missing}",
            "echo \"%{input:x}\"",
            "echo %{input:x",
        ] {
            let yaml = format!(
                "default: SSH\nprofiles: [{{name: SSH, command: 'ssh %h'}}]\nconfiguration_scripts:\n  - name: Setup\n    command: '{command}'\n    inputs: [{{id: x, type: promptString}}]\n"
            );
            assert!(Config::parse(&yaml).is_err(), "{command}");
        }
    }

    #[test]
    fn generated_input_configuration_is_validated() {
        let input: Input = serde_yaml_ng::from_str(
            r#"
id: generated
type: pickString
command: printf '%s' '{"options":["one","two"],"default":"two","description":"Generated"}'
"#,
        )
        .unwrap();
        input.validate().unwrap();
        let resolved = input.resolve().unwrap();
        assert_eq!(resolved.options, ["one", "two"]);
        assert_eq!(resolved.initial(), "two");
        assert_eq!(resolved.label(), "Generated");
        let mut prompt = input.clone();
        prompt.kind = InputType::PromptString;
        prompt.command = Some("printf 'default: generated text\\n'".into());
        assert_eq!(prompt.resolve().unwrap().initial(), "generated text");
        for command in [
            "exit 7",
            "printf 'not a mapping'",
            "printf 'options: []'",
            "printf 'options: [one]\\ndefault: two'",
            "printf 'type: promptString'",
        ] {
            let mut invalid = input.clone();
            invalid.command = Some(command.into());
            invalid.validate().unwrap(); // Loading definitions never runs commands.
            assert!(invalid.resolve().is_err(), "{command}");
        }
    }

    #[test]
    fn key_generator_lists_pairs_and_quotes_filenames() {
        let home = std::env::temp_dir().join(format!("ssh-menu-key-test-{}", std::process::id()));
        fs::create_dir_all(home.join(".ssh")).unwrap();
        for name in [
            "id_ed25519",
            "id_ed25519.pub",
            "id_work's key",
            "id_work's key.pub",
            "id_orphan.pub",
            "id_private_only",
            "other",
            "other.pub",
        ] {
            fs::write(home.join(".ssh").join(name), "fixture").unwrap();
        }
        let config = Config::parse(DEFAULT_CONFIG).unwrap();
        let mut input = config.configuration_scripts[0].inputs[0].clone();
        input.command = Some(format!(
            "HOME={}; export HOME; {}",
            shell_words::quote(home.to_str().unwrap()),
            input.command.unwrap()
        ));
        assert_eq!(
            input.resolve().unwrap().options,
            ["id_ed25519.pub", "id_work's key.pub"]
        );
        fs::remove_file(home.join(".ssh/id_ed25519")).unwrap();
        fs::remove_file(home.join(".ssh/id_work's key")).unwrap();
        assert!(input.resolve().is_err());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejects_conflicting_or_misplaced_profile_keys() {
        assert!(Config::parse("default: A\nprofiles: [{name: A, command: 'ssh %h', key: Alt+M}, {name: B, command: 'mosh %h', key: alt+m, available_command: 'exit 1'}]").is_err());
        assert!(Config::parse("default: A\nprofiles: [{name: A, command: 'ssh %h'}]\nconfiguration_scripts: [{name: Setup, command: 'true', key: Alt+M}]").is_err());
        for key in ["M", "Ctrl+C", "Alt+Enter", "Ctrl+Alt+H"] {
            assert!(
                Config::parse(&format!(
                    "default: A\nprofiles: [{{name: A, command: 'ssh %h', key: '{key}'}}]"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn profile_availability_preserves_order_and_available_default() {
        let config = Config::parse(
            "default: Preferred\nprofiles:\n  - {name: First, command: 'ssh %h'}\n  - {name: Missing, command: 'mc %h', available_command: 'exit 1'}\n  - {name: Preferred, command: 'mosh %h', available_command: 'exit 0'}\n",
        ).unwrap();
        assert_eq!(config.profiles.len(), 3, "parsing must not run checks");
        let available = config.available_profiles().unwrap();
        assert_eq!(
            available
                .profiles
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["First", "Preferred"]
        );
        assert_eq!(available.default_index(), 1);
        for command in ["''", "' '", "\"\\0\""] {
            assert!(Config::parse(&format!("default: X\nprofiles: [{{name: X, command: 'ssh %h', available_command: {command}}}]\n")).is_err());
        }
    }

    #[test]
    fn creates_config_only_in_existing_ssh_directory() {
        let home =
            std::env::temp_dir().join(format!("ssh-menu-template-test-{}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        Config::load(&home).unwrap();
        assert!(!home.join(".ssh").exists());
        fs::create_dir(home.join(".ssh")).unwrap();
        Config::load(&home).unwrap();
        let path = home.join(".ssh").join(CONFIG_NAME);
        assert_eq!(fs::read_to_string(&path).unwrap(), DEFAULT_CONFIG);
        let custom = "default: Custom\nprofiles: [{name: Custom, command: 'ssh %h'}]\n";
        fs::write(&path, custom).unwrap();
        assert_eq!(Config::load(&home).unwrap().default, "Custom");
        assert_eq!(fs::read_to_string(&path).unwrap(), custom);
        fs::remove_dir_all(home).unwrap();
    }
}
