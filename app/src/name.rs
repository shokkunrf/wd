//! The canonical form of a name, shared by `name`, `control` and the `name` of a repo.

use crate::output::quote;

const MIN_LEN: usize = 2;
const MAX_LEN: usize = 30;

/// Checks that `text` matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$` and has 2 to 30 characters.
pub fn check(text: &str) -> Result<(), String> {
    if (MIN_LEN..=MAX_LEN).contains(&text.len()) && is_canonical(text) {
        Ok(())
    } else {
        Err(format!(
            "{} is not a canonical name; use {MIN_LEN} to {MAX_LEN} lowercase letters, digits and hyphens, starting with a letter, with no trailing or doubled hyphen",
            quote(text)
        ))
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
            assert!(check(text).is_ok(), "{text:?}");
        }
    }

    #[test]
    fn rejects_everything_else() {
        let long = "a".repeat(31);
        for text in [
            "", "a", "A", "Ab", "1a", "-a", "a-", "a--b", "a_b", "a b", "a.b", "日本", "a\nb",
            &long,
        ] {
            assert!(check(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn the_reason_names_the_value_on_one_line() {
        assert_eq!(
            check("A\nb").unwrap_err(),
            "\"A\\nb\" is not a canonical name; use 2 to 30 lowercase letters, digits and hyphens, starting with a letter, with no trailing or doubled hyphen"
        );
    }
}
