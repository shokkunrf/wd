//! The key table of workspace.yaml and its checks.

use crate::output::escape;
use crate::yaml::{Entry, Error, Node};

const KEYS: [&str; 5] = ["name", "control", "agent", "image", "repos"];
const REPO_KEYS: [&str; 3] = ["name", "url", "default_ref"];

/// Why workspace.yaml does not pass.
#[derive(Debug)]
pub enum Failure {
    /// The file does not fit the YAML format or a key has a value of the wrong type. The first
    /// one stops the check.
    Yaml(Error),
    /// The keys and values that are not accepted, one per line, all collected.
    Problems(Vec<String>),
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        Failure::Yaml(err)
    }
}

fn find<'a>(entries: &'a [Entry], key: &str) -> Option<&'a Node> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| &entry.value)
}

fn label(key: &str) -> String {
    format!("`{}`", escape(key))
}

/// Checks the keys of a parsed workspace.yaml.
pub fn check(root: &Node) -> Result<(), Failure> {
    let top = root.as_map("the document")?;

    // The types of the keys in the key table. The first mismatch stops the check.
    let mut elements = Vec::new();
    for entry in top {
        match entry.key.as_str() {
            "name" | "control" | "agent" | "image" => {
                entry.value.as_str(&label(&entry.key))?;
            }
            "repos" => {
                for item in entry.value.as_seq("`repos`")? {
                    let element = item.as_map("an element of `repos`")?;
                    for field in element {
                        if REPO_KEYS.contains(&field.key.as_str()) {
                            field.value.as_str(&format!(
                                "{} of an element of `repos`",
                                label(&field.key)
                            ))?;
                        }
                    }
                    elements.push(element);
                }
            }
            _ => {}
        }
    }

    // The keys that are not in the key table, then the keys that are missing. All are collected.
    let scopes: Vec<(String, &[Entry], &[&str])> = std::iter::once((String::new(), top, &KEYS[..]))
        .chain(
            elements
                .iter()
                .enumerate()
                .map(|(index, element)| (element_scope(element, index), *element, &REPO_KEYS[..])),
        )
        .collect();
    let mut problems = Vec::new();
    for (scope, entries, keys) in &scopes {
        for entry in *entries {
            if !keys.contains(&entry.key.as_str()) {
                problems.push(problem(
                    scope,
                    &format!("unknown key \"{}\"", escape(&entry.key)),
                ));
            }
        }
    }
    for (scope, entries, keys) in &scopes {
        for key in *keys {
            if find(entries, key).is_none() {
                problems.push(problem(scope, &format!("missing key \"{key}\"")));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Failure::Problems(problems))
    }
}

/// Points at an element of `repos` by its `name`, or by its place when it has none.
fn element_scope(element: &[Entry], index: usize) -> String {
    match find(element, "name").and_then(|name| name.as_str("`name`").ok()) {
        Some(name) => format!("repos[{}]", escape(name)),
        None => format!("repos[#{}]", index + 1),
    }
}

fn problem(scope: &str, reason: &str) -> String {
    if scope.is_empty() {
        reason.to_owned()
    } else {
        format!("{scope}: {reason}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml;

    const VALID: &str = "name: autopilot
control: sandbox
agent: claude
image: ghcr.io/shokkunrf/devcontainer-images/base:4.1.0
repos:
  - name: sandbox
    url: git@github.com:shokkunrf/autopilot-sandbox.git
    default_ref: branch:develop
  - name: wd
    url: git@github.com:shokkunrf/wd.git
    default_ref: branch:develop
";

    fn run(text: &str) -> Result<(), Failure> {
        check(&yaml::parse(text.as_bytes()).unwrap())
    }

    fn problems(text: &str) -> Vec<String> {
        match run(text) {
            Err(Failure::Problems(problems)) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    }

    fn yaml_error(text: &str) -> String {
        match run(text) {
            Err(Failure::Yaml(err)) => err.to_string(),
            other => panic!("expected a YAML error, got {other:?}"),
        }
    }

    #[test]
    fn the_example_of_the_spec_passes() {
        assert!(run(VALID).is_ok());
    }

    #[test]
    fn unknown_keys_are_named_including_identity_and_features() {
        let text = format!("identity: me\nfeatures: [a]\n{VALID}");
        assert_eq!(
            problems(&text),
            ["unknown key \"identity\"", "unknown key \"features\""]
        );
    }

    #[test]
    fn the_value_of_an_unknown_key_is_not_typed() {
        let text = format!("extra: {{a: [1, 2]}}\nnone:\n{VALID}");
        assert_eq!(
            problems(&text),
            ["unknown key \"extra\"", "unknown key \"none\""]
        );
    }

    #[test]
    fn unknown_keys_in_repos_elements_are_named_by_the_element() {
        let text = VALID.replace(
            "    default_ref: branch:develop\n  - name: wd",
            "    default_ref: branch:develop\n    extra: x\n  - name: wd",
        );
        assert_eq!(problems(&text), ["repos[sandbox]: unknown key \"extra\""]);
    }

    #[test]
    fn missing_keys_are_all_listed_one_per_line() {
        assert_eq!(
            problems("name: a\nrepos:\n  - name: x\n  - url: u\n  - {}\n"),
            [
                "missing key \"control\"",
                "missing key \"agent\"",
                "missing key \"image\"",
                "repos[x]: missing key \"url\"",
                "repos[x]: missing key \"default_ref\"",
                "repos[#2]: missing key \"name\"",
                "repos[#2]: missing key \"default_ref\"",
                "repos[#3]: missing key \"name\"",
                "repos[#3]: missing key \"url\"",
                "repos[#3]: missing key \"default_ref\"",
            ]
        );
    }

    #[test]
    fn an_empty_document_misses_every_key() {
        assert_eq!(
            problems(""),
            [
                "missing key \"name\"",
                "missing key \"control\"",
                "missing key \"agent\"",
                "missing key \"image\"",
                "missing key \"repos\"",
            ]
        );
    }

    #[test]
    fn unknown_and_missing_keys_are_collected_together_unknown_first() {
        let text = "identity: me\nname: a\nrepos:\n  - name: x\n    extra: 1\n";
        assert_eq!(
            problems(text),
            [
                "unknown key \"identity\"",
                "repos[x]: unknown key \"extra\"",
                "missing key \"control\"",
                "missing key \"agent\"",
                "missing key \"image\"",
                "repos[x]: missing key \"url\"",
                "repos[x]: missing key \"default_ref\"",
            ]
        );
    }

    #[test]
    fn keys_and_names_are_escaped_to_one_line() {
        let text = "\"a\\nb\": 1\nname: a\nrepos:\n  - name: \"x\\ny\"\n";
        let lines = problems(text);
        assert!(lines.contains(&"unknown key \"a\\nb\"".to_owned()));
        assert!(lines.contains(&"repos[x\\ny]: missing key \"url\"".to_owned()));
        assert!(lines.iter().all(|line| !line.contains('\n')));
    }

    #[test]
    fn a_type_mismatch_stops_at_the_first_one_before_the_key_checks() {
        let text = "identity: me\nname: true\nagent: 010\n";
        assert_eq!(
            yaml_error(text),
            "line 2, column 7: `name` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
        );
    }

    #[test]
    fn repos_must_be_a_sequence_of_mappings_with_string_values() {
        assert_eq!(
            yaml_error("repos: x\n"),
            "line 1, column 8: `repos` must be a sequence, but it is a string"
        );
        assert_eq!(
            yaml_error("repos:\n  - x\n"),
            "line 2, column 5: an element of `repos` must be a mapping, but it is a string"
        );
        assert_eq!(
            yaml_error("repos:\n  - name: x\n    url: true\n"),
            "line 3, column 10: `url` of an element of `repos` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
        );
        assert!(yaml_error("repos:\n  - name:\n").contains("it has no value"));
    }

    #[test]
    fn the_document_must_be_a_mapping() {
        assert_eq!(
            yaml_error("- a\n"),
            "line 1, column 1: the document must be a mapping, but it is a sequence"
        );
        assert!(yaml_error("x\n").contains("but it is a string"));
    }
}
