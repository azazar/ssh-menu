//! Read SSH aliases and their source configuration without running SSH or a shell.
//!
//! Connections still use the original alias: OpenSSH owns the effective settings,
//! including dynamic Match conditions, canonicalization and system configuration.

use crate::{Target, target, valid_host};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_DEPTH: usize = 16;
const MAX_FILES: usize = 256;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ALIASES: usize = 4096;
const MAX_HINT_BYTES: usize = 64 * 1024;
const MAX_TOTAL_HINT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Config {
    pub targets: Vec<Target>,
    pub hints: HashMap<String, String>,
    pub users: HashMap<String, String>,
}

#[derive(Default)]
struct Document {
    blocks: Vec<Block>,
}

#[derive(Default)]
struct Block {
    selector: Selector,
    entries: Vec<Entry>,
}

#[derive(Default)]
enum Selector {
    #[default]
    Global,
    Host(Vec<String>),
    Match(Vec<String>),
}

struct Entry {
    text: String,
    directive: Option<(String, Vec<String>)>,
    includes: Vec<Document>,
    include_unknown: bool,
}

impl Entry {
    fn new(line: &str) -> Self {
        // Source configuration is displayed in the terminal. Keep tabs readable
        // and discard controls before either parsing or rendering the text.
        let text: String = line
            .replace('\t', "    ")
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let code = without_comment(text.trim());
        let directive = code
            .split_once(|c: char| c.is_whitespace() || c == '=')
            .and_then(|(key, value)| {
                let value = value.trim_start();
                let value = value.strip_prefix('=').unwrap_or(value).trim_start();
                shell_words::split(value)
                    .ok()
                    .map(|values| (key.to_ascii_lowercase(), values))
            });
        let include_unknown = directive.as_ref().is_some_and(|(key, _)| key == "include");
        Self {
            text,
            directive,
            includes: Vec::new(),
            include_unknown,
        }
    }
}

impl Document {
    fn parse(text: &str) -> Self {
        let mut document = Self::default();
        let mut block = Block::default();
        let mut pending: Vec<Entry> = Vec::new();
        for line in text.lines() {
            let entry = Entry::new(line);
            if entry.text.trim().is_empty() || entry.text.trim_start().starts_with('#') {
                // A less-indented heading following an indented trailing note
                // belongs to the next Host; retain the preceding note here.
                if entry.text.trim_start().starts_with('#')
                    && matches!(block.selector, Selector::Host(_))
                    && let Some(previous) = pending.iter().rev().find(|e| !e.text.trim().is_empty())
                    && indentation(&previous.text) > indentation(&entry.text)
                {
                    block.entries.append(&mut pending);
                }
                pending.push(entry);
                continue;
            }
            let selector = match &entry.directive {
                Some((key, values)) if key == "host" => Some(Selector::Host(values.clone())),
                Some((key, values)) if key == "match" => Some(Selector::Match(values.clone())),
                _ => None,
            };
            if let Some(selector) = selector {
                document.blocks.push(block);
                let first = pending
                    .iter()
                    .position(|entry| !entry.text.trim().is_empty())
                    .unwrap_or(pending.len());
                pending.drain(..first);
                block = Block {
                    selector,
                    entries: std::mem::take(&mut pending),
                };
            } else {
                block.entries.append(&mut pending);
            }
            block.entries.push(entry);
        }
        block.entries.append(&mut pending);
        document.blocks.push(block);
        document
    }

    fn aliases(&self, aliases: &mut Vec<String>, seen: &mut HashSet<String>) {
        for block in &self.blocks {
            if let Selector::Host(patterns) = &block.selector {
                for alias in patterns {
                    if aliases.len() < MAX_ALIASES
                        && !alias.contains(['*', '?', '!'])
                        && !alias.contains('@')
                        && valid_host(alias)
                        && seen.insert(alias.clone())
                    {
                        aliases.push(alias.clone());
                    }
                }
            }
            for entry in &block.entries {
                for child in &entry.includes {
                    child.aliases(aliases, seen);
                }
            }
        }
    }

