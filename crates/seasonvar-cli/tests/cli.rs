use std::process::Command;

#[test]
fn version_flag_prints_name_and_semver() {
    let out = Command::new(env!("CARGO_BIN_EXE_seasonvar"))
        .arg("--version")
        .output()
        .expect("run seasonvar --version");
    assert!(out.status.success(), "exit status {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.trim(),
        format!("seasonvar {}", env!("CARGO_PKG_VERSION"))
    );
}
