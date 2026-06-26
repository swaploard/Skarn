use std::path::Path;

use skarn_common::CommandSpec;
use skarn_compress::Compressor;

fn fixture_path(name: &str) -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    dir.join(name).to_string_lossy().to_string()
}

fn read_fixture(name: &str) -> String {
    let s = std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"));
    s.replace("\r\n", "\n")
}

fn run_golden(profile_name: &str, fixture_stem: &str, extra_args: Vec<&str>) {
    let raw = read_fixture(&format!("{fixture_stem}_fixture.txt"));
    let expected = read_fixture(&format!("{fixture_stem}_expected.txt"));
    let expected = expected.trim_end_matches('\n');

    let spec = CommandSpec::new(profile_name, extra_args);
    let c = Compressor::builtin();

    let out = c.compress(&spec, raw.as_bytes(), b"");

    let actual = out.text;
    assert_eq!(
        actual,
        expected,
        "golden mismatch for profile `{profile_name}`\n\
         --- expected (from {fixture_stem}_expected.txt)\n\
         +++ actual\n\
         {}",
        diff(expected, &actual)
    );
}

fn diff(a: &str, b: &str) -> String {
    let lines_a: Vec<&str> = a.lines().collect();
    let lines_b: Vec<&str> = b.lines().collect();
    let mut out = String::new();
    let max = lines_a.len().max(lines_b.len());
    for i in 0..max {
        let la = lines_a.get(i).copied().unwrap_or("");
        let lb = lines_b.get(i).copied().unwrap_or("");
        if la != lb {
            out.push_str(&format!(
                "  {}:{:>4} | {la}\n  {}:{:>4} | {lb}\n",
                if i < lines_a.len() { "" } else { "?" },
                i + 1,
                if i < lines_b.len() { "" } else { "?" },
                i + 1,
            ));
        }
    }
    if out.is_empty() {
        out.push_str("  (no diff)");
    }
    out
}

#[test]
fn cargo_profile_golden() {
    run_golden("cargo", "cargo", vec!["test"]);
}

#[test]
fn git_profile_golden() {
    run_golden("git", "git", vec!["diff"]);
}

#[test]
fn cargo_preserves_errors_and_warnings() {
    let raw = read_fixture("cargo_fixture.txt");
    let spec = CommandSpec::new("cargo", ["test"]);
    let c = Compressor::builtin();
    let out = c.compress(&spec, raw.as_bytes(), b"");

    assert!(
        out.text.contains("error[E0308]"),
        "error[E0308] must survive compression"
    );
    assert!(
        out.text.contains("error: aborting due to"),
        "fatal error message must survive compression"
    );
    assert!(
        out.text.contains("warning:"),
        "warning lines must survive compression"
    );
    assert!(
        !out.text.contains("Compiling"),
        "compile spam must be removed"
    );
    assert!(
        !out.text.contains("... ok"),
        "passing test lines must be removed"
    );
    assert!(
        out.savings.percent() >= 30,
        "got only {}%, expected meaningful savings",
        out.savings.percent()
    );
}
