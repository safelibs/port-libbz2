use std::fs;
use std::path::PathBuf;

const EXPECTED_DEPENDENTS: &[&str] = &[
    "libapt-pkg6.0t64",
    "bzip2",
    "libpython3.12-stdlib",
    "php8.3-bz2",
    "pike8.0-bzip2",
    "libcompress-raw-bzip2-perl",
    "mariadb-plugin-provider-bzip2",
    "gpg",
    "zip",
    "unzip",
    "libarchive13t64",
    "libfreetype6",
    "gstreamer1.0-plugins-good",
];

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn read_repo_text(path: &str) -> String {
    fs::read_to_string(repo_path(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

fn extract_binary_packages(json: &str) -> Vec<String> {
    json.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("\"binary_package\": \"")?;
            Some(
                rest.split('"')
                    .next()
                    .expect("binary package line should contain a closing quote")
                    .to_string(),
            )
        })
        .collect()
}

fn assert_contains(text: &str, needle: &str, context: &str) {
    assert!(
        text.contains(needle),
        "{context} is missing expected text: {needle}"
    );
}

#[test]
fn dependent_runtime_matrix_keeps_all_thirteen_installed_smokes() {
    let dependents = extract_binary_packages(&read_repo_text("dependents.json"));
    let expected = EXPECTED_DEPENDENTS
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>();
    assert_eq!(dependents, expected);
}

#[test]
fn release_gate_keeps_runtime_and_compile_compatibility_split_explicit() {
    let full_suite = read_repo_text("safe/scripts/run-full-suite.sh");
    assert_contains(
        &full_suite,
        "bash \"$ROOT/safe/scripts/link-original-tests.sh\" --all",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "bash \"$ROOT/safe/scripts/run-debian-tests.sh\" --tests link-with-shared bigfile bzexe-test compare compress grep",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$ROOT/test-original.sh\"",
        "safe/scripts/run-full-suite.sh",
    );

    let original_harness = read_repo_text("test-original.sh");
    for dependent in EXPECTED_DEPENDENTS {
        let needle = format!("run_test \"{dependent}\" ");
        assert_eq!(
            original_harness.matches(&needle).count(),
            1,
            "test-original.sh should keep exactly one runtime smoke for {dependent}"
        );
    }

    let debian_control = read_repo_text("safe/debian/tests/control");
    assert_contains(
        &debian_control,
        "Tests: link-with-shared",
        "safe/debian/tests/control",
    );
    assert_contains(
        &debian_control,
        "Tests: bigfile bzexe-test compare compress grep",
        "safe/debian/tests/control",
    );
}

#[test]
fn bigfile_autopkgtest_keeps_sparse_file_strategy_and_full_stream_validation() {
    let bigfile = read_repo_text("safe/debian/tests/bigfile");
    let uses_sparse_creation =
        bigfile.contains("truncate -s 2049M bigfile") || bigfile.contains("count=0 seek=2049M");
    assert!(
        uses_sparse_creation,
        "safe/debian/tests/bigfile must keep the sparse-file strategy"
    );
    assert_contains(
        &bigfile,
        "bzip2 -t bigfile.bz2",
        "safe/debian/tests/bigfile",
    );
}

#[test]
fn package_consumers_fail_fast_on_missing_current_debs() {
    let layout = read_repo_text("safe/scripts/check-package-layout.sh");
    assert_contains(
        &layout,
        "missing package manifest: $MANIFEST; run bash safe/scripts/build-debs.sh first",
        "safe/scripts/check-package-layout.sh",
    );
    assert_contains(
        &layout,
        "required package artifact missing from $OUT",
        "safe/scripts/check-package-layout.sh",
    );

    let debian_tests = read_repo_text("safe/scripts/run-debian-tests.sh");
    assert_contains(
        &debian_tests,
        "missing package manifest: $MANIFEST; run bash safe/scripts/build-debs.sh first",
        "safe/scripts/run-debian-tests.sh",
    );
    assert_contains(
        &debian_tests,
        "required package artifact missing from $OUT",
        "safe/scripts/run-debian-tests.sh",
    );

    let original = read_repo_text("test-original.sh");
    assert_contains(
        &original,
        "missing package manifest: $PACKAGE_MANIFEST; run bash safe/scripts/build-debs.sh first",
        "test-original.sh",
    );
    assert_contains(
        &original,
        "required package artifact missing from $PACKAGE_OUT",
        "test-original.sh",
    );
}