    fn inspect(&self, alias: &str, inherited: Activity, view: &mut View) {
        for block in &self.blocks {
            let active = match &block.selector {
                Selector::Global => inherited,
                Selector::Host(patterns) => inherited.and(host_matches(patterns, alias)),
                Selector::Match(values) if values.len() == 1 && values[0] == "all" => inherited,
                Selector::Match(_) => inherited.unknown(),
            };
            if active == Activity::No {
                continue;
            }
            if let Selector::Host(patterns) = &block.selector
                && patterns.iter().any(|pattern| pattern == alias)
            {
                view.declared = true;
            }
            // Match blocks are intentionally not presented as applicable source
            // settings. Evaluating them may depend on external state or execute
            // local commands; OpenSSH handles them when the connection starts.
            let show = active == Activity::Yes && !matches!(block.selector, Selector::Match(_));
            let mut source = String::new();
            for entry in &block.entries {
                if show && view.remaining > 0 {
                    let mut length = entry.text.len().min(view.remaining.saturating_sub(1));
                    while !entry.text.is_char_boundary(length) {
                        length -= 1;
                    }
                    source.push_str(&entry.text[..length]);
                    source.push('\n');
                    view.remaining -= length + 1;
                }
                if let Some((key, values)) = &entry.directive
                    && key == "user"
                    && view.login.is_none()
                    && let Some(user) = values.first()
                {
                    view.login = Some(if active == Activity::Yes {
                        Some(user.clone())
                    } else {
                        None
                    });
                }
                if entry.include_unknown && view.login.is_none() {
                    view.login = Some(None);
                }
                if !entry.includes.is_empty() {
                    view.push_source(&mut source);
                    for child in &entry.includes {
                        child.inspect(alias, active, view);
                    }
                }
            }
            view.push_source(&mut source);
        }
    }
}

fn indentation(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Yes,
    No,
    Unknown,
}

impl Activity {
    fn and(self, matches: bool) -> Self {
        if matches { self } else { Self::No }
    }

    fn unknown(self) -> Self {
        if self == Self::No {
            self
        } else {
            Self::Unknown
        }
    }
}

#[derive(Default)]
struct View {
    declared: bool,
    // Outer None: no User encountered. Some(None): an unevaluated Match may
    // choose the user. Some(Some(user)): first static User, before any ambiguity.
    login: Option<Option<String>>,
    sources: Vec<String>,
    remaining: usize,
}

impl View {
    fn push_source(&mut self, text: &mut String) {
        if !text.trim().is_empty() {
            if !self.sources.is_empty() {
                self.remaining = self.remaining.saturating_sub(2);
            }
            self.sources.push(text.trim_end().to_owned());
        }
        text.clear();
    }
}

impl Config {
    fn from_document(document: Document, user: &str) -> Self {
        let mut aliases = Vec::new();
        document.aliases(&mut aliases, &mut HashSet::new());
        let mut result = Self::default();
        let mut hint_budget = MAX_TOTAL_HINT_BYTES;
        for alias in aliases {
            let mut view = View {
                remaining: MAX_HINT_BYTES.min(hint_budget / 2),
                ..View::default()
            };
            document.inspect(&alias, Activity::Yes, &mut view);
            if !view.declared {
                continue;
            }
            let connection = Target {
                host: alias.clone(),
                port: None,
            };
            // Without an explicit user, system-wide SSH configuration can still
            // override the local account. Only a proven User is safe to omit.
            if let Some(Some(login)) = &view.login
                && !login.is_empty()
            {
                result.users.insert(alias.clone(), login.clone());
            }
            let login = match view.login {
                Some(Some(login)) => Some(login),
                None => Some(user.to_owned()),
                Some(None) => None,
            }
            .filter(|login| !login.is_empty());
            let source = view.sources.join("\n\n");
            if !source.is_empty() {
                hint_budget = hint_budget.saturating_sub(source.len());
                result.hints.insert(connection.host.clone(), source.clone());
                if let Some(login) = login
                    && let Some(explicit) = target(&format!("{login}@{alias}"), user)
                {
                    hint_budget = hint_budget.saturating_sub(source.len());
                    result.hints.insert(explicit.host, source);
                }
            }
            result.targets.push(connection);
        }
        result
    }
}

pub(super) fn normalize_target(target: &Target, users: &HashMap<String, String>) -> Target {
    if let Some((user, alias)) = target.host.rsplit_once('@')
        && users
            .get(alias)
            .is_some_and(|configured| configured == user)
    {
        Target {
            host: alias.to_owned(),
            port: target.port.clone(),
        }
    } else {
        target.clone()
    }
}

struct Loader<'a> {
    home: &'a Path,
    stack: HashSet<PathBuf>,
    files: usize,
    remaining: usize,
}

