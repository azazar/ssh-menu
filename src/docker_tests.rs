//! Export real, validated command templates for the opt-in Docker integration suite.
use crate::{
    Target,
    launch::{Config, DEFAULT_CONFIG},
};
use std::{collections::HashSet, fs, path::PathBuf};

#[test]
#[ignore = "fixture generator for tests/docker/run.py; requires SHUI_DOCKER_FIXTURES"]
fn export_docker_fixtures() {
    let directory =
        PathBuf::from(std::env::var_os("SHUI_DOCKER_FIXTURES").expect("fixture directory"));
    fs::create_dir_all(&directory).unwrap();
    let config = Config::parse(DEFAULT_CONFIG).unwrap();
    let target = Target {
        host: "remote@shui-docker".into(),
        port: Some("2222".into()),
    };
    let covered = [
        ("SSH", "ssh"),
        ("Bash", "bash"),
        ("Tmux", "tmux"),
        ("Zellij", "zellij"),
        ("Monitor", "monitor"),
    ];
    // These need client-side transport/GUI behavior, not just remote script execution.
    let excluded = ["Mosh", "MC", "SFTP", "Tunnel", "SOCKS"];
    let known: HashSet<_> = covered
        .iter()
        .map(|(name, _)| *name)
        .chain(excluded)
        .collect();
    assert_eq!(
        config.profiles.len(),
        known.len(),
        "update Docker coverage for added/removed profiles"
    );
    for p in &config.profiles {
        assert!(
            known.contains(p.name.as_str()),
            "unclassified profile: {}",
            p.name
        );
    }
    let write = |name: &str, command: (String, Vec<String>)| {
        fs::write(
            directory.join(format!("{name}.sh")),
            format!(
                "#!/bin/sh\nexec {}\n",
                shell_words::join(std::iter::once(command.0).chain(command.1))
            ),
        )
        .unwrap();
    };
    for (name, id) in covered {
        let p = config.profiles.iter().find(|p| p.name == name).unwrap();
        write(
            id,
            p.command_with_inputs(&target, &p.input_defaults()).unwrap(),
        );
        if let Some(command) = p.prepare(&target).unwrap() {
            write(&format!("{id}-prepare"), command);
        }
    }
    let recipes = [
        ("Configure key-based auth", "auth"),
        ("Configure key-based auth + install dotfiles", "dotfiles"),
        ("Copy terminal definition", "terminfo"),
    ];
    assert_eq!(
        config.configuration_scripts.len(),
        recipes.len(),
        "update Docker coverage for added/removed recipes"
    );
    for (name, id) in recipes {
        let mut p = config
            .configuration_scripts
            .iter()
            .find(|p| p.name == name)
            .expect("known setup recipe")
            .clone();
        // The key generator normally discovers these on the client during the dialog.
        // Resolve its two possible choices without reading the developer's SSH directory.
        for input in &mut p.inputs {
            if input.id == "key" {
                input.command = None;
                input.options = vec!["id_ed25519.pub".into(), "Generate new Ed25519 key".into()];
                input.default = Some("id_ed25519.pub".into());
            }
        }
        let values = p.input_defaults();
        write(id, p.command_with_inputs(&target, &values).unwrap());
        if id == "auth" {
            for expiry in ["8 hours", "day", "week", "month", "year", "never"] {
                let mut expiring = values.clone();
                expiring.insert("expiry".into(), expiry.into());
                write(
                    &format!("auth-expiry-{}", expiry.replace(' ', "-")),
                    p.command_with_inputs(&target, &expiring).unwrap(),
                );
            }
            let mut generated = values;
            generated.insert("key".into(), "Generate new Ed25519 key".into());
            write(
                "auth-generate",
                p.command_with_inputs(&target, &generated).unwrap(),
            );
        }
    }
    fs::write(directory.join("coverage.txt"), "Profiles: SSH, Bash, Tmux, Zellij, Monitor\nRecipes: key auth (existing and generated), key auth + dotfiles, terminal definition\nExcluded: Mosh (UDP client), MC (VFS client), SFTP (client), Tunnel/SOCKS (client forwarding)\n").unwrap();
}
