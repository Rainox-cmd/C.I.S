use assert_fs::prelude::*;
use std::process::Command;

#[test]
fn test_cis_init_creates_cis_directory() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(project_dir.child(".cis").exists());
    assert!(project_dir.child(".cis").child("config.toml").exists());
    assert!(project_dir.child(".cis").child("project.db").exists());
    assert!(project_dir.child(".cis").child("logs").exists());
    assert!(project_dir.child(".cis").child("cache").exists());
}

#[test]
fn test_cis_status_with_empty_index() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["status"])
        .current_dir(&project_dir)
        .status()
        .unwrap();

    assert!(status.success());
}

#[test]
fn test_cis_doctor_reports_stats() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["doctor"])
        .current_dir(&project_dir)
        .status()
        .unwrap();

    assert!(status.success());
}

#[test]
fn test_cis_config_show() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["config", "show"])
        .current_dir(&project_dir)
        .status()
        .unwrap();

    assert!(status.success());
}

#[test]
fn test_cis_config_set() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["config", "set", "general.project_name", "MyProject"])
        .current_dir(&project_dir)
        .status()
        .unwrap();

    assert!(status.success());
    
    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["config", "show"])
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(String::from_utf8_lossy(&output.stdout).contains("MyProject"));
}