impl Loader<'_> {
    fn read(&mut self, path: &Path, depth: usize) -> Option<Document> {
        if depth >= MAX_DEPTH || self.files >= MAX_FILES || self.remaining == 0 {
            return None;
        }
        let canonical = path.canonicalize().ok()?;
        // Avoid blocking on FIFOs/devices and detect cycles through symlinks too.
        if self.stack.contains(&canonical) || !canonical.metadata().ok()?.is_file() {
            return None;
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::fcntl::OFlag::O_NONBLOCK.bits());
        }
        let file = options.open(&canonical).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        self.files += 1;
        let mut bytes = Vec::new();
        file.take(self.remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > self.remaining {
            self.remaining = 0;
            return None;
        }
        self.remaining -= bytes.len();
        let text = String::from_utf8(bytes).ok()?;
        self.stack.insert(canonical.clone());
        let mut document = Document::parse(&text);
        'includes: for block in &mut document.blocks {
            for entry in &mut block.entries {
                if self.files >= MAX_FILES || self.remaining == 0 {
                    break 'includes;
                }
                if let Some((key, paths)) = &entry.directive
                    && key == "include"
                {
                    entry.include_unknown = false;
                    for pattern in paths {
                        let Some(pattern) = self.include_path(pattern) else {
                            entry.include_unknown = true;
                            continue;
                        };
                        let Some(pattern) = pattern.to_str() else {
                            entry.include_unknown = true;
                            continue;
                        };
                        // OpenSSH uses ordinary glob(3), where consecutive stars
                        // stay within one path component. Rust glob's ** would
                        // recursively walk whole directory trees instead.
                        let mut previous_star = false;
                        let pattern: String = pattern
                            .chars()
                            .filter(|character| {
                                let repeated = *character == '*' && previous_star;
                                previous_star = *character == '*';
                                !repeated
                            })
                            .collect();
                        let options = glob::MatchOptions {
                            require_literal_separator: true,
                            require_literal_leading_dot: true,
                            ..glob::MatchOptions::new()
                        };
                        if let Ok(matches) = glob::glob_with(&pattern, options) {
                            for path in matches.take(MAX_FILES) {
                                let Ok(path) = path else {
                                    entry.include_unknown = true;
                                    continue;
                                };
                                if self.files >= MAX_FILES || self.remaining == 0 {
                                    entry.include_unknown = true;
                                    break 'includes;
                                }
                                if let Some(child) = self.read(&path, depth + 1) {
                                    entry.includes.push(child);
                                } else {
                                    entry.include_unknown = true;
                                }
                            }
                        } else {
                            entry.include_unknown = true;
                        }
                    }
                }
            }
        }
        self.stack.remove(&canonical);
        Some(document)
    }

    fn include_path(&self, pattern: &str) -> Option<PathBuf> {
        // Host-dependent tokens and other users' homes require OpenSSH's runtime
        // context. Skip them rather than invent a different expansion.
        if pattern.contains('%') || pattern.contains('$') {
            return None;
        }
        if let Some(relative) = pattern.strip_prefix("~/") {
            return Some(self.home.join(relative));
        }
        if pattern.starts_with('~') {
            return None;
        }
        let path = Path::new(pattern);
        Some(if path.is_absolute() {
            path.to_owned()
        } else {
            self.home.join(".ssh").join(path)
        })
    }
}

pub(super) fn load(home: &Path, user: &str) -> Config {
    let mut loader = Loader {
        home,
        stack: HashSet::new(),
        files: 0,
        remaining: MAX_BYTES,
    };
    loader
        .read(&home.join(".ssh/config"), 0)
        .map(|document| Config::from_document(document, user))
        .unwrap_or_default()
}

#[cfg(test)]
pub(super) fn parse(text: &str, user: &str) -> Config {
    Config::from_document(Document::parse(text), user)
}

// SSH host patterns support * and ? (not shell character classes).
fn host_matches(patterns: &[String], alias: &str) -> bool {
    let mut matched = false;
    for pattern in patterns {
        let (negative, pattern) = pattern
            .strip_prefix('!')
            .map_or((false, pattern.as_str()), |pattern| (true, pattern));
        if wildcard_match(pattern.as_bytes(), alias.as_bytes()) {
            if negative {
                return false;
            }
            matched = true;
        }
    }
    matched
}

