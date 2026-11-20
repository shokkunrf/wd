//! The key table of workspace.yaml and its checks.

use std::collections::HashSet;

use crate::image::Image;
use crate::name::Name;
use crate::output::{escape, quote};
use crate::repo_url::RepoUrl;
use crate::yaml::{Entry, Error, Node, Pos};

const KEYS: [&str; 5] = ["name", "control", "agent", "image", "repos"];
const REPO_KEYS: [&str; 3] = ["name", "url", "default_ref"];
const AGENTS: [&str; 1] = ["claude"];
const BRANCH_PREFIX: &str = "branch:";

/// Why workspace.yaml does not pass.
#[derive(Debug)]
pub enum Failure {
    /// The file does not fit the YAML format or a key has a value of the wrong type. The first
    /// one stops the check.
    Yaml(Error),
    /// The keys and values that are not accepted, one per line, all collected.
    Problems(Vec<Error>),
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        Failure::Yaml(err)
    }
}

/// A workspace.yaml that passed every check.
#[derive(Debug)]
#[expect(dead_code, reason = "up reads the workspace (#17)")]
pub struct Workspace {
    name: Name,
    control: Name,
    agent: String,
    image: Image,
    repos: Vec<Repo>,
}

#[derive(Debug)]
#[expect(dead_code, reason = "up reads the workspace (#17)")]
struct Repo {
    name: Name,
    url: RepoUrl,
    default_branch: String,
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

/// The key as it is named in a message: in backticks, with the element of `repos` it belongs to
/// as the path when `scope` is not empty.
fn path(scope: &str, key: &str) -> String {
    if scope.is_empty() {
        format!("`{}`", escape(key))
    } else {
        format!("`{scope}.{}`", escape(key))
    }
}

/// The value of `key`, when it is a string that passes `check`. A failure goes to `problems`
/// at the position of the value.
fn checked<T>(
    entries: &[Entry],
    scope: &str,
    key: &str,
    problems: &mut Vec<Error>,
    check: impl Fn(&str) -> Result<T, String>,
) -> Option<T> {
    let node = find(entries, key)?;
    let value = node.as_str("the value").ok()?;
    match check(value) {
        Ok(checked) => Some(checked),
        Err(reason) => {
            problems.push(Error::new(
                node.pos(),
                format!("{} is invalid: {reason}", path(scope, key)),
            ));
            None
        }
    }
}

/// A mapping with its place in the file and its scope in messages.
struct Element<'a> {
    scope: String,
    pos: Pos,
    entries: &'a [Entry],
}

