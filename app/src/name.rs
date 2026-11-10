//! The canonical form of a name, shared by `name`, `control`, the `name` of a repo and the keys of
//! the identity ledger.

use crate::output::quote;

const MIN_LEN: usize = 2;
const MAX_LEN: usize = 30;

/// A name that matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` and is 2 to 30 characters long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    /// The error is the reason, which starts with the quoted value.
    pub fn parse(text: &str) -> Result<Name, String> {
        if (MIN_LEN..=MAX_LEN).contains(&text.len()) && is_canonical(text) {
            Ok(Name(text.to_owned()))
        } else {
            Err(format!(
                "{} is not a canonical name; use {MIN_LEN} to {MAX_LEN} lowercase letters, digits and hyphens, starting with a letter, with no trailing or doubled hyphen",
                quote(text)
            ))
        }
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "up and the ledger read the name (#17)")
    )]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_canonical(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !text.ends_with('-')
        && !text.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_canonical_form() {
        for text in ["ab", "a1", "autopilot", "a-b", "a1-b2-c3", &"a".repeat(30)] {
            assert_eq!(Name::parse(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn rejects_everything_else() {
        let long = "a".repeat(31);
        for text in [
            "", "a", "A", "Ab", "1a", "-a", "a-", "a--b", "a_b", "a b", "a.b", "日本", "a\nb",
            &long,
        ] {
            assert!(Name::parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn the_reason_names_the_value_on_one_line() {
        assert_eq!(
            Name::parse("A\nb").unwrap_err(),
            "\"A\\nb\" is not a canonical name; use 2 to 30 lowercase letters, digits and hyphens, starting with a letter, with no trailing or doubled hyphen"
        );
    }
}
