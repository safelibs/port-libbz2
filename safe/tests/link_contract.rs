use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn resolve(repo: &Path, preferred: &str, fallback: &str) -> PathBuf {
    let preferred = repo.join(preferred);
    if preferred.exists() {
        preferred
    } else {
        repo.join(fallback)
    }
}

fn run(repo: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    if !output.status.success() {
        panic!(
            "{} {:?} failed\nstdout:\n{}\nstderr:\n{}",
            program,
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn undefined_symbols(object: &Path) -> Vec<String> {
    let output = Command::new("readelf")
        .args(["-Ws", object.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());

    let mut symbols: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() >= 8 && fields[6] == "UND" {
                Some(fields[7].to_string())
            } else {
                None
            }
        })
        .collect();
    symbols.sort();
    symbols.dedup();
    symbols
}

fn bz2_undefined_count(object: &Path) -> usize {
    undefined_symbols(object)
        .into_iter()
        .filter(|symbol| symbol.starts_with("BZ2_"))
        .count()
}

#[test]
fn selected_object_files_still_match_captured_undefined_sets() {
    let repo = repo_root();
    let public_api_object = resolve(
        &repo,
        "original/public_api_test.o",
        "target/original-baseline/public_api_test.o",
    );
    let cli_object = resolve(
        &repo,
        "original/bzip2.o",
        "target/original-baseline/bzip2.o",
    );

    let expected_public_api =
        fs::read_to_string(repo.join("safe/abi/original.public_api_undefined.txt")).unwrap();
    let expected_cli =
        fs::read_to_string(repo.join("safe/abi/original.cli_undefined.txt")).unwrap();

    let actual_public_api = undefined_symbols(&public_api_object).join("\n");
    let actual_cli = undefined_symbols(&cli_object).join("\n");

    assert_eq!(actual_public_api, expected_public_api.trim_end());
    assert_eq!(actual_cli, expected_cli.trim_end());
    assert_eq!(bz2_undefined_count(&public_api_object), 23);
    assert_eq!(bz2_undefined_count(&cli_object), 8);
}

#[test]
fn source_and_object_link_contracts_run_against_the_safe_library() {
    let repo = repo_root();
    run(&repo, "bash", &["safe/scripts/build-safe.sh"]);
    run(
        &repo,
        "bash",
        &["safe/scripts/link-original-tests.sh", "--all"],
    );
}
