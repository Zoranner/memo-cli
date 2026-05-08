use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

fn memo_bin() -> &'static str {
    env!("CARGO_BIN_EXE_memo")
}

fn run_memo(
    home: &Path,
    workdir: &Path,
    extra_env: &[(&str, &str)],
    args: &[&str],
) -> anyhow::Result<Output> {
    let mut command = Command::new(memo_bin());
    command.current_dir(workdir);
    command.args(args);
    command.env("HOME", home);
    command.env("USERPROFILE", home);
    command.env_remove("HOMEDRIVE");
    command.env_remove("HOMEPATH");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    Ok(command.output()?)
}

fn assert_success(output: &Output) -> anyhow::Result<String> {
    if !output.status.success() {
        anyhow::bail!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[test]
fn awaken_uses_fixed_home_config_root_and_default_data_dir() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let workdir = TempDir::new()?;

    let output = run_memo(home.path(), workdir.path(), &[], &["awaken"])?;
    let stdout = assert_success(&output)?;
    let memo_home = home.path().join(".memo");
    let default_data_dir = memo_home.join("data");

    assert!(stdout.contains(&format!(
        "Awakened memory space at {}",
        default_data_dir.display()
    )));
    assert!(stdout.contains(&format!("config_dir: {}", memo_home.display())));
    assert!(memo_home.join("config.toml").exists());
    assert!(memo_home.join("providers.toml").exists());
    assert!(default_data_dir.join("memory.db").exists());
    assert!(default_data_dir.join("text-index").is_dir());
    assert!(!memo_home.join("memory.db").exists());
    assert!(!workdir.path().join(".memo").exists());
    Ok(())
}

#[test]
fn cli_respects_configured_and_environment_data_dir_precedence() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let workdir = TempDir::new()?;
    let memo_home = home.path().join(".memo");

    assert_success(&run_memo(home.path(), workdir.path(), &[], &["awaken"])?)?;
    fs::write(
        memo_home.join("config.toml"),
        "[storage]\ndata_dir = \"data-store\"\n",
    )?;

    let configured_output = run_memo(home.path(), workdir.path(), &[], &["awaken"])?;
    let configured_stdout = assert_success(&configured_output)?;
    let configured_data_dir = memo_home.join("data-store");
    assert!(configured_stdout.contains(&format!(
        "Awakened memory space at {}",
        configured_data_dir.display()
    )));
    assert!(configured_data_dir.join("memory.db").exists());

    let state_output = run_memo(home.path(), workdir.path(), &[], &["state", "--json"])?;
    let state_stdout = assert_success(&state_output)?;
    let state_json: serde_json::Value = serde_json::from_str(&state_stdout)?;
    assert!(matches!(
        state_json["status"].as_str(),
        Some("needs_setup") | Some("needs_dream") | Some("ready")
    ));
    assert!(state_json["message"].is_string());
    assert!(state_json["next"].is_string());
    assert_eq!(state_json["diagnostics"]["state"]["episode_count"], 0);

    let overridden_output = run_memo(
        home.path(),
        workdir.path(),
        &[("MEMO_DATA_DIR", "env-store")],
        &["awaken"],
    )?;
    let overridden_stdout = assert_success(&overridden_output)?;
    let overridden_data_dir = memo_home.join("env-store");
    assert!(overridden_stdout.contains(&format!(
        "Awakened memory space at {}",
        overridden_data_dir.display()
    )));
    assert!(overridden_data_dir.join("memory.db").exists());
    Ok(())
}

#[test]
fn readme_install_commands_point_to_repo_scripts() -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md"))?;
    let zh_readme = fs::read_to_string(root.join("docs/zh-CN/README.md"))?;

    for expected_path in ["scripts/install.ps1", "scripts/install.sh"] {
        assert!(
            root.join(expected_path).exists(),
            "expected installer script to exist: {expected_path}"
        );
        assert!(
            readme.contains(expected_path),
            "README.md should reference {expected_path}"
        );
        assert!(
            zh_readme.contains(expected_path),
            "docs/zh-CN/README.md should reference {expected_path}"
        );
    }

    assert!(!readme.contains("scripts/install/install."));
    assert!(!zh_readme.contains("scripts/install/install."));
    Ok(())
}
