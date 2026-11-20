//! The identity ledger, `~/.config/wd/identities.yaml`. This module checks the contents it is
//! given and opens no file, so the caller opens the path.
#![cfg_attr(not(test), expect(dead_code, reason = "up reads the ledger (#17)"))]

use crate::name::Name;
use crate::output::{escape, quote};
use crate::yaml::{self, Entry, Error, Node, Pos};

const APP_KEYS: [&str; 6] = ["type", "login", "email", "app_id", "installation_id", "pem"];
const USER_KEYS: [&str; 3] = ["type", "login", "email"];

/// Why the ledger does not pass.
#[derive(Debug)]
pub enum Failure {
    /// The file does not fit the YAML format or a value has the wrong type. The first one stops
    /// the check.
    Yaml(Error),
    /// The problems of the ledger and of the selected entry, one per line, all collected.
    Problems(Vec<Error>),
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        Failure::Yaml(err)
    }
}

/// The selected entry of the ledger, after every check.
#[derive(Debug)]
#[expect(dead_code, reason = "up reads the identity (#17)")]
pub struct Identity {
    login: String,
    email: String,
    kind: Kind,
}

#[derive(Debug)]
#[expect(dead_code, reason = "up reads the identity (#17)")]
enum Kind {
    App {
        app_id: i64,
        installation_id: i64,
        pem: String,
    },
    User,
}

/// Reads the ledger and checks the entry of `selected`. The other entries are checked for the
/// YAML format, the key and whether each value is a string or an integer.
pub fn read(bytes: &[u8], selected: &Name) -> Result<Identity, Failure> {
    let root = yaml::parse(bytes)?;
    let top = root.as_map("the document")?;
    let mut problems = Vec::new();
    let mut entries: &[Entry] = &[];
    let mut identities_pos = root.pos();
    for entry in top {
        if entry.key == "identities" {
            entries = entry.value.as_map("`identities`")?;
            identities_pos = entry.value.pos();
        } else {
            problems.push(Error::new(
                entry.key_pos,
                format!("unknown key `{}`", escape(&entry.key)),
            ));
        }
    }

    let mut chosen = None;
    for entry in entries {
        let fields = entry
            .value
            .as_map(&format!("`identities[{}]`", escape(&entry.key)))?;
        if let Err(reason) = Name::parse(&entry.key) {
            problems.push(Error::new(
                entry.key_pos,
                format!(
                    "the name of `identities[{}]` is invalid: {reason}",
                    escape(&entry.key)
                ),
            ));
        }
        if entry.key == selected.as_str() {
            chosen = Some((entry.value.pos(), fields));
        } else {
            for field in fields {
                field.value.check_string_or_int(&format!(
                    "`identities[{}].{}`",
                    escape(&entry.key),
                    escape(&field.key)
                ))?;
            }
        }
    }

    let Some((entry_pos, fields)) = chosen else {
        let keys: Vec<String> = entries.iter().map(|e| quote(&e.key)).collect();
        let present = if keys.is_empty() {
            "the ledger has no entries".to_owned()
        } else {
            format!("the keys present are {}", keys.join(", "))
        };
        problems.push(Error::new(
            identities_pos,
            format!(
                "the identity {} is not in the ledger; {present}; add an entry with that key under `identities`",
                quote(selected.as_str())
            ),
        ));
        return Err(Failure::Problems(problems));
    };
    let scope = format!("identities[{}]", escape(selected.as_str()));
    let identity = check_entry(&scope, entry_pos, fields, &mut problems);
    match identity {
        Some(identity) if problems.is_empty() => Ok(identity),
        _ => Err(Failure::Problems(problems)),
    }
}

/// The key as it is named in a message, with the entry as the path.
fn path(scope: &str, key: &str) -> String {
    format!("`{scope}.{}`", escape(key))
}

fn find<'a>(fields: &'a [Entry], key: &str) -> Option<&'a Node> {
    fields
        .iter()
        .find(|field| field.key == key)
        .map(|field| &field.value)
}