fn wildcard_match(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t, mut star, mut retry) = (0, 0, None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = t;
        } else if let Some(previous) = star {
            p = previous + 1;
            retry += 1;
            t = retry;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

fn without_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        match (quote, ch) {
            (None, '#') => return &line[..index],
            (None, '\'' | '"') => quote = Some(ch),
            (Some(open), close) if open == close => quote = None,
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let home = std::env::temp_dir().join(format!(
                "shui-ssh-config-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(home.join(".ssh")).unwrap();
            Self(home)
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(".ssh").join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn labels(config: &Config) -> Vec<String> {
        config.targets.iter().map(Target::label).collect()
    }

    #[test]
    fn aliases_keep_ssh_user_resolution_and_hints_support_implicit_users() {
        let config = parse(
            "# Production\nHoSt = \"prod\" p # aliases\n User = \"deploy\"\n User ignored\n IdentityFile \"~/.ssh/key#work\" # key\nHost implicit\n HostName 192.0.2.3\nHost prod\n User later\n ServerAliveInterval 30\nHost * !implicit\n User fallback\n",
            "local",
        );
        assert_eq!(labels(&config), ["prod", "p", "implicit"]);
        assert!(config.hints["prod"].starts_with("# Production\nHoSt = \"prod\" p"));
        assert!(config.hints["prod"].contains("key#work"));
        assert!(config.hints["prod"].contains("ServerAliveInterval 30"));
        assert!(!config.hints["p"].contains("ServerAliveInterval 30"));
        assert_eq!(config.hints["deploy@prod"], config.hints["prod"]);
        assert!(!config.hints.contains_key("ignored@prod"));
        assert!(!config.hints.contains_key("later@prod"));
        assert!(!config.hints.contains_key("root@prod"));
        assert!(config.hints["implicit"].contains("192.0.2.3"));
        assert!(!config.hints["implicit"].contains("User fallback"));
    }

    #[test]
    fn first_user_applies_across_matching_blocks_and_global_defaults() {
        let config = parse(
            "User first\nHost one\n User second\nHost two\nHost *\n User third\n",
            "local",
        );
        assert!(config.hints.contains_key("first@one"));
        assert!(config.hints.contains_key("first@two"));
        assert!(!config.hints.contains_key("second@one"));
        let config = parse("Host one two\nHost * !two\n User fallback\n", "local");
        assert!(config.hints.contains_key("fallback@one"));
        assert!(!config.hints.contains_key("fallback@two"));
    }

    #[test]
    fn normalize_only_the_proven_configured_user_and_preserve_ports() {
        let config = parse(
            "Host production\n User deploy\n User ignored\nHost implicit\n",
            "local",
        );
        assert_eq!(config.users["production"], "deploy");
        assert!(!config.users.contains_key("implicit"));
        for (input, expected) in [
            ("deploy@production", "production"),
            ("deploy@production:2222", "production:2222"),
            ("local@production", "local@production"),
            ("root@production", "root@production"),
            ("local@implicit", "local@implicit"),
            ("local@unknown", "local@unknown"),
            ("production", "production"),
        ] {
            let connection = target(input, "").unwrap();
            assert_eq!(
                normalize_target(&connection, &config.users).label(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn normalize_keeps_explicit_users_when_match_can_choose_the_account() {
        let config = parse(
            "Host certain\n User deploy\nMatch exec \"some-local-command\"\n User dynamic\nHost uncertain\n User fallback\nHost implicit\n",
            "local",
        );
        assert_eq!(config.users["certain"], "deploy");
        assert!(!config.users.contains_key("uncertain"));
        assert!(!config.users.contains_key("implicit"));
        for input in ["fallback@uncertain", "dynamic@uncertain", "local@implicit"] {
            let connection = target(input, "").unwrap();
            assert_eq!(normalize_target(&connection, &config.users), connection);
        }
        assert!(!parse("Host implicit\n", "").users.contains_key("implicit"));
    }

    #[test]
    fn normalize_does_not_guess_users_after_unexpanded_includes() {
        let fixture = Fixture::new();
        fixture.write(
            "config",
            "Host certain\n User deploy\nHost *\n Include ${HOME}/dynamic.conf\nHost uncertain\n User fallback\n",
        );
        let config = load(&fixture.0, "local");
        assert_eq!(config.users["certain"], "deploy");
        assert!(!config.users.contains_key("uncertain"));
        let connection = target("fallback@uncertain:2222", "").unwrap();
        assert_eq!(normalize_target(&connection, &config.users), connection);

        fixture.write("config", "Include missing-*\nHost known\n User deploy\n");
        assert_eq!(load(&fixture.0, "local").users["known"], "deploy");
    }

    #[test]
    fn match_conditions_do_not_leak_into_host_blocks_or_run_commands() {
        let config = parse(
            "Host first\n User alice\nMatch exec \"touch should-never-exist\"\n User unexpected\nHost second\n HostName 192.0.2.2\nMatch all\n User fallback\n",
            "local",
        );
        assert_eq!(labels(&config), ["first", "second"]);
        assert!(config.hints.contains_key("alice@first"));
        assert!(!config.hints.contains_key("unexpected@first"));
        assert!(!config.hints.contains_key("fallback@second"));
        assert!(!config.hints["first"].contains("Match"));
        assert!(!config.hints["second"].contains("unexpected"));
    }

    #[test]
    fn includes_expand_in_lexical_order_restore_context_and_stop_cycles() {
        let fixture = Fixture::new();
        fixture.write(
            "config",
            "Include config.d/*.conf\nHost parent\n Include shared-settings\n User later\n HostName parent.example\nHost last\n Include config\n",
        );
        fixture.write("config.d/20-second.conf", "Host second\n User bob\n");
        fixture.write(
            "config.d/10-first.conf",
            "# First included host\nHost first\n User alice\n Include nested.conf\n",
        );
        fixture.write("nested.conf", " Port 2222\n");
        fixture.write(
            "shared-settings",
            "User deploy\nHost unrelated\nUser other\n",
        );
        let config = load(&fixture.0, "local");
        assert_eq!(labels(&config), ["first", "second", "parent", "last"]);
        assert!(config.hints["first"].contains("# First included host"));
        assert!(config.hints["first"].contains("Port 2222"));
        assert_eq!(config.hints["alice@first"], config.hints["first"]);
        assert!(config.hints.contains_key("deploy@parent"));
        assert!(!config.hints.contains_key("later@parent"));
        assert!(!config.hints["last"].contains("User other"));
    }

    #[test]
    fn missing_includes_home_paths_and_non_regular_files_are_safe() {
        let fixture = Fixture::new();
        fixture.write(
            "config",
            "Include missing* config.d ~/extra.conf\nHost literal\n",
        );
        fs::create_dir_all(fixture.0.join(".ssh/config.d")).unwrap();
        fs::write(fixture.0.join("extra.conf"), "Host from-home\n").unwrap();
        let config = load(&fixture.0, "local");
        assert_eq!(labels(&config), ["from-home", "literal"]);
    }

    #[test]
    fn include_double_star_does_not_recursively_walk_directories() {
        let fixture = Fixture::new();
        fixture.write("config", "Include **/*.conf\n");
        fixture.write("config.d/host.conf", "Host included\n");
        fixture.write("config.d/nested/host.conf", "Host too-deep\n");
        assert_eq!(labels(&load(&fixture.0, "local")), ["included"]);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_cycles_and_named_pipes_do_not_block_discovery() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("config", "Include cycle pipe\nHost reachable\n");
        symlink("config", fixture.0.join(".ssh/cycle")).unwrap();
        nix::unistd::mkfifo(
            &fixture.0.join(".ssh/pipe"),
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert_eq!(labels(&load(&fixture.0, "local")), ["reachable"]);
    }

    #[test]
    fn hint_memory_is_bounded_without_losing_discovered_aliases() {
        let aliases = (0..256)
            .map(|index| format!("host-{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let config = parse(
            &format!("Host {aliases}\n # {}\n", "x".repeat(MAX_HINT_BYTES * 2)),
            "local",
        );
        assert_eq!(config.targets.len(), 256);
        assert!(config.hints.values().map(String::len).sum::<usize>() <= MAX_TOTAL_HINT_BYTES);
        assert!(
            config
                .hints
                .values()
                .all(|hint| hint.len() <= MAX_HINT_BYTES)
        );
    }

    #[test]
    fn recursive_depth_and_total_bytes_are_bounded() {
        let fixture = Fixture::new();
        fixture.write("config", "Include level-0\nHost start\n");
        for depth in 0..MAX_DEPTH + 2 {
            fixture.write(
                &format!("level-{depth}"),
                &format!("Include level-{}\nHost level-{depth}\n", depth + 1),
            );
        }
        let config = load(&fixture.0, "local");
        assert_eq!(config.targets.len(), MAX_DEPTH);
        fixture.write("config", &"#".repeat(MAX_BYTES + 1));
        assert!(load(&fixture.0, "local").targets.is_empty());
    }

    #[test]
    fn ssh_patterns_handle_negation_and_literal_ipv6_brackets() {
        for (pattern, value, expected) in [
            ("prod-*", "prod-one", true),
            ("prod-?", "prod-one", false),
            ("prod-?", "prod-a", true),
            ("*a*b", "xaayb", true),
            ("[::1]", "[::1]", true),
            ("[ab]", "a", false),
        ] {
            assert_eq!(
                wildcard_match(pattern.as_bytes(), value.as_bytes()),
                expected
            );
        }
        assert!(!host_matches(&["*".into(), "!prod-*".into()], "prod-one"));
    }
}
