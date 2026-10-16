#[path = "../src/config_defaults.rs"]
mod config_defaults;

fn main() {
    for path in [
        "docs/config.example.yml",
        "docs/config.windows.yml",
        "docs/config.optional.yml",
        "docs/config.optional.windows.yml",
        "src/config_defaults.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let windows = std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    let config = config_defaults::user_config(windows).expect("valid default config parts");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("config.yml"), config).unwrap();
}
