//! The key table of workspace.yaml and its checks.

use crate::output::escape;
use crate::yaml::{Entry, Error, Node, Pos};

const KEYS: [&str; 4] = ["name", "control", "agent", "repos"];
const REPO_KEYS: [&str; 3] = ["name", "url", "default_ref"];

/// Why workspace.yaml does not pass.
#[derive(Debug)]
pub enum Failure {
    /// A value or the document of the wrong type. The first one stops the check.
    Yaml(Error),
    /// The keys that are not accepted, one per line, all collected.
    Problems(Vec<Error>),
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

/// The string value of a key that the type checks already passed.
fn text<'a>(entries: &'a [Entry], key: &str) -> Option<&'a str> {
    find(entries, key).and_then(|node| node.as_str("the value").ok())
}

/// The key as a message names it, such as `name` or `repos[wd].url`.
fn path(scope: &str, key: &str) -> String {
    if scope.is_empty() {
        format!("`{}`", escape(key))
    } else {
        format!("`{scope}.{}`", escape(key))
    }
}

/// A mapping with its place in the file and its scope in messages.
struct Element<'a> {
    scope: String,
    pos: Pos,
    entries: &'a [Entry],
}

/// Checks the keys of a parsed workspace.yaml.
pub fn check(root: &Node) -> Result<(), Failure> {
    let top = root.as_map("the document")?;

    // The types of the keys in the key table. The first mismatch stops the check.
    let mut elements = Vec::new();
    for entry in top {
        match entry.key.as_str() {
            "name" | "control" | "agent" => {
                entry.value.as_str(&path("", &entry.key))?;
            }
            "repos" => {
                let items = entry.value.as_seq("`repos`")?;
                for (index, item) in items.iter().enumerate() {
                    let entries = item.as_map(&format!("`repos[#{}]`", index + 1))?;
                    let scope = element_scope(entries, index);
                    for field in entries {
                        if REPO_KEYS.contains(&field.key.as_str()) {
                            field.value.as_str(&path(&scope, &field.key))?;
                        }
                    }
                    elements.push(Element {
                        scope,
                        pos: item.pos(),
                        entries,
                    });
                }
            }
            _ => {}
        }
    }

    // Unknown keys, then missing keys. All are collected.
    let top_element = Element {
        scope: String::new(),
        pos: root.pos(),
        entries: top,
    };
    let scopes: Vec<(&Element, &[&str])> = std::iter::once((&top_element, &KEYS[..]))
        .chain(elements.iter().map(|element| (element, &REPO_KEYS[..])))
        .collect();
    let mut problems = Vec::new();
    for (element, keys) in &scopes {
        for entry in element.entries {
            if !keys.contains(&entry.key.as_str()) {
                problems.push(Error::new(
                    entry.key_pos,
                    format!("unknown key {}", path(&element.scope, &entry.key)),
                ));
            }
        }
    }
    for (element, keys) in &scopes {
        for key in *keys {
            if find(element.entries, key).is_none() {
                problems.push(Error::new(
                    element.pos,
                    format!("missing key {}", path(&element.scope, key)),
                ));
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
    match text(element, "name") {
        Some(name) => format!("repos[{}]", escape(name)),
        None => format!("repos[#{}]", index + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml;

    const VALID: &str = "name: autopilot
control: sandbox
agent: claude
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
            Err(Failure::Problems(problems)) => problems.iter().map(ToString::to_string).collect(),
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
    fn a_valid_workspace_passes() {
        assert!(run(VALID).is_ok());
    }

    #[test]
    fn unknown_keys_are_named_including_identity_features_and_image() {
        let text = format!("identity: me\nfeatures: [a]\nimage: base:1\n{VALID}");
        assert_eq!(
            problems(&text),
            [
                "line 1, column 1: unknown key `identity`",
                "line 2, column 1: unknown key `features`",
                "line 3, column 1: unknown key `image`",
            ]
        );
    }

    #[test]
    fn the_value_of_an_unknown_key_is_not_typed() {
        let text = format!("extra: {{a: [1, 2]}}\nnone:\n{VALID}");
        assert_eq!(
            problems(&text),
            [
                "line 1, column 1: unknown key `extra`",
                "line 2, column 1: unknown key `none`"
            ]
        );
    }

    #[test]
    fn unknown_keys_in_repos_elements_are_named_by_the_element() {
        let text = VALID.replace(
            "    default_ref: branch:develop\n  - name: wd",
            "    default_ref: branch:develop\n    extra: x\n  - name: wd",
        );
        assert_eq!(
            problems(&text),
            ["line 8, column 5: unknown key `repos[sandbox].extra`"]
        );
    }

    #[test]
    fn missing_keys_are_all_listed_one_per_line() {
        assert_eq!(
            problems("name: ab\nrepos:\n  - name: xy\n  - url: https://github.com/o/r\n  - {}\n"),
            [
                "line 1, column 1: missing key `control`",
                "line 1, column 1: missing key `agent`",
                "line 3, column 5: missing key `repos[xy].url`",
                "line 3, column 5: missing key `repos[xy].default_ref`",
                "line 4, column 5: missing key `repos[#2].name`",
                "line 4, column 5: missing key `repos[#2].default_ref`",
                "line 5, column 5: missing key `repos[#3].name`",
                "line 5, column 5: missing key `repos[#3].url`",
                "line 5, column 5: missing key `repos[#3].default_ref`",
            ]
        );
    }

    #[test]
    fn an_empty_document_misses_every_key() {
        assert_eq!(
            problems(""),
            [
                "line 1, column 1: missing key `name`",
                "line 1, column 1: missing key `control`",
                "line 1, column 1: missing key `agent`",
                "line 1, column 1: missing key `repos`",
            ]
        );
    }

    #[test]
    fn unknown_and_missing_keys_are_collected_together_unknown_first() {
        let text = "identity: me\nname: ab\nrepos:\n  - name: xy\n    extra: 1\n";
        assert_eq!(
            problems(text),
            [
                "line 1, column 1: unknown key `identity`",
                "line 5, column 5: unknown key `repos[xy].extra`",
                "line 1, column 1: missing key `control`",
                "line 1, column 1: missing key `agent`",
                "line 4, column 5: missing key `repos[xy].url`",
                "line 4, column 5: missing key `repos[xy].default_ref`",
            ]
        );
    }

    #[test]
    fn keys_and_names_are_escaped_to_one_line() {
        let text = "\"a\\nb\": 1\nname: ab\nrepos:\n  - name: \"x\\ny\"\n";
        let lines = problems(text);
        assert!(lines.contains(&"line 1, column 1: unknown key `a\\nb`".to_owned()));
        assert!(lines.contains(&"line 4, column 5: missing key `repos[x\\ny].url`".to_owned()));
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
            "line 2, column 5: `repos[#1]` must be a mapping, but it is a string"
        );
        assert_eq!(
            yaml_error("repos:\n  - name: x\n    url: true\n"),
            "line 3, column 10: `repos[x].url` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
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
