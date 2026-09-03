#[path = "common/fixture_types.rs"]
mod fixture_types;
#[path = "common/state_test_types.rs"]
mod state_test_types;
#[path = "runner/test_harness.rs"]
mod test_harness;

use colored::Colorize;
use flate2::read::GzDecoder;
use state_test_types::TestSuite;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use test_harness::run_state_test_case;
use walkdir::WalkDir;

#[test]
fn test_cancun_official_vectors() {
    let root = Path::new("tests/fixtures/ethereum_tests/GeneralStateTests");
    if !root.exists() {
        eprintln!(
            "{} official fixture submodule not present; skipping vector run",
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
        let suite: TestSuite = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for (name, case) in suite.0 {
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
        }
    }
    if checked == 0 {
        eprintln!(
            "{} no Cancun vectors found under {}; skipping",
            "SKIP".yellow(),
            root.display()
        );
    }
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
