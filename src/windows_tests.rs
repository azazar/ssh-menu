//! Exercise real Windows execution with Git and POSIX shells absent from PATH.
use crate::{
    Target,
    command::Shell,
    launch::{Config, DEFAULT_CONFIG},
};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

fn records(path: &Path) -> Vec<Vec<String>> {
    let data = fs::read(path).unwrap_or_default();
    let mut cursor = 0;
    let number = |cursor: &mut usize| {
        let result = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().unwrap()) as usize;
        *cursor += 4;
        result
    };
    let mut records = Vec::new();
    while cursor < data.len() {
        let mut record = Vec::new();
        for _ in 0..number(&mut cursor) {
            let len = number(&mut cursor);
            record.push(String::from_utf8(data[cursor..cursor + len].to_vec()).unwrap());
            cursor += len;
        }
        records.push(record);
    }
    records
}

fn run(command: (String, Vec<String>), log: &Path) {
    let output = Command::new(command.0)
        .args(command.1)
        .env("SHUI_NATIVE_LOG", log)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn windows_defaults_work_without_posix_tools() {
    if let Some(root) = std::env::var_os("SHUI_NATIVE_CHILD") {
        child(Path::new(&root));
        return;
    }
    let root = std::env::temp_dir().join(format!("shui-native-{}", std::process::id()));
    fs::create_dir_all(root.join("bin")).unwrap();
    let result = std::panic::catch_unwind(|| {
        let fixture = root.join("client.exe");
        if let Some(source) = std::env::var_os("SHUI_TEST_NATIVE_CLIENT") {
            fs::copy(source, &fixture).unwrap();
        } else {
            assert!(
                Command::new("rustc")
                    .args([
                        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/native_client.rs"),
                        "-o"
                    ])
                    .arg(&fixture)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        for name in [
            "ssh",
            "ssh-keygen",
            "scp",
            "mosh",
            "mc",
            "sftp",
            "dbclient",
            "putty",
            "plink",
            "infocmp",
        ] {
            fs::copy(&fixture, root.join("bin").join(format!("{name}.exe"))).unwrap();
        }
        let system = std::env::var_os("SystemRoot").unwrap();
        let system = Path::new(&system).join("System32");
        let path = std::env::join_paths([
            root.join("bin"),
            system.clone(),
            system.join("WindowsPowerShell/v1.0"),
        ])
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows_tests::windows_defaults_work_without_posix_tools",
                "--nocapture",
            ])
            .env_remove("WINEPATH")
            .env("SHUI_NATIVE_CHILD", &root)
            .env("PATH", path)
            .env("XDG_CONFIG_HOME", root.join("settings"))
            .env("USERPROFILE", &root)
            .env("TERM", "test-terminal")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    });
    fs::remove_dir_all(&root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn child(root: &Path) {
    for name in ["sh", "git"] {
        assert!(
            !Command::new("where.exe")
                .arg(name)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let loaded = Config::load(root).unwrap();
    assert!(loaded.profiles.iter().any(|p| p.name == "SSH"));
    let config = Config::parse(DEFAULT_CONFIG).unwrap();
    let target = Target {
        host: "alice@host".into(),
        port: Some("2222".into()),
    };
    let log = root.join("calls");
    for profile in &config.profiles {
        assert_eq!(profile.shell, Shell::Direct, "{}", profile.name);
        // Wine supplies no functional Windows PowerShell; native CI tests it.
        if profile.name != "Mosh" || std::env::var_os("SHUI_TEST_WINE").is_none() {
            assert!(profile.is_available(), "{}", profile.name);
            if let Some(command) = profile.prepare(&target).unwrap() {
                run(command, &log);
            }
        }
        run(profile.command(&target).unwrap(), &log);
        let all = records(&log);
        let args = all.last().unwrap();
        assert!(
            args.iter().any(|a| a.contains("alice@host")),
            "{}: {args:?}",
            profile.name
        );
        assert!(
            args.iter().any(|a| a.contains("2222")) || profile.name == "MC",
            "{}: {args:?}",
            profile.name
        );
    }
    fs::remove_file(root.join("bin/putty.exe")).unwrap();
    assert!(
        !config
            .profiles
            .iter()
            .find(|p| p.name == "PuTTY")
            .unwrap()
            .is_available()
    );
    if std::env::var_os("SHUI_TEST_WINE").is_some() {
        return;
    }
    fs::create_dir_all(root.join(".ssh")).unwrap();
    let key = "id_o'hara ü.pub";
    fs::write(
        root.join(".ssh").join(key),
        "ssh-ed25519 AAAATEST example\n",
    )
    .unwrap();
    fs::write(
        root.join(".ssh").join(key.trim_end_matches(".pub")),
        "private",
    )
    .unwrap();
    fs::create_dir_all(root.join("src/dotfiles")).unwrap();
    fs::write(root.join("src/dotfiles/install.sh"), "fixture").unwrap();
    for profile in &config.configuration_scripts {
        assert_eq!(profile.shell, Shell::Powershell);
        assert!(profile.is_available(), "{}", profile.name);
        let mut profile = profile.clone();
        for input in &mut profile.inputs {
            if input.command.is_some() {
                *input = input.resolve().unwrap();
            }
        }
        let values = profile.input_defaults();
        if let Some(selected) = values.get("key") {
            assert_eq!(selected, key);
        }
        fs::remove_file(&log).unwrap();
        run(profile.command_with_inputs(&target, &values).unwrap(), &log);
        let all = records(&log);
        assert!(!all.is_empty());
        for call in &all {
            assert_ne!(Path::new(&call[0]).file_name().unwrap(), "sh.exe");
        }
        if profile.name.starts_with("Configure key") {
            assert!(all[0].last().unwrap().contains("ssh-ed25519 AAAATEST"));
            assert!(all[1].iter().any(|arg| arg.ends_with("id_o'hara ü")));
            assert!(all[1].last().unwrap().contains("ssh-ed25519 AAAATEST"));
        }
    }
}
