use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const DLLTEST_BZ2_UNDEFINED: &[&str] = &[
    "BZ2_bzRead",
    "BZ2_bzReadClose",
    "BZ2_bzReadOpen",
    "BZ2_bzWrite",
    "BZ2_bzWriteClose",
    "BZ2_bzWriteOpen",
];

static BUILD_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
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

fn run_checked(command: &mut Command, context: &str) {
    let child = command.spawn().unwrap();
    let output = child.wait_with_output().unwrap();
    if !output.status.success() {
        panic!(
            "{} failed\nstdout:\n{}\nstderr:\n{}",
            context,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

fn bz2_undefined_symbols(object: &Path) -> Vec<String> {
    undefined_symbols(object)
        .into_iter()
        .filter(|symbol| symbol.starts_with("BZ2_"))
        .collect()
}

fn bz2_undefined_count(object: &Path) -> usize {
    bz2_undefined_symbols(object).len()
}

fn compat_ld_library_path(compat: &Path) -> String {
    match std::env::var("LD_LIBRARY_PATH") {
        Ok(existing) if !existing.is_empty() => format!("{}:{existing}", compat.display()),
        _ => compat.display().to_string(),
    }
}

fn temp_dir(repo: &Path, label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = repo.join("target").join(format!(
        "link-contract-{label}-{stamp}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn repo_relative<'a>(repo: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(repo).unwrap_or(path)
}

fn assert_same_file(actual: &Path, expected: &Path) {
    assert_eq!(
        fs::read(actual).unwrap(),
        fs::read(expected).unwrap(),
        "file contents differed: actual={} expected={}",
        actual.display(),
        expected.display()
    );
}

#[test]
fn selected_object_files_still_match_captured_undefined_sets() {
    let repo = repo_root();
    let baseline = repo.join("target/original-baseline");
    let public_api_object = baseline.join("public_api_test.o");
    let cli_object = baseline.join("bzip2.o");
    let dlltest_object = baseline.join("dlltest.o");

    assert!(
        dlltest_object.is_file(),
        "phase 1 contract failure: missing {}",
        dlltest_object.display()
    );

    let expected_public_api =
        fs::read_to_string(repo.join("safe/abi/original.public_api_undefined.txt")).unwrap();
    let expected_cli =
        fs::read_to_string(repo.join("safe/abi/original.cli_undefined.txt")).unwrap();

    let actual_public_api = undefined_symbols(&public_api_object).join("\n");
    let actual_cli = undefined_symbols(&cli_object).join("\n");

    assert_eq!(actual_public_api, expected_public_api.trim_end());
    assert_eq!(actual_cli, expected_cli.trim_end());
    assert_eq!(
        bz2_undefined_symbols(&dlltest_object),
        DLLTEST_BZ2_UNDEFINED
            .iter()
            .map(|symbol| (*symbol).to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(bz2_undefined_count(&public_api_object), 23);
    assert_eq!(bz2_undefined_count(&cli_object), 8);
    assert_eq!(bz2_undefined_count(&dlltest_object), 6);
}

#[test]
fn dlltest_object_relinks_and_runs_against_the_staged_shared_object() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let repo = repo_root();
    let compat = repo.join("target/compat");
    let baseline = repo.join("target/original-baseline");
    let object = baseline.join("dlltest.o");
    let shared_object = compat.join("libbz2.so.1.0.4");
    let executable = compat.join("dlltest-object-contract");
    let ld_library_path = compat_ld_library_path(&compat);

    assert!(
        object.is_file(),
        "phase 1 contract failure: missing {}",
        object.display()
    );

    run(&repo, "bash", &["safe/scripts/build-safe.sh"]);
    assert!(shared_object.is_file());

    let mut link = Command::new("gcc");
    link.current_dir(&repo)
        .arg("-o")
        .arg(&executable)
        .arg(&object)
        .arg("-Wl,-rpath,$ORIGIN")
        .arg(&shared_object);
    run_checked(
        &mut link,
        "link target/original-baseline/dlltest.o against target/compat/libbz2.so.1.0.4",
    );

    let tmpdir = temp_dir(&repo, "dlltest-object");
    let path_out = tmpdir.join("path.out");
    let stdio_out = tmpdir.join("stdio.out");
    let path_bz2 = tmpdir.join("path.bz2");
    let stdio_bz2 = tmpdir.join("stdio.bz2");

    let baseline_path_bz2 = baseline.join("dlltest-path.bz2");
    let baseline_path_out = baseline.join("dlltest-path.out");
    let baseline_stdio_bz2 = baseline.join("dlltest-stdio.bz2");
    let baseline_stdio_out = baseline.join("dlltest-stdio.out");
    let path_read_arg = repo_relative(&repo, &baseline_path_bz2);
    let path_out_arg = repo_relative(&repo, &baseline_path_out);
    let tmpdir_arg = repo_relative(&repo, &tmpdir);

    let mut path_read = Command::new(&executable);
    path_read
        .current_dir(&repo)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .arg("-d")
        .arg(path_read_arg)
        .arg(tmpdir_arg.join("path.out"));
    run_checked(&mut path_read, "run dlltest.o path read mode");
    assert_same_file(&path_out, &baseline_path_out);

    let mut stdio_read = Command::new(&executable);
    stdio_read
        .current_dir(&repo)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .arg("-d")
        .stdin(Stdio::from(File::open(&baseline_stdio_bz2).unwrap()))
        .stdout(Stdio::from(File::create(&stdio_out).unwrap()));
    run_checked(&mut stdio_read, "run dlltest.o stdio read mode");
    assert_same_file(&stdio_out, &baseline_stdio_out);

    let mut path_write = Command::new(&executable);
    path_write
        .current_dir(&repo)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .arg(path_out_arg)
        .arg(tmpdir_arg.join("path.bz2"));
    run_checked(&mut path_write, "run dlltest.o path write mode");
    assert_same_file(&path_bz2, &baseline_path_bz2);

    let mut stdio_write = Command::new(&executable);
    stdio_write
        .current_dir(&repo)
        .env("LD_LIBRARY_PATH", &ld_library_path)
        .arg("-1")
        .stdin(Stdio::from(File::open(&baseline_stdio_out).unwrap()))
        .stdout(Stdio::from(File::create(&stdio_bz2).unwrap()));
    run_checked(&mut stdio_write, "run dlltest.o stdio write mode");
    assert_same_file(&stdio_bz2, &baseline_stdio_bz2);

    let _ = fs::remove_dir_all(&tmpdir);
}

#[test]
fn source_and_object_link_contracts_run_against_the_safe_library() {
    let _guard = BUILD_LOCK.lock().unwrap();
    let repo = repo_root();
    run(&repo, "bash", &["safe/scripts/build-safe.sh"]);
    run(
        &repo,
        "bash",
        &["safe/scripts/link-original-tests.sh", "--all"],
    );
}
