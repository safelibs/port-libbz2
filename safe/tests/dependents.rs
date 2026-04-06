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
const EXPECTED_AUTOPKGTESTS: &[&str] = &[
    "link-with-shared",
    "bigfile",
    "bzexe-test",
    "compare",
    "compress",
    "grep",
];
const EXPECTED_REPRESENTATIVE_DOWNSTREAMS: &[&str] = &[
    "libapt-pkg6.0t64",
    "bzip2",
    "libpython3.12-stdlib",
    "php8.3-bz2",
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

fn extract_autopkgtests(control: &str) -> Vec<String> {
    control
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Tests: "))
        .flat_map(|tests| tests.split_whitespace().map(str::to_string))
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
        "run_step 05-dlltest-object-release-gate run_direct_dlltest_object_gate",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$ROOT/target/original-baseline/dlltest.o\"",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$dlltest_exe\" -d \"$path_bz2_rel\" \"$tmpdir_rel/path.out\"",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$dlltest_exe\" -d < \"$stdio_bz2_rel\" > \"$tmpdir_rel/stdio.out\"",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$dlltest_exe\" \"$path_out_rel\" \"$tmpdir_rel/path.bz2\"",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "\"$dlltest_exe\" -1 < \"$stdio_out_rel\" > \"$tmpdir_rel/stdio.bz2\"",
        "safe/scripts/run-full-suite.sh",
    );
    assert_contains(
        &full_suite,
        "); then\n    status=0\n  else\n    status=$?\n  fi",
        "safe/scripts/run-full-suite.sh",
    );
    assert!(
        !full_suite.contains("if ! ("),
        "safe/scripts/run-full-suite.sh must not use `if ! (...)` around the direct dlltest.o gate because it masks the failing subshell status"
    );
    assert_contains(
        &full_suite,
        "run_step 10-test-original-all \"$ROOT/test-original.sh\"",
        "safe/scripts/run-full-suite.sh",
    );
    for dependent in EXPECTED_REPRESENTATIVE_DOWNSTREAMS {
        let needle = format!("\"$ROOT/test-original.sh\" --only {dependent}");
        assert_contains(&full_suite, &needle, "safe/scripts/run-full-suite.sh");
    }
    assert_eq!(
        full_suite.matches("\"$ROOT/test-original.sh\"").count(),
        EXPECTED_REPRESENTATIVE_DOWNSTREAMS.len() + 1,
        "safe/scripts/run-full-suite.sh should keep one full downstream matrix plus four representative downstream --only checks",
    );
    assert_eq!(
        full_suite.matches("\"$ROOT/test-original.sh\" --only ").count(),
        EXPECTED_REPRESENTATIVE_DOWNSTREAMS.len(),
        "safe/scripts/run-full-suite.sh should keep exactly four representative downstream --only checks",
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
    let expected_tests = EXPECTED_AUTOPKGTESTS
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        extract_autopkgtests(&debian_control),
        expected_tests,
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
    assert_contains(
        &layout,
        "unexpected staged Cargo target tree copied into $SRC",
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