/// Checks the keys and values of one entry. The result is `None` when a problem was found.
fn check_entry(
    scope: &str,
    entry_pos: Pos,
    fields: &[Entry],
    problems: &mut Vec<Error>,
) -> Option<Identity> {
    let kind = string(scope, fields, "type", problems, check_type);
    let keys: &[&str] = match kind.as_deref() {
        Some("app") => &APP_KEYS,
        Some("user") => &USER_KEYS,
        _ => {
            if find(fields, "type").is_none() {
                problems.push(Error::new(
                    entry_pos,
                    format!("missing key {}", path(scope, "type")),
                ));
            }
            return None;
        }
    };
    for field in fields {
        if !keys.contains(&field.key.as_str()) {
            problems.push(Error::new(
                field.key_pos,
                format!("unknown key {}", path(scope, &field.key)),
            ));
        }
    }
    for key in keys {
        if find(fields, key).is_none() {
            problems.push(Error::new(
                entry_pos,
                format!("missing key {}", path(scope, key)),
            ));
        }
    }

    let login = string(scope, fields, "login", problems, check_login);
    let email = string(scope, fields, "email", problems, check_email);
    let source = if kind.as_deref() == Some("app") {
        let pem = string(scope, fields, "pem", problems, check_pem);
        let app_id = positive(scope, fields, "app_id", problems);
        let installation_id = positive(scope, fields, "installation_id", problems);
        match (app_id, installation_id, pem) {
            (Some(app_id), Some(installation_id), Some(pem)) => Some(Kind::App {
                app_id,
                installation_id,
                pem,
            }),
            _ => None,
        }
    } else {
        Some(Kind::User)
    };
    match (login, email, source) {
        (Some(login), Some(email), Some(kind)) => Some(Identity { login, email, kind }),
        _ => None,
    }
}

/// A string value of `key` that passes `check`. A missing key is reported by the caller.
fn string(
    scope: &str,
    fields: &[Entry],
    key: &str,
    problems: &mut Vec<Error>,
    check: fn(&str) -> Result<(), String>,
) -> Option<String> {
    let node = find(fields, key)?;
    match node.as_str(&path(scope, key)) {
        Err(err) => {
            problems.push(err);
            None
        }
        Ok(value) => match check(value) {
            Ok(()) => Some(value.to_owned()),
            Err(reason) => {
                problems.push(Error::new(
                    node.pos(),
                    format!("{} is invalid: {reason}", path(scope, key)),
                ));
                None
            }
        },
    }
}

/// An integer value of `key` that is 1 or more.
fn positive(scope: &str, fields: &[Entry], key: &str, problems: &mut Vec<Error>) -> Option<i64> {
    let node = find(fields, key)?;
    match node.as_int(&path(scope, key)) {
        Err(err) => {
            problems.push(err);
            None
        }
        Ok(value) if value >= 1 => Some(value),
        Ok(value) => {
            problems.push(Error::new(
                node.pos(),
                format!(
                    "{} is invalid: {value} is not 1 or more; use a positive number",
                    path(scope, key)
                ),
            ));
            None
        }
    }
}

fn check_type(value: &str) -> Result<(), String> {
    if matches!(value, "app" | "user") {
        Ok(())
    } else {
        Err(format!(
            "{} is not a type; use \"app\" or \"user\"",
            quote(value)
        ))
    }
}

fn is_edge(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric())
}

/// `[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\[bot\])?`
fn check_login(value: &str) -> Result<(), String> {
    let base = value.strip_suffix("[bot]").unwrap_or(value);
    if is_edge(base.chars().next())
        && is_edge(base.chars().next_back())
        && base.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        Ok(())
    } else {
        Err(format!(
            "{} is not a GitHub login; use letters, digits and hyphens, starting and ending with a letter or digit, with an optional \"[bot]\"",
            quote(value)
        ))
    }
}

/// `<local>@<host>`
fn check_email(value: &str) -> Result<(), String> {
    let ok = value.split_once('@').is_some_and(|(local, host)| {
        !local.is_empty()
            && local.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '[' | ']' | '-')
            })
            && is_edge(host.chars().next())
            && is_edge(host.chars().next_back())
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            && !host.contains("..")
    });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{} is not an email address of the form <local>@<host>",
            quote(value)
        ))
    }
}

