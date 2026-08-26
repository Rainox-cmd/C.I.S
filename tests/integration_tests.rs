use assert_fs::prelude::*;
use std::process::Command;

fn init_project(project_dir: &assert_fs::fixture::ChildPath) {
    let status = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["init"])
        .current_dir(project_dir)
        .status()
        .unwrap();
    assert!(status.success(), "cis init failed");
    assert!(project_dir.child(".cis").exists());
}

fn run_scan(project_dir: &assert_fs::fixture::ChildPath, incremental: bool) -> String {
    let args = if incremental {
        vec!["scan", "--incremental"]
    } else {
        vec!["scan"]
    };
    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(&args)
        .current_dir(project_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "cis scan failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn run_status(project_dir: &assert_fs::fixture::ChildPath) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["status"])
        .current_dir(project_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "cis status failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

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

#[test]
fn test_cis_scan_finds_python_files() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("app.py").write_str("print('hello')\ndef main():\n    pass\n").unwrap();
    project_dir.child("utils.py").write_str("def helper():\n    return 42\n").unwrap();
    project_dir.child("config.json").write_str("{\"key\": \"value\"}").unwrap();
    project_dir.child("README.md").write_str("# Project\n\nHello\n").unwrap();
    project_dir.child(".env").write_str("DATABASE_URL=postgres://localhost\n").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Scan complete"));
    assert!(output.contains("Files scanned: 6"));
    assert!(output.contains("Python"));
    assert!(output.contains("JSON"));
    assert!(output.contains("Markdown"));
    assert!(output.contains("source"));
    assert!(output.contains("config"));
    assert!(output.contains("assets"));
}

#[test]
fn test_cis_scan_recursive_subdirectories() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("src").create_dir_all().unwrap();
    project_dir.child("src/main.py").write_str("print('main')").unwrap();
    project_dir.child("src/utils.py").write_str("print('utils')").unwrap();
    project_dir.child("tests/test_main.py").write_str("def test_main(): pass").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 4"));
    assert!(output.contains("Python"));
}

#[test]
fn test_cis_scan_respects_gitignore() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("app.py").write_str("print('hello')").unwrap();
    project_dir.child(".gitignore").write_str("*.log\nbuild/\n").unwrap();
    project_dir.child("debug.log").write_str("log entry").unwrap();
    project_dir.child("build").create_dir_all().unwrap();
    project_dir.child("build/output.py").write_str("print('built')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 2"));
    assert!(output.contains("Python"));
}

#[test]
fn test_cis_scan_respects_cisignore() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("app.py").write_str("print('hello')").unwrap();
    project_dir.child(".cisignore").write_str("secrets/\n*.tmp\n").unwrap();
    project_dir.child("temp.tmp").write_str("temporary").unwrap();
    project_dir.child("secrets").create_dir_all().unwrap();
    project_dir.child("secrets/api_key.py").write_str("KEY = 'secret'").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 2"));
    assert!(output.contains("Python"));
}

#[test]
fn test_cis_scan_skips_target_and_node_modules() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("app.py").write_str("print('hello')").unwrap();
    project_dir.child("target").create_dir_all().unwrap();
    project_dir.child("target/wrapper.py").write_str("print('built')").unwrap();
    project_dir.child("node_modules").create_dir_all().unwrap();
    project_dir.child("node_modules/lib.js").write_str("console.log(1)").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 2"));
    assert!(output.contains("Python"));
}

#[test]
fn test_cis_scan_large_file_filtered() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("small.py").write_str("print('hello')").unwrap();
    let large_path = project_dir.child("large.rs");
    let mut large_content = String::new();
    for _ in 0..(6 * 1024 * 1024 / 100 + 1) {
        large_content.push_str(&"x".repeat(100));
    }
    large_path.write_str(&large_content).unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 2"));
    assert!(output.contains("Files skipped: 1"));
    assert!(output.contains("Python"));
    assert!(output.contains("TOML"));
}

#[test]
fn test_cis_scan_status_shows_indexed_files() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("utils.py").write_str("def helper(): pass").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    run_scan(&project_dir, false);

    let output = run_status(&project_dir);
    assert!(output.contains("Indexed files: 3"));
}

#[test]
fn test_cis_scan_incremental_detects_new_file() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output1 = run_scan(&project_dir, true);
    assert!(output1.contains("Files added: 2"));

    project_dir.child("new_module.py").write_str("print('new')").unwrap();

    let output2 = run_scan(&project_dir, true);
    assert!(output2.contains("Files added: 1"));
    assert!(output2.contains("Files changed: 0"));
    assert!(output2.contains("Files unchanged: 2"));
    assert!(output2.contains("Files deleted: 0"));
}

#[test]
fn test_cis_scan_incremental_detects_changed_file() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    run_scan(&project_dir, true);

    project_dir.child("main.py").write_str("print('world')").unwrap();

    let output = run_scan(&project_dir, true);
    assert!(output.contains("Files changed: 1"));
    assert!(output.contains("Files added: 0"));
    assert!(output.contains("Files unchanged: 1"));
}

