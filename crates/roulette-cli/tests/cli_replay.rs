//! TASK-044 CI smoke (TASK-043 `--script`): the CLI driven by the same
//! scripted input twice must emit byte-identical stdout.

use std::process::{Command, Stdio};

fn run_cli(seed: &str) -> Vec<u8> {
    let script_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/smoke_script.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_roulette"))
        .args(["--seed", seed, "--script", script_path, "--non-interactive"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn roulette");
    assert!(out.status.success(), "cli failed: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

#[test]
fn scripted_cli_output_is_byte_identical() {
    let a = run_cli("smoke-seed-7");
    let b = run_cli("smoke-seed-7");
    assert_eq!(a, b, "same script + seed ⇒ byte-identical output");
    assert!(!a.is_empty(), "smoke run produced no output");
}

#[test]
fn different_seed_diverges() {
    let a = run_cli("smoke-seed-7");
    let b = run_cli("smoke-seed-8");
    assert_ne!(a, b, "different seeds must diverge");
}