fn check_pem(value: &str) -> Result<(), String> {
    if value.starts_with('/') {
        Ok(())
    } else {
        Err(format!(
            "{} is not an absolute path; it must start with \"/\"",
            quote(value)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = "identities:
  bot:
    type: app
    app_id: 123456
    installation_id: 7890123
    pem: /home/me/.config/wd/bot.pem
    login: bot[bot]
    email: 123456+bot[bot]@users.noreply.github.com
  me:
    type: user
    login: me
    email: me@example.com
";

    fn name(text: &str) -> Name {
        Name::parse(text).unwrap()
    }

    fn problems(text: &str, selected: &str) -> Vec<String> {
        match read(text.as_bytes(), &name(selected)) {
            Err(Failure::Problems(problems)) => problems.iter().map(ToString::to_string).collect(),
            other => panic!("expected problems, got {other:?}"),
        }
    }

    fn yaml_error(text: &str, selected: &str) -> String {
        match read(text.as_bytes(), &name(selected)) {
            Err(Failure::Yaml(err)) => err.to_string(),
            other => panic!("expected a YAML error, got {other:?}"),
        }
    }

    #[test]
    fn the_example_of_the_spec_passes_for_both_types() {
        let app = read(LEDGER.as_bytes(), &name("bot")).unwrap();
        assert_eq!(
            format!("{app:?}"),
            "Identity { login: \"bot[bot]\", email: \"123456+bot[bot]@users.noreply.github.com\", kind: App { app_id: 123456, installation_id: 7890123, pem: \"/home/me/.config/wd/bot.pem\" } }"
        );
        let user = read(LEDGER.as_bytes(), &name("me")).unwrap();
        assert_eq!(
            format!("{user:?}"),
            "Identity { login: \"me\", email: \"me@example.com\", kind: User }"
        );
    }

    #[test]
    fn quoted_and_hex_integers_are_accepted() {
        let text = LEDGER
            .replace("app_id: 123456", "app_id: \"123456\"")
            .replace("installation_id: 7890123", "installation_id: 0x10");
        assert!(read(text.as_bytes(), &name("bot")).is_ok());
    }

    #[test]
    fn a_file_without_identities_has_no_entries() {
        for text in ["", "# nothing\n", "identities: {}\n"] {
            let lines = problems(text, "me");
            assert_eq!(lines.len(), 1, "{lines:?}");
            assert!(
                lines[0].ends_with(
                    "the identity \"me\" is not in the ledger; the ledger has no entries; add an entry with that key under `identities`"
                ),
                "{lines:?}"
            );
        }
    }

    #[test]
    fn a_missing_selected_entry_lists_the_keys() {
        assert_eq!(
            problems(LEDGER, "other"),
            [
                "line 2, column 3: the identity \"other\" is not in the ledger; the keys present are \"bot\", \"me\"; add an entry with that key under `identities`"
            ]
        );
    }

    #[test]
    fn only_identities_is_a_top_level_key() {
        let text = format!("extra: 1\n{LEDGER}");
        assert_eq!(
            problems(&text, "me"),
            ["line 1, column 1: unknown key `extra`"]
        );
    }

    #[test]
    fn a_key_that_is_not_canonical_is_reported_for_every_entry() {
        let text = LEDGER.replace("  bot:", "  Bot:");
        let lines = problems(&text, "me");
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with(
                "line 2, column 3: the name of `identities[Bot]` is invalid: \"Bot\" is not a canonical name"
            ),
            "{lines:?}"
        );
    }

    #[test]
    fn the_selected_entry_misses_keys_and_has_unknown_keys() {
        let text = "identities:\n  bot:\n    type: app\n    login: me\n    extra: 1\n";
        assert_eq!(
            problems(text, "bot"),
            [
                "line 5, column 5: unknown key `identities[bot].extra`",
                "line 3, column 5: missing key `identities[bot].email`",
                "line 3, column 5: missing key `identities[bot].app_id`",
                "line 3, column 5: missing key `identities[bot].installation_id`",
                "line 3, column 5: missing key `identities[bot].pem`",
            ]
        );
    }

    #[test]
    fn a_user_entry_rejects_the_keys_of_the_app_type() {
        let text = LEDGER.replace(
            "    login: me\n",
            "    login: me\n    pem: /x\n    app_id: 1\n",
        );
        assert_eq!(
            problems(&text, "me"),
            [
                "line 12, column 5: unknown key `identities[me].pem`",
                "line 13, column 5: unknown key `identities[me].app_id`"
            ]
        );
    }

    #[test]
    fn the_type_is_required_and_checked() {
        let missing = "identities:\n  me:\n    login: me\n";
        assert_eq!(
            problems(missing, "me"),
            ["line 3, column 5: missing key `identities[me].type`"]
        );
        let bad = "identities:\n  me:\n    type: robot\n    login: me\n";
        assert_eq!(
            problems(bad, "me"),
            [
                "line 3, column 11: `identities[me].type` is invalid: \"robot\" is not a type; use \"app\" or \"user\""
            ]
        );
    }

    #[test]
    fn values_that_do_not_fit_their_format_are_named() {
        for (from, to, key) in [
            ("login: me", "login: -me", "login"),
            ("login: me", "login: me-", "login"),
            ("login: me", "login: me[bot", "login"),
            ("login: me", "login: '[bot]'", "login"),
            ("login: me", "login: 'a b'", "login"),
            ("me@example.com", "me@exa..mple.com", "email"),
            ("me@example.com", "meexample.com", "email"),
            ("me@example.com", "me@@example.com", "email"),
            ("me@example.com", "'@example.com'", "email"),
            ("me@example.com", "me@example.com-", "email"),
            ("me@example.com", "'a b@example.com'", "email"),
        ] {
            let text = LEDGER.replace(from, to);
            let lines = problems(&text, "me");
            assert_eq!(lines.len(), 1, "{to}: {lines:?}");
            assert!(
                lines[0].contains(&format!("`identities[me].{key}` is invalid: ")),
                "{lines:?}"
            );
        }
        let pem = LEDGER.replace("pem: /home/me/.config/wd/bot.pem", "pem: bot.pem");
        assert!(problems(&pem, "bot")[0].contains("must start with \"/\""));
    }

    #[test]
    fn ids_must_be_integers_of_one_or_more() {
        for (value, why) in [
            ("0", "0 is not 1 or more"),
            ("-3", "-3 is not 1 or more"),
            ("abc", "is not an integer"),
            ("1.5", "is not an integer"),
            ("'0'", "0 is not 1 or more"),
        ] {
            let text = LEDGER.replace("app_id: 123456", &format!("app_id: {value}"));
            let lines = problems(&text, "bot");
            assert_eq!(lines.len(), 1, "{value}: {lines:?}");
            assert!(lines[0].contains(why), "{value}: {lines:?}");
            assert!(lines[0].contains("`identities[bot].app_id`"), "{lines:?}");
            assert!(lines[0].starts_with("line 4, column "), "{lines:?}");
        }
    }

    #[test]
    fn unselected_entries_are_only_checked_for_the_yaml_format_and_scalar_types() {
        // A wrong type, an unknown key, a missing key and bad formats all pass.
        let text = "identities:\n  me:\n    type: user\n    login: me\n    email: me@example.com\n  odd:\n    type: robot\n    junk: 5\n    login: '!!'\n    app_id: -1\n    pem: relative\n";
        assert!(read(text.as_bytes(), &name("me")).is_ok());
        // A value that is neither a string nor an integer stops, with its position.
        let boolean = text.replace("junk: 5", "junk: true");
        assert_eq!(
            yaml_error(&boolean, "me"),
            "line 8, column 11: `identities[odd].junk` must be a string or an integer, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
        );
        let nested = text.replace("junk: 5", "junk: [1]");
        assert!(yaml_error(&nested, "me").contains("it is a sequence"));
    }

    #[test]
    fn an_entry_must_be_a_mapping() {
        assert_eq!(
            yaml_error("identities:\n  me: x\n", "me"),
            "line 2, column 7: `identities[me]` must be a mapping, but it is a string"
        );
        assert!(yaml_error("identities: x\n", "me").contains("`identities` must be a mapping"));
    }

    #[test]
    fn the_yaml_format_applies_to_the_whole_file() {
        assert!(yaml_error("identities: &a {}\n", "me").contains("anchors (&) are not allowed"));
        assert!(yaml_error("identities: {}\n---\nx: 1\n", "me").contains("a second document"));
        assert!(yaml_error("identities:\n  me: {}\n  me: {}\n", "me").contains("duplicate key"));
    }

    #[test]
    fn a_non_string_value_of_the_selected_entry_is_a_problem_with_its_position() {
        let text = LEDGER.replace("login: me", "login: true");
        assert_eq!(
            problems(&text, "me"),
            [
                "line 11, column 12: `identities[me].login` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
            ]
        );
    }

    #[test]
    fn an_invalid_value_is_reported_at_its_position() {
        let text = LEDGER.replace("pem: /home/me/.config/wd/bot.pem", "pem: bot.pem");
        assert_eq!(
            problems(&text, "bot"),
            [
                "line 6, column 10: `identities[bot].pem` is invalid: \"bot.pem\" is not an absolute path; it must start with \"/\""
            ]
        );
    }
}
