//! The key table of workspace.yaml and its checks.

use std::collections::HashSet;

use crate::image::Image;
use crate::name::Name;
use crate::output::{escape, quote};
use crate::repo_url::RepoUrl;
use crate::yaml::{Entry, Error, Node};

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
    Problems(Vec<String>),
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

fn label(key: &str) -> String {
    format!("`{}`", escape(key))
}

/// Checks a parsed workspace.yaml.
pub fn check(root: &Node) -> Result<Workspace, Failure> {
    let top = root.as_map("the document")?;

    // The types of the keys in the key table. The first mismatch stops the check.
    let mut elements = Vec::new();
    let mut repos_len = None;
    for entry in top {
        match entry.key.as_str() {
            "name" | "control" | "agent" | "image" => {
                entry.value.as_str(&label(&entry.key))?;
            }
            "repos" => {
                let items = entry.value.as_seq("`repos`")?;
                repos_len = Some(items.len());
                for item in items {
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

    // The keys that are not in the key table, then the keys that are missing, then the values.
    // All are collected.
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

    let mut formats = Vec::new();
    let mut invalid = |scope: &str, key: &str, reason: String| {
        formats.push(problem(
            scope,
            &format!("invalid value of \"{key}\": {reason}"),
        ));
    };
    let name =
        text(top, "name").and_then(|v| Name::parse(v).map_err(|r| invalid("", "name", r)).ok());
    let control = text(top, "control")
        .and_then(|v| Name::parse(v).map_err(|r| invalid("", "control", r)).ok());
    let agent =
        text(top, "agent").and_then(|v| check_agent(v).map_err(|r| invalid("", "agent", r)).ok());
    let image =
        text(top, "image").and_then(|v| Image::parse(v).map_err(|r| invalid("", "image", r)).ok());
    let mut repos = Vec::new();
    for (scope, entries, _) in scopes.iter().skip(1) {
        let name = text(entries, "name")
            .and_then(|v| Name::parse(v).map_err(|r| invalid(scope, "name", r)).ok());
        let url = text(entries, "url")
            .and_then(|v| RepoUrl::parse(v).map_err(|r| invalid(scope, "url", r)).ok());
        let default_branch = text(entries, "default_ref").and_then(|v| {
            check_default_ref(v)
                .map_err(|r| invalid(scope, "default_ref", r))
                .ok()
        });
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
    let names: Vec<&str> = elements.iter().filter_map(|e| text(e, "name")).collect();
    if let Some(control) = text(top, "control")
        && !names.contains(&control)
    {
        let listed = if names.is_empty() {
            "none".to_owned()
        } else {
            names
                .iter()
                .map(|n| quote(n))
                .collect::<Vec<_>>()
                .join(", ")
        };
        problems.push(format!(
            "`control` is {}, but no element of `repos` has that `name`; the names present are {listed}",
            quote(control)
        ));
    }

    // Duplicate names, then duplicate identifiers.
    let mut seen = HashSet::new();
    let mut reported = HashSet::new();
    for name in &names {
        if !seen.insert(*name) && reported.insert(*name) {
            problems.push(format!(
                "`name` {} is used by more than one element of `repos`",
                quote(name)
            ));
        }
    }
    let urls: Vec<RepoUrl> = repos.iter().flatten().map(|r| r.url.clone()).collect();
    let mut reported = HashSet::new();
    for url in &urls {
        let id = url.identifier();
        let same: Vec<&RepoUrl> = urls.iter().filter(|u| u.identifier() == id).collect();
        if same.len() > 1 && reported.insert(id) {
            let listed: Vec<String> = same.iter().map(|u| quote(u.as_str())).collect();
            problems.push(format!(
                "the identifier {} is derived from more than one `url`: {}",
                quote(id),
                listed.join(", ")
            ));
        }
    }

    if repos_len == Some(0) {
        problems.push("`repos` is empty; it needs at least one element".to_owned());
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
        _ => Err(Failure::Problems(vec![
            "internal error: a checked value is missing".to_owned(),
        ])),
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
        check(&yaml::parse(text.as_bytes()).unwrap()).map(|_| ())
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
            problems("name: ab\nrepos:\n  - name: xy\n  - url: https://github.com/o/r\n  - {}\n"),
            [
                "missing key \"control\"",
                "missing key \"agent\"",
                "missing key \"image\"",
                "repos[xy]: missing key \"url\"",
                "repos[xy]: missing key \"default_ref\"",
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
        let text = "identity: me\nname: ab\nrepos:\n  - name: xy\n    extra: 1\n";
        assert_eq!(
            problems(text),
            [
                "unknown key \"identity\"",
                "repos[xy]: unknown key \"extra\"",
                "missing key \"control\"",
                "missing key \"agent\"",
                "missing key \"image\"",
                "repos[xy]: missing key \"url\"",
                "repos[xy]: missing key \"default_ref\"",
            ]
        );
    }

    #[test]
    fn keys_and_names_are_escaped_to_one_line() {
        let text = "\"a\\nb\": 1\nname: ab\nrepos:\n  - name: \"x\\ny\"\n";
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
    fn the_checked_workspace_is_built_when_everything_passes() {
        assert!(check(&yaml::parse(VALID.as_bytes()).unwrap()).is_ok());
    }

    #[test]
    fn the_dangling_control_lists_the_names() {
        let text = VALID.replace("control: sandbox", "control: other");
        assert_eq!(
            problems(&text),
            [
                "`control` is \"other\", but no element of `repos` has that `name`; the names present are \"sandbox\", \"wd\""
            ]
        );
    }

    #[test]
    fn duplicates_are_reported_once_each() {
        let text = VALID.replace("name: wd", "name: sandbox").replace(
            "git@github.com:shokkunrf/wd.git",
            "https://github.com/shokkunrf/AUTOPILOT-sandbox.git",
        );
        let lines = problems(&text);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("`name` \"sandbox\" is used by more than one"));
        assert!(lines[1].contains("\"github.com/shokkunrf/autopilot-sandbox\""));
        assert!(lines[1].contains("git@github.com:shokkunrf/autopilot-sandbox.git"));
        assert!(lines[1].contains("https://github.com/shokkunrf/AUTOPILOT-sandbox.git"));
    }

    #[test]
    fn an_empty_repos_is_reported() {
        let text = "name: ab\ncontrol: cd\nagent: claude\nimage: i:1\nrepos: []\n";
        let lines = problems(text);
        assert!(lines.iter().any(|l| l.starts_with("`repos` is empty")));
        assert!(lines.iter().any(|l| l.contains("names present are none")));
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
