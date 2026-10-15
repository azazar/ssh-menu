//! Execute shipped shell recipes with isolated PATHs; never contact a server.
use crate::{
    Target,
    launch::{Config, DEFAULT_CONFIG, Profile},
};
use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output, Stdio},
};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("ssh-menu-presets-{name}-{}", std::process::id()));
        fs::create_dir_all(root.join("bin")).unwrap();
        let sandbox = Self(root);
        sandbox.link("sh");
        sandbox
    }
    fn stub(&self, name: &str, body: &str) {
        let path = self.0.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn link(&self, name: &str) {
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .unwrap();
        assert!(found.status.success());
        symlink(
            String::from_utf8(found.stdout).unwrap().trim(),
            self.0.join("bin").join(name),
        )
        .unwrap();
    }
    fn run(&self, profile: &Profile, port: Option<&str>, input: Option<(&str, &str)>) -> Output {
        self.run_input(profile, port, input, b"")
    }
    fn run_input(
        &self,
        profile: &Profile,
        port: Option<&str>,
        input: Option<(&str, &str)>,
        stdin: &[u8],
    ) -> Output {
        let mut values = profile.input_defaults();
        if let Some((id, value)) = input {
            values.insert(id.into(), value.into());
        }
        let (_, args) = profile
            .command_with_inputs(
                &Target {
                    host: "user@host".into(),
                    port: port.map(str::to_owned),
                },
                &values,
            )
            .unwrap();
        let mut child = Command::new("/bin/sh")
            .current_dir(&self.0)
            .args(args)
            .env("PATH", self.0.join("bin"))
            .env("HOME", &self.0)
            .env("TMPDIR", &self.0)
            .env("TERM", "test-terminal")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn profile(name: &str) -> Profile {
    let config = Config::parse(DEFAULT_CONFIG).unwrap();
    config
        .profiles
        .into_iter()
        .chain(config.configuration_scripts)
        .find(|p| p.name == name)
        .unwrap()
}
fn stdout(out: Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn shipped_transports_preserve_ports_and_tunnel_arguments() {
    let env = Sandbox::new("transports");
    for name in ["ssh", "sftp"] {
        env.stub(name, "printf '<%s>\\n' \"$@\"");
    }
    for (port, ssh_port, sftp_port) in [
        (None, "", ""),
        (Some("2222"), "<-p>\n<2222>\n", "<-P>\n<2222>\n"),
    ] {
        assert_eq!(
            stdout(env.run(&profile("SFTP"), port, None)),
            format!("{sftp_port}<-->\n<user@host>\n")
        );
        let tmux = stdout(env.run(&profile("Tmux"), port, None));
        assert!(tmux.starts_with(&format!("{ssh_port}<-tt>\n<-->\n<user@host>\n<sh -c ")));
        assert!(tmux.contains("exec tmux new-session -A -s main"));
        for (name, flag, value) in [("Tunnel", "L", "8080:localhost:80"), ("SOCKS", "D", "1080")] {
            let out = stdout(env.run(&profile(name), port, None));
            assert!(out.contains(&format!("{ssh_port}<-N>\n<-T>")), "{out}");
            assert!(out.contains("<ExitOnForwardFailure=yes>"));
            assert!(
                out.ends_with(&format!(
                    "<-{flag}>\n<127.0.0.1:{value}>\n<-->\n<user@host>\n"
                )),
                "{out}"
            );
        }
    }
    let value = "8080:$(touch injected);'host:80";
    let out = stdout(env.run(&profile("Tunnel"), None, Some(("forward", value))));
    assert!(out.contains(&format!("<127.0.0.1:{value}>")));
    assert!(!env.0.join("injected").exists());
}

#[test]
fn monitor_uses_isolated_dashboard_and_cleans_up_on_failure() {
    let env = Sandbox::new("monitor");
    for tool in ["cat", "mktemp", "mkdir", "rm"] {
        env.link(tool);
    }
    let mut p = profile("Monitor");
    p.command = "sh -c %{remote}".into();
    for tool in ["top", "htop"] {
        env.stub(tool, "echo wrong-monitor");
    }
    let missing = env.run(&p, None, None);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("btop is missing on the remote"));
    fs::create_dir_all(env.0.join(".config/btop")).unwrap();
    let personal = env.0.join(".config/btop/btop.conf");
    fs::write(&personal, "personal configuration").unwrap();
    env.stub("btop", "cat \"$XDG_CONFIG_HOME/btop/btop.conf\"");
    assert_eq!(
        stdout(env.run(&p, None, None)),
        format!("{}\n", p.fields["dashboard"])
    );
    env.stub("btop", "exit 42");
    assert_eq!(env.run(&p, None, None).status.code(), Some(42));
    assert_eq!(
        fs::read_to_string(personal).unwrap(),
        "personal configuration"
    );
    assert!(!fs::read_dir(&env.0).unwrap().any(|f| {
        f.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("ssh-menu-monitor.")
    }));
}

#[test]
fn profile_installer_selects_manager_and_propagates_failure() {
    let env = Sandbox::new("packages");
    env.stub("id", "echo 1000");
    env.stub("sudo", "printf 'sudo\\n'; shift; exec \"$@\"");
    let mut p = profile("SSH");
    p.command = "sh -c %{install} -- tmux tmux".into();
    assert!(!env.run(&p, None, None).status.success());
    for (manager, args) in [
        ("brew", "install"),
        ("apt-get", "install"),
        ("dnf", "install"),
        ("yum", "install"),
        ("pacman", "-S\n--needed"),
        ("zypper", "install"),
        ("apk", "add"),
    ] {
        env.stub(manager, "printf '%s\\n' \"$@\"");
        let prefix = if manager == "brew" { "" } else { "sudo\n" };
        assert_eq!(
            stdout(env.run(&p, None, None)),
            format!("{prefix}{args}\ntmux\n")
        );
        p.command = "sh -c %{install} -- openssh sftp".into();
        let package = match manager {
            "apt-get" => "openssh-client",
            "dnf" | "yum" => "openssh-clients",
            "apk" => "openssh-sftp-client",
            _ => "openssh",
        };
        assert_eq!(
            stdout(env.run(&p, None, None)),
            format!("{prefix}{args}\n{package}\n")
        );
        p.command = "sh -c %{install} -- tmux tmux".into();
        env.stub(manager, "exit 42");
        assert_eq!(env.run(&p, None, None).status.code(), Some(42));
        fs::remove_file(env.0.join("bin").join(manager)).unwrap();
    }
    env.stub("id", "echo 0");
    env.stub("apt-get", "printf '%s\\n' \"$@\"");
    p.command = "sh -c %{install} -- btop btop".into();
    assert_eq!(stdout(env.run(&p, None, None)), "install\nbtop\n");
}

#[test]
fn terminal_definition_is_exported_before_ssh_and_temp_files_are_removed() {
    let env = Sandbox::new("terminfo");
    for utility in ["cat", "mktemp", "mkdir", "rm"] {
        env.link(utility);
    }
    // Emulate ssh's remote shell, forwarding stdin without executing any real ssh.
    env.stub(
        "ssh",
        "[ \"$1\" = -p ] && shift 2; shift 2; exec /bin/sh -c \"$1\"",
    );
    env.stub("infocmp", "echo 'test-terminal|test terminal, cols#80,'");
    env.stub("tic", "[ \"$1\" = -x ] && [ \"$2\" = -o ] && [ \"$3\" = \"$HOME/.terminfo\" ] || exit 9; cat \"$4\"");
    let p = profile("Copy terminal definition");
    assert_eq!(
        stdout(env.run(&p, Some("2222"), None)),
        "test-terminal|test terminal, cols#80,\n"
    );
    assert!(env.0.join(".terminfo").is_dir());
    env.stub("tic", "exit 42");
    assert_eq!(env.run(&p, None, None).status.code(), Some(42));
    assert!(!fs::read_dir(&env.0).unwrap().any(|f| {
        f.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("ssh-menu-terminfo.")
    }));
    env.stub("ssh", "touch \"$HOME/contacted\"");
    env.link("touch");
    env.stub("infocmp", "echo partial; exit 1");
    assert!(!env.run(&p, None, None).status.success());
    assert!(!env.0.join("contacted").exists());
}

#[test]
fn dependency_installation_requires_consent_and_verifies_the_result() {
    let env = Sandbox::new("dependency");
    let mut p = profile("SSH");
    p.command = "sh -c %{ensure} -- wanted wanted 'this machine' %{install}".into();
    env.stub(
        "brew",
        "printf '#!/bin/sh\\nexit 0\\n' > \"$HOME/bin/wanted\"; /bin/chmod +x \"$HOME/bin/wanted\"",
    );
    for reply in [b"n\n".as_slice(), b"\n", b""] {
        let result = env.run_input(&p, None, None, reply);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("Install wanted? [y/N]"));
        assert!(!env.0.join("bin/wanted").exists());
    }
    assert!(env.run_input(&p, None, None, b"yes\n").status.success());
    assert!(
        env.run(&p, None, None).stderr.is_empty(),
        "installed tools must not prompt"
    );
    fs::remove_file(env.0.join("bin/wanted")).unwrap();
    env.stub("brew", "exit 42");
    assert_eq!(
        env.run_input(&p, None, None, b"y\n").status.code(),
        Some(42)
    );
    env.stub("brew", "exit 0");
    let result = env.run_input(&p, None, None, b"y\n");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("still unavailable"));
}

