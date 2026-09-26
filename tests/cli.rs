use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_our-tool"))
}

#[test]
fn prints_version() {
    let output = bin()
        .arg("--version")
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!("our-tool {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn prints_hello_by_default() {
    let output = bin().output().expect("failed to run binary");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "hello from our-tool");
}
