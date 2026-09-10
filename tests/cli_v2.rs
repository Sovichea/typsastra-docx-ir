use std::process::Command;

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_typsastra-docx-ir"))
        .args(arguments)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

#[test]
fn validates_and_summarizes_v2() {
    let validated = run(&["validate", "examples/minimal-v2.docx-ir.json"]);
    assert!(validated.status.success());
    assert!(String::from_utf8_lossy(&validated.stdout).contains("valid IR: typsastra-docx-ir 2.0"));

    let summarized = run(&["summary", "examples/minimal-v2.docx-ir.json"]);
    assert!(summarized.status.success());
    let stdout = String::from_utf8_lossy(&summarized.stdout);
    assert!(stdout.contains("recorded sections: 1"));
    assert!(stdout.contains("recorded stories: 1"));
    assert!(stdout.contains("recorded resolved runs: 1"));
    assert!(stdout.contains("not independently verified"));
}

#[test]
fn schema_dispatch_preserves_v1_default_and_exposes_v2() {
    let v1 = run(&["schema"]);
    assert!(v1.status.success());
    let v1 = String::from_utf8_lossy(&v1.stdout);
    assert!(v1.contains("ParagraphRegion"));
    assert!(!v1.contains("TableCellRegion"));

    let v2 = run(&["schema", "--major", "2"]);
    assert!(v2.status.success());
    let v2 = String::from_utf8_lossy(&v2.stdout);
    assert!(v2.contains("TableCellRegion"));
    assert!(v2.contains("AffineTransform"));

    let unsupported = run(&["schema", "--major", "3"]);
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("supported: 1, 2"));
}