/// Checks a parsed workspace.yaml.
pub fn check(root: &Node) -> Result<Workspace, Failure> {
    let top = root.as_map("the document")?;

    // The types of the keys in the key table. The first mismatch stops the check.
    let mut elements = Vec::new();
    let mut repos_node = None;
    for entry in top {
        match entry.key.as_str() {
            "name" | "control" | "agent" | "image" => {
                entry.value.as_str(&path("", &entry.key))?;
            }
            "repos" => {
                let items = entry.value.as_seq("`repos`")?;
                repos_node = Some((&entry.value, items.len()));
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

    // The keys that are not in the key table, then the keys that are missing, then the values.
    // All are collected.
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

    let mut formats = Vec::new();
    let name = checked(top, "", "name", &mut formats, Name::parse);
    let control = checked(top, "", "control", &mut formats, Name::parse);
    let agent = checked(top, "", "agent", &mut formats, check_agent);
    let image = checked(top, "", "image", &mut formats, Image::parse);
    let mut repos = Vec::new();
    let mut urls = Vec::new();
    for element in &elements {
        let (scope, entries) = (element.scope.as_str(), element.entries);
        let name = checked(entries, scope, "name", &mut formats, Name::parse);
        let url = checked(entries, scope, "url", &mut formats, RepoUrl::parse);
        let default_branch = checked(
            entries,
            scope,
            "default_ref",
            &mut formats,
            check_default_ref,
        );
        if let (Some(url), Some(node)) = (&url, find(entries, "url")) {
            urls.push((scope, node.pos(), url.clone()));
        }
        repos.push(
            name.zip(url)
                .zip(default_branch)
                .map(|((name, url), default_branch)| Repo {
                    name,
                    url,
                    default_branch,
                }),
        );
    }
    problems.append(&mut formats);

    // The reference of `control` to the names of repos[].
    let names: Vec<(&str, &str, Pos)> = elements
        .iter()
        .filter_map(|e| {
            let node = find(e.entries, "name")?;
            Some((e.scope.as_str(), node.as_str("the value").ok()?, node.pos()))
        })
        .collect();
    if let (Some(control), Some(node)) = (text(top, "control"), find(top, "control"))
        && !names.iter().any(|(_, name, _)| *name == control)
    {
        let reason = if names.is_empty() {
            format!(
                "`control` is {}, but `repos` has no elements",
                quote(control)
            )
        } else {
            let listed = names
                .iter()
                .map(|(_, n, _)| quote(n))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "`control` is {}, but no element of `repos` has that `name`; the names present are {listed}",
                quote(control)
            )
        };
        problems.push(Error::new(node.pos(), reason));
    }

    // Duplicate names, then duplicate identifiers. The later occurrence is the one reported.
    let mut seen = HashSet::new();
    for (scope, name, at) in &names {
        if !seen.insert(*name) {
            problems.push(Error::new(
                *at,
                format!(
                    "{} is {}, but an earlier element of `repos` has that `name`; give every element its own name",
                    path(scope, "name"),
                    quote(name)
                ),
            ));
        }
    }
    let mut seen: Vec<(&str, &RepoUrl)> = Vec::new();
    for (scope, at, url) in &urls {
        let id = url.identifier();
        if let Some((_, earlier)) = seen.iter().find(|(seen_id, _)| *seen_id == id) {
            problems.push(Error::new(
                *at,
                format!(
                    "{} is {}, which gives the identifier {} that the earlier `url` {} gives too; use a different repository",
                    path(scope, "url"),
                    quote(url.as_str()),
                    quote(id),
                    quote(earlier.as_str())
                ),
            ));
        } else {
            seen.push((id, url));
        }
    }

    if let Some((node, 0)) = repos_node {
        problems.push(Error::new(
            node.pos(),
            "`repos` is empty; it needs at least one element",
        ));
    }

    if !problems.is_empty() {
        return Err(Failure::Problems(problems));
    }
    let repos: Option<Vec<Repo>> = repos.into_iter().collect();
    match (name, control, agent, image, repos) {
        (Some(name), Some(control), Some(agent), Some(image), Some(repos)) => Ok(Workspace {
            name,
            control,
            agent,
            image,
            repos,
        }),
        _ => Err(Failure::Problems(vec![Error::new(
            root.pos(),
            "internal error: a checked value is missing",
        )])),
    }
}

fn check_agent(text: &str) -> Result<String, String> {
    if AGENTS.contains(&text) {
        Ok(text.to_owned())
    } else {
        let allowed: Vec<String> = AGENTS.iter().map(|a| quote(a)).collect();
        Err(format!(
            "{} is not an agent; the allowed words are {}",
            quote(text),
            allowed.join(", ")
        ))
    }
}

/// Checks `branch:<branch name>` and returns the branch name.
fn check_default_ref(text: &str) -> Result<String, String> {
    let Some(branch) = text.strip_prefix(BRANCH_PREFIX) else {
        return Err(format!(
            "{} must be \"branch:<branch name>\"; \"tag:\", \"pr:\" and \"sha:\" are not accepted",
            quote(text)
        ));
    };
    match branch_problem(branch) {
        None => Ok(branch.to_owned()),
        Some(why) => Err(format!("{} is not a branch name; {why}", quote(branch))),
    }
}

fn branch_problem(branch: &str) -> Option<&'static str> {
    if branch.is_empty() {
        return Some("it is empty");
    }
    if branch == "HEAD" || branch == "@" {
        return Some("it is reserved");
    }
    if branch.contains("..") {
        return Some("it contains \"..\"");
    }
    if branch.contains("@{") {
        return Some("it contains \"@{\"");
    }
    if branch.chars().any(|c| c.is_ascii_control()) {
        return Some("it contains a control character");
    }
    if branch.contains([' ', '~', '^', ':', '?', '*', '[', '\\']) {
        return Some("it contains a space or one of ~ ^ : ? * [ \\");
    }
    if branch.starts_with(['/', '-']) {
        return Some("it starts with \"/\" or \"-\"");
    }
    if branch.ends_with(['/', '.']) {
        return Some("it ends with \"/\" or \".\"");
    }
    if branch.contains("//") {
        return Some("it contains \"//\"");
    }
    if branch
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Some("a component starts with \".\" or ends with \".lock\"");
    }
    None
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
        check(&yaml::parse(text.as_bytes()).unwrap()).map(|_| ())
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
    fn the_example_of_the_spec_passes() {
        assert!(run(VALID).is_ok());
    }

    #[test]
    fn unknown_keys_are_named_including_identity_and_features() {
        let text = format!("identity: me\nfeatures: [a]\n{VALID}");
        assert_eq!(
            problems(&text),
            [
                "line 1, column 1: unknown key `identity`",
                "line 2, column 1: unknown key `features`"
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
            ["line 9, column 5: unknown key `repos[sandbox].extra`"]
        );
    }

    #[test]
    fn missing_keys_are_all_listed_one_per_line() {
        assert_eq!(
            problems("name: ab\nrepos:\n  - name: xy\n  - url: https://github.com/o/r\n  - {}\n"),
            [
                "line 1, column 1: missing key `control`",
                "line 1, column 1: missing key `agent`",
                "line 1, column 1: missing key `image`",
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
                "line 1, column 1: missing key `image`",
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
                "line 1, column 1: missing key `image`",
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

    #[test]
    fn default_ref_accepts_branches_and_rejects_the_table() {
        for ok in [
            "develop",
            "feature/a-b",
            "日本語",
            "a.b",
            "@a",
            "release/1.0",
            "a@b",
        ] {
            assert_eq!(check_default_ref(&format!("branch:{ok}")).unwrap(), ok);
        }
        for bad in [
            "", "a/.b", ".a", "a.lock", "a/b.lock", "a..b", "a\u{1f}b", "a\u{7f}b", "a b", "a~",
            "a^", "a:b", "a?", "a*", "a[", "a\\b", "a@{b", "@", "/a", "a/", "a//b", "a.", "-a",
            "HEAD",
        ] {
            assert!(
                check_default_ref(&format!("branch:{bad}")).is_err(),
                "{bad:?}"
            );
        }
        for bad in ["develop", "tag:v1", "pr:1", "sha:abc", "Branch:a"] {
            assert!(check_default_ref(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            check_default_ref("branch:HEAD").unwrap_err(),
            "\"HEAD\" is not a branch name; it is reserved"
        );
    }

    #[test]
    fn the_dangling_control_lists_the_names() {
        let text = VALID.replace("control: sandbox", "control: other");
        assert_eq!(
            problems(&text),
            [
                "line 2, column 10: `control` is \"other\", but no element of `repos` has that `name`; the names present are \"sandbox\", \"wd\""
            ]
        );
    }

    #[test]
    fn duplicates_are_reported_at_the_later_occurrence() {
        let text = VALID.replace("name: wd", "name: sandbox").replace(
            "git@github.com:shokkunrf/wd.git",
            "https://github.com/shokkunrf/AUTOPILOT-sandbox.git",
        );
        assert_eq!(
            problems(&text),
            [
                "line 9, column 11: `repos[sandbox].name` is \"sandbox\", but an earlier element of `repos` has that `name`; give every element its own name",
                "line 10, column 10: `repos[sandbox].url` is \"https://github.com/shokkunrf/AUTOPILOT-sandbox.git\", which gives the identifier \"github.com/shokkunrf/autopilot-sandbox\" that the earlier `url` \"git@github.com:shokkunrf/autopilot-sandbox.git\" gives too; use a different repository",
            ]
        );
    }

    #[test]
    fn an_invalid_value_is_reported_at_its_position() {
        let text = VALID.replace("agent: claude", "agent: gpt");
        assert_eq!(
            problems(&text),
            [
                "line 3, column 8: `agent` is invalid: \"gpt\" is not an agent; the allowed words are \"claude\""
            ]
        );
        let text = VALID.replace(
            "url: git@github.com:shokkunrf/wd.git",
            "url: https://example.com/o/r",
        );
        let lines = problems(&text);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("line 10, column 10: `repos[wd].url` is invalid: "),
            "{lines:?}"
        );
    }

    #[test]
    fn an_empty_repos_is_reported_at_its_value() {
        let text = "name: ab\ncontrol: cd\nagent: claude\nimage: i:1\nrepos: []\n";
        assert_eq!(
            problems(text),
            [
                "line 2, column 10: `control` is \"cd\", but `repos` has no elements",
                "line 5, column 8: `repos` is empty; it needs at least one element",
            ]
        );
    }

    #[test]
    fn urls_that_differ_only_in_case_and_git_share_an_identifier() {
        let text = VALID.replace(
            "git@github.com:shokkunrf/wd.git",
            "https://github.com/ShokkunRF/AUTOPILOT-SANDBOX.GIT",
        );
        let lines = problems(&text);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("\"github.com/shokkunrf/autopilot-sandbox\""),
            "{lines:?}"
        );
    }
}
