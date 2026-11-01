use std::path::Path;

const WRITE_FIXTURES_VARIABLE: &str = "P9_WRITE_FIXTURES";
const DIFFERENCES_SHOWN: usize = 5;

/// Compares `actual` with `tests/fixtures/<name>`. A missing fixture is created only when `P9_WRITE_FIXTURES=1`
/// and the test still fails, so a person reads the file before it is frozen. An existing fixture is never rewritten.
pub(crate) fn assert_matches_fixture(name: &str, actual: &str) {
    let create = std::env::var(WRITE_FIXTURES_VARIABLE).is_ok_and(|value| value == "1");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if let Err(message) = check(&root, name, actual, create) {
        panic!("{message}");
    }
}

fn check(root: &Path, name: &str, actual: &str, create: bool) -> Result<(), String> {
    let path = root.join(name);
    match std::fs::read_to_string(&path) {
        Ok(expected) if expected == actual => Ok(()),
        Ok(expected) => Err(format!(
            "fixture {} differs:\n{}",
            path.display(),
            line_differences(&expected, actual)
        )),
        Err(_) if create => create_fixture(&path, actual),
        Err(_) => Err(format!(
            "missing fixture {}; create it once with {WRITE_FIXTURES_VARIABLE}=1 cargo test <test name>",
            path.display()
        )),
    }
}

fn create_fixture(path: &Path, actual: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or(path);
    std::fs::create_dir_all(parent)
        .and_then(|()| std::fs::write(path, actual))
        .map_err(|error| format!("cannot create fixture {}: {error}", path.display()))?;
    Err(format!("fixture created, review it: {}", path.display()))
}

fn line_differences(expected: &str, actual: &str) -> String {
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut differences = Vec::new();
    let mut number = 0;
    loop {
        number += 1;
        let (left, right) = (expected_lines.next(), actual_lines.next());
        if left.is_none() && right.is_none() {
            break;
        }
        if left != right {
            differences.push(format!(
                "line {number}: expected {}, actual {}",
                describe_line(left),
                describe_line(right)
            ));
        }
        if differences.len() == DIFFERENCES_SHOWN {
            break;
        }
    }
    if differences.is_empty() {
        return "only the line endings differ (a trailing newline)".to_string();
    }
    differences.join("\n")
}

fn describe_line(line: Option<&str>) -> String {
    line.map_or("<no line>".to_string(), |text| format!("{text:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    #[test]
    fn should_fail_a_missing_fixture_and_say_how_to_create_it() {
        let root = TempDir::new("fixture-missing");

        let result = check(root.path(), "scenario.log", "actual\n", false);

        let message = result.unwrap_err();
        assert!(message.contains("P9_WRITE_FIXTURES=1"), "{message}");
    }

    #[test]
    fn should_create_a_missing_fixture_only_when_asked_and_still_fail() {
        let root = TempDir::new("fixture-create");

        let result = check(root.path(), "nested/scenario.log", "actual\n", true);

        let message = result.unwrap_err();
        let created = std::fs::read_to_string(root.path().join("nested/scenario.log"));
        assert_eq!(
            (message.contains("fixture created, review it"), created.ok()),
            (true, Some("actual\n".to_string()))
        );
    }

    #[test]
    fn should_show_the_difference_and_never_rewrite_an_existing_fixture() {
        let root = TempDir::new("fixture-differs");
        std::fs::write(root.path().join("scenario.log"), "same\nexpected\n").unwrap();

        let result = check(root.path(), "scenario.log", "same\nactual\n", true);

        let message = result.unwrap_err();
        let kept = std::fs::read_to_string(root.path().join("scenario.log")).unwrap();
        assert_eq!(
            (
                message.contains("line 2: expected \"expected\", actual \"actual\""),
                kept
            ),
            (true, "same\nexpected\n".to_string())
        );
    }
}