#[test]
fn key_expiry_updates_all_matching_entries_and_preserves_restrictions() {
    let env = Sandbox::new("key-expiry");
    for tool in ["awk", "mktemp", "cp", "cmp", "chmod", "mv", "rm"] {
        env.link(tool);
    }
    let date = Command::new("sh")
        .args(["-c", "command -v date"])
        .output()
        .unwrap();
    let date = String::from_utf8(date.stdout).unwrap();
    env.stub(
        "date",
        &format!(
            "if [ \"$1\" = +%s ]; then echo 1704067200; else exec {} \"$@\"; fi",
            date.trim()
        ),
    );
    fs::create_dir_all(env.0.join(".ssh")).unwrap();
    let authorized = env.0.join(".ssh/authorized_keys");
    let prefix = "# ssh-ed25519 AAAATEST commented out\n\nssh-ed25519 OTHER unrelated\n";
    let restrictions = r#"command="echo \"quoted, text\" expiry-time=keep",no-agent-forwarding"#;
    let original = format!(
        "{prefix}{restrictions},expiry-time=\"20200101Z\" ssh-ed25519 AAAATEST comment with spaces\nssh-ed25519 AAAATEST duplicate\n"
    );
    let mut p = profile("Configure key-based auth");
    p.inputs.retain(|input| input.id == "expiry");
    p.command = "sh -c %{set_expiry} -- %{input:expiry}".into();
    for (choice, expiry) in [
        ("8 hours", "20240101080000Z"),
        ("day", "20240102000000Z"),
        ("week", "20240108000000Z"),
        ("month", "20240131000000Z"),
        ("year", "20241231000000Z"),
        ("never", ""),
    ] {
        fs::write(&authorized, &original).unwrap();
        stdout(env.run_input(
            &p,
            None,
            Some(("expiry", choice)),
            b"ssh-ed25519 AAAATEST local comment\n",
        ));
        let option = if expiry.is_empty() {
            String::new()
        } else {
            format!("expiry-time=\"{expiry}\"")
        };
        let comma = if option.is_empty() { "" } else { "," };
        let space = if option.is_empty() { "" } else { " " };
        assert_eq!(
            fs::read_to_string(&authorized).unwrap(),
            format!(
                "{prefix}{restrictions}{comma}{option} ssh-ed25519 AAAATEST comment with spaces\n{option}{space}ssh-ed25519 AAAATEST duplicate\n"
            )
        );
        assert_eq!(
            fs::metadata(&authorized).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Missing keys and malformed choices must leave the file unchanged.
    fs::write(&authorized, &original).unwrap();
    assert!(
        !env.run_input(&p, None, Some(("expiry", "week")), b"ssh-ed25519 MISSING\n")
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(&authorized).unwrap(), original);
    p.command = "sh -c %{set_expiry} -- invalid".into();
    assert!(
        !env.run_input(&p, None, None, b"ssh-ed25519 AAAATEST\n")
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(&authorized).unwrap(), original);
    assert!(!fs::read_dir(env.0.join(".ssh")).unwrap().any(|f| {
        f.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ssh-menu-expiry.")
    }));
    // Both shipped recipes share the same picker and updater.
    let dotfiles = profile("Configure key-based auth + install dotfiles");
    assert_eq!(p.fields["set_expiry"], dotfiles.fields["set_expiry"]);
    assert_eq!(
        dotfiles
            .inputs
            .iter()
            .find(|i| i.id == "expiry")
            .unwrap()
            .options,
        ["never", "8 hours", "day", "week", "month", "year"]
    );
}
