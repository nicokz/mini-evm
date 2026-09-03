#[path = "common/fixture_types.rs"]
mod fixture_types;
#[path = "runner/test_harness.rs"]
mod test_harness;

use colored::Colorize;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use test_harness::run_state_test_case;
use walkdir::WalkDir;

#[test]
fn execution_spec_vectors_match_cancun_roots() {
    let root = Path::new("tests/fixtures/GeneralStateTests");
    if !root.exists() {
        eprintln!(
            "{} fixture directory not present; skipping vector run",
            "SKIP".yellow()
        );
        return;
    }

    let mut checked = 0usize;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file()
            || !(path.extension().is_some_and(|ext| ext == "json")
                || path.extension().is_some_and(|ext| ext == "gz"))
        {
            continue;
        }
        let bytes =
            read_fixture(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let fixture: fixture_types::TestFixture = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for (name, case) in fixture {
            let Some(expectations) = case.post.get("Cancun") else {
                continue;
            };
            let root_hash = run_state_test_case(&case, "Cancun")
                .unwrap_or_else(|error| panic!("{} ({name}): {error:?}", path.display()));
            let actual = format!("0x{}", hex::encode(root_hash));
            for expectation in expectations {
                checked += 1;
                assert_eq!(
                    actual,
                    expectation.hash.to_ascii_lowercase(),
                    "{} -> {name}",
                    path.display()
                );
            }
            println!(
                "{} {} -> root matches {}",
                "[PASS]".green(),
                path.display(),
                actual
            );
        }
    }
    assert!(
        checked > 0,
        "no Cancun vectors found under {}",
        root.display()
    );
}

fn read_fixture(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    if path.extension().is_some_and(|ext| ext == "gz") {
        let mut decoder = GzDecoder::new(file);
        let mut bytes = Vec::new();
        decoder.read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        let mut bytes = Vec::new();
        let mut file = file;
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