#[test]
fn test_cis_scan_incremental_detects_deleted_file() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("keep.py").write_str("print('keep')").unwrap();
    project_dir.child("delete.py").write_str("print('delete')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    run_scan(&project_dir, true);

    std::fs::remove_file(project_dir.child("delete.py").path()).unwrap();

    let output = run_scan(&project_dir, true);
    assert!(output.contains("Files deleted: 1"));
    assert!(output.contains("Files added: 0"));
    assert!(output.contains("Files changed: 0"));
    assert!(output.contains("Files unchanged: 2"));
}

#[test]
fn test_cis_scan_incremental_no_changes_second_run() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    run_scan(&project_dir, true);

    let output = run_scan(&project_dir, true);
    assert!(output.contains("Files added: 0"));
    assert!(output.contains("Files changed: 0"));
    assert!(output.contains("Files unchanged: 2"));
    assert!(output.contains("Files deleted: 0"));
}

#[test]
fn test_cis_scan_env_file_detected() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("app.py").write_str("print('hello')").unwrap();
    project_dir.child(".env").write_str("SECRET_KEY=abc123\n").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 3"));
    assert!(output.contains("config"));
}

#[test]
fn test_cis_doctor_runs_on_scanned_project() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    run_scan(&project_dir, false);

    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["doctor"])
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("C.I.S. Doctor"));
    assert!(stdout.contains("PASS"));
    assert!(stdout.contains("Summary:"));
}

#[test]
fn test_cis_scan_multiple_languages() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let project_dir = temp_dir.child("my_project");
    project_dir.create_dir_all().unwrap();
    project_dir.child("main.py").write_str("print('hello')").unwrap();
    project_dir.child("app.js").write_str("console.log(1)").unwrap();
    project_dir.child("lib.rs").write_str("fn main() {}").unwrap();
    project_dir.child("main.go").write_str("package main").unwrap();
    project_dir.child("Cargo.toml").touch().unwrap();

    init_project(&project_dir);
    let output = run_scan(&project_dir, false);

    assert!(output.contains("Files scanned: 5"));
    assert!(output.contains("Python"));
    assert!(output.contains("JavaScript"));
    assert!(output.contains("Rust"));
    assert!(output.contains("Go"));
}

#[test]
fn test_cis_scan_real_project_directory() {
    let project_path = r"C:\Users\Admin\Desktop\projects1234\bots\business-lead-system";
    let path = std::path::Path::new(project_path);
    if !path.exists() {
        eprintln!("Skipping real project test: {} not found", project_path);
        return;
    }
    if !path.join("Cargo.toml").exists() && !path.join(".git").exists() {
        let status = Command::new(env!("CARGO_BIN_EXE_cis"))
            .args(["init"])
            .current_dir(path)
            .status();
        let _ = status;
    } else {
        let _ = Command::new(env!("CARGO_BIN_EXE_cis"))
            .args(["init"])
            .current_dir(path)
            .status();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["scan"])
        .current_dir(path)
        .output();

    match output {
        Ok(out) => {
            assert!(out.status.success(), "cis scan failed on real project: {}", String::from_utf8_lossy(&out.stderr));
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("Real project scan stdout:\n{}", stdout);
            eprintln!("Real project scan stderr:\n{}", stderr);
            assert!(stdout.contains("Scan complete"));
            assert!(stdout.contains("Files scanned:"));
        }
        Err(e) => {
            eprintln!("Failed to execute cis on real project: {}", e);
            eprintln!("Windows Application Control may be blocking the binary.");
        }
    }
}

#[test]
fn test_cis_doctor_real_project_directory() {
    let project_path = r"C:\Users\Admin\Desktop\projects1234\bots\business-lead-system";
    let path = std::path::Path::new(project_path);
    if !path.exists() {
        eprintln!("Skipping real project test: {} not found", project_path);
        return;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["doctor"])
        .current_dir(path)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            eprintln!("Real project doctor stdout:\n{}", stdout);
            assert!(stdout.contains("C.I.S. Doctor"));
        }
        Err(e) => {
            eprintln!("Failed to execute cis doctor: {}", e);
        }
    }
}

#[test]
fn test_cis_status_real_project_directory() {
    let project_path = r"C:\Users\Admin\Desktop\projects1234\bots\business-lead-system";
    let path = std::path::Path::new(project_path);
    if !path.exists() {
        eprintln!("Skipping real project test: {} not found", project_path);
        return;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_cis"))
        .args(["status"])
        .current_dir(path)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            eprintln!("Real project status stdout:\n{}", stdout);
            assert!(stdout.contains("C.I.S. Status"));
        }
        Err(e) => {
            eprintln!("Failed to execute cis status: {}", e);
        }
    }
}
