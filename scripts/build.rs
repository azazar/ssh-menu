#[path = "../src/config_defaults.rs"]
mod config_defaults;

fn main() {
    for path in [
        "docs/config.example.yml",
        "docs/config.windows.yml",
        "docs/config.optional.yml",
        "src/config_defaults.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let windows = std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    let config = config_defaults::defaults(windows, false).expect("valid default config parts");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("config.yml"), format!("# Shui config: F9 edits and reloads. Existing files are preserved on upgrade.\n# Reference and optional examples: https://github.com/azazar/shui/blob/master/docs/configuration.md\n{config}")).unwrap();
}
