//! The YAML reader shared by workspace.yaml, the identity ledger and the other files wd reads.
//!
//! One YAML 1.2 document is read into a tree that owns its data. The reader rejects what the
//! specification does not accept (anchors, aliases, merge keys, tags outside the core schema,
//! duplicate keys, a second document and the size and depth limits). Mapping a tree onto the keys
//! and value formats of a file is the job of the caller.

use std::collections::HashSet;
use std::fmt;

use saphyr_parser::{Event, Parser, ScalarStyle, Span, StrInput, Tag};

use crate::output::escape;

/// The size limit of a file in bytes.
pub const MAX_BYTES: usize = 1 << 20;
/// The limit of nested mappings and sequences. The top-level mapping is level 1.
const MAX_DEPTH: usize = 8;
/// The message saphyr-parser gives for an alias, whose anchor can never be defined here.
const UNKNOWN_ANCHOR: &str = "while parsing node, found unknown anchor";

/// A position in the file. Both numbers count from 1, the column in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pos: Option<Pos>,
    reason: String,
}

impl Error {
    pub fn new(pos: Pos, reason: impl Into<String>) -> Self {
        Error {
            pos: Some(pos),
            reason: reason.into(),
        }
    }

    /// A file over the size limit has no position, so the error shows its size instead.
    pub fn too_large(size: u64) -> Self {
        Error {
            pos: None,
            reason: format!(
                "the file is {size} bytes, over the limit of {MAX_BYTES} bytes (1 MiB)"
            ),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pos {
            Some(pos) => write!(
                f,
                "line {}, column {}: {}",
                pos.line, pos.column, self.reason
            ),
            None => write!(f, "{}", self.reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarType {
    Str,
    Null,
    Bool,
    Int,
    Float,
}

impl ScalarType {
    fn name(self) -> &'static str {
        match self {
            ScalarType::Str => "a string",
            ScalarType::Null => "null",
            ScalarType::Bool => "a boolean",
            ScalarType::Int => "an integer",
            ScalarType::Float => "a float",
        }
    }
}

#[derive(Debug)]
struct Scalar {
    value: String,
    ty: ScalarType,
    plain: bool,
    tagged: bool,
}

impl Scalar {
    /// The reason is the rest of a sentence that starts with "must be a string, but".
    fn string(&self) -> Result<&str, String> {
        if self.ty == ScalarType::Str {
            return Ok(&self.value);
        }
        if self.tagged {
            return Err(format!("its tag makes it {}", self.ty.name()));
        }
        if self.value.is_empty() {
            return Err("it has no value".to_owned());
        }
        Err(format!(
            "the plain value \"{}\" is read as {}; write it in quotes to use it as a string",
            escape(&self.value),
            self.ty.name()
        ))
    }
}

#[derive(Debug)]
pub struct Entry {
    pub key: String,
    pub value: Node,
}

#[derive(Debug)]
enum Kind {
    Scalar(Scalar),
    Map(Vec<Entry>),
    Seq(Vec<Node>),
}

#[derive(Debug)]
pub struct Node {
    pos: Pos,
    kind: Kind,
}

impl Node {
    fn describe(&self) -> String {
        match &self.kind {
            Kind::Scalar(scalar) => scalar.ty.name().to_owned(),
            Kind::Map(_) => "a mapping".to_owned(),
            Kind::Seq(_) => "a sequence".to_owned(),
        }
    }

    /// `label` names the value in the message, for example "`name`".
    pub fn as_str(&self, label: &str) -> Result<&str, Error> {
        let reason = match &self.kind {
            Kind::Scalar(scalar) => match scalar.string() {
                Ok(value) => return Ok(value),
                Err(reason) => reason,
            },
            _ => format!("it is {}", self.describe()),
        };
        Err(Error::new(
            self.pos,
            format!("{label} must be a string, but {reason}"),
        ))
    }

    pub fn as_map(&self, label: &str) -> Result<&[Entry], Error> {
        match &self.kind {
            Kind::Map(entries) => Ok(entries),
            _ => Err(Error::new(
                self.pos,
                format!("{label} must be a mapping, but it is {}", self.describe()),
            )),
        }
    }

    pub fn as_seq(&self, label: &str) -> Result<&[Node], Error> {
        match &self.kind {
            Kind::Seq(items) => Ok(items),
            _ => Err(Error::new(
                self.pos,
                format!("{label} must be a sequence, but it is {}", self.describe()),
            )),
        }
    }
}

/// Reads one YAML document. A file without a document is an empty mapping.
pub fn parse(bytes: &[u8]) -> Result<Node, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::too_large(bytes.len() as u64));
    }
    let text = std::str::from_utf8(bytes).map_err(|err| utf8_error(bytes, err.valid_up_to()))?;
    let mut parser = Parser::new_from_str(text);
    let mut root = None;
    loop {
        let (event, span) = next(&mut parser)?;
        match event {
            Event::StreamStart | Event::DocumentEnd => {}
            Event::StreamEnd => break,
            Event::DocumentStart(_) => {
                if root.is_some() {
                    return Err(Error::new(pos(&span), "a second document is not allowed"));
                }
                let (event, span) = next(&mut parser)?;
                root = Some(node(&mut parser, event, &span, 0)?);
            }
            _ => return Err(unexpected(&span)),
        }
    }
    Ok(root.unwrap_or(Node {
        pos: Pos { line: 1, column: 1 },
        kind: Kind::Map(Vec::new()),
    }))
}

fn utf8_error(bytes: &[u8], valid_up_to: usize) -> Error {
    let prefix = &bytes[..valid_up_to];
    let line = 1 + prefix.iter().filter(|byte| **byte == b'\n').count();
    let start = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let column = 1 + String::from_utf8_lossy(&prefix[start..]).chars().count();
    Error::new(Pos { line, column }, "the file is not valid UTF-8")
}

fn pos(span: &Span) -> Pos {
    Pos {
        line: span.start.line(),
        column: span.start.col() + 1,
    }
}

fn unexpected(span: &Span) -> Error {
    Error::new(pos(span), "unexpected YAML structure")
}

fn next<'a>(parser: &mut Parser<'a, StrInput<'a>>) -> Result<(Event<'a>, Span), Error> {
    match parser.next() {
        Some(Ok(pair)) => Ok(pair),
        Some(Err(err)) => {
            let at = Pos {
                line: err.marker().line(),
                column: err.marker().col() + 1,
            };
            // Anchors are rejected before an alias can reach us, so an alias arrives as an
            // error from saphyr-parser, which reports it as an unknown anchor. Match its
            // message (saphyr-parser 0.3.3, parser.rs) to give the actual reason.
            if err.info() == UNKNOWN_ANCHOR {
                Err(alias_error(at))
            } else {
                Err(Error::new(at, err.info()))
            }
        }
        None => Err(Error {
            pos: None,
            reason: "the document ends unexpectedly".to_owned(),
        }),
    }
}

fn node<'a>(
    parser: &mut Parser<'a, StrInput<'a>>,
    event: Event<'a>,
    span: &Span,
    depth: usize,
) -> Result<Node, Error> {
    let at = pos(span);
    match event {
        Event::Scalar(value, style, anchor, tag) => {
            no_anchor(anchor, at)?;
            let scalar = scalar(&value, style, tag.as_deref(), at)?;
            Ok(Node {
                pos: at,
                kind: Kind::Scalar(scalar),
            })
        }
        Event::Alias(_) => Err(alias_error(at)),
        Event::SequenceStart(anchor, tag) => {
            no_anchor(anchor, at)?;
            collection_tag(tag.as_deref(), "seq", "a sequence", at)?;
            enter(depth, at)?;
            let mut items = Vec::new();
            loop {
                let (event, span) = next(parser)?;
                if matches!(event, Event::SequenceEnd) {
                    break;
                }
                items.push(node(parser, event, &span, depth + 1)?);
            }
            Ok(Node {
                pos: at,
                kind: Kind::Seq(items),
            })
        }
        Event::MappingStart(anchor, tag) => {
            no_anchor(anchor, at)?;
            collection_tag(tag.as_deref(), "map", "a mapping", at)?;
            enter(depth, at)?;
            let mut entries = Vec::new();
            let mut seen = HashSet::new();
            loop {
                let (event, span) = next(parser)?;
                if matches!(event, Event::MappingEnd) {
                    break;
                }
                let key_pos = pos(&span);
                let key = key(event, key_pos)?;
                if !seen.insert(key.clone()) {
                    return Err(Error::new(
                        key_pos,
                        format!("duplicate key \"{}\"", escape(&key)),
                    ));
                }
                let (event, span) = next(parser)?;
                let value = node(parser, event, &span, depth + 1)?;
                entries.push(Entry { key, value });
            }
            Ok(Node {
                pos: at,
                kind: Kind::Map(entries),
            })
        }
        _ => Err(unexpected(span)),
    }
}

fn key(event: Event<'_>, at: Pos) -> Result<String, Error> {
    match event {
        Event::Scalar(value, style, anchor, tag) => {
            no_anchor(anchor, at)?;
            let scalar = scalar(&value, style, tag.as_deref(), at)?;
            if scalar.plain && !scalar.tagged && scalar.value == "<<" {
                return Err(Error::new(
                    at,
                    "the merge key << is not allowed; write \"<<\" in quotes to use it as a name",
                ));
            }
            match scalar.string() {
                Ok(key) => Ok(key.to_owned()),
                Err(reason) => Err(Error::new(
                    at,
                    format!("a mapping key must be a string, but {reason}"),
                )),
            }
        }
        Event::Alias(_) => Err(alias_error(at)),
        _ => Err(Error::new(at, "a mapping key must be a string")),
    }
}

fn alias_error(at: Pos) -> Error {
    Error::new(at, "aliases (*) are not allowed")
}

fn no_anchor(anchor: usize, at: Pos) -> Result<(), Error> {
    if anchor == 0 {
        Ok(())
    } else {
        Err(Error::new(at, "anchors (&) are not allowed"))
    }
}

fn enter(depth: usize, at: Pos) -> Result<(), Error> {
    if depth + 1 > MAX_DEPTH {
        Err(Error::new(
            at,
            format!("mappings and sequences are nested deeper than {MAX_DEPTH} levels"),
        ))
    } else {
        Ok(())
    }
}

fn tag_name(tag: &Tag) -> String {
    if tag.is_yaml_core_schema() {
        format!("!!{}", tag.suffix)
    } else if tag.handle == "!" {
        format!("!{}", tag.suffix)
    } else {
        format!("{}{}", tag.handle, tag.suffix)
    }
}

/// The name of a core schema tag, such as "str" for `!!str`.
fn core_tag(tag: &Tag, at: Pos) -> Result<&str, Error> {
    if tag.is_yaml_core_schema()
        && matches!(
            tag.suffix.as_str(),
            "str" | "int" | "float" | "bool" | "null" | "map" | "seq"
        )
    {
        Ok(&tag.suffix)
    } else {
        Err(Error::new(
            at,
            format!(
                "the tag {} is not in the YAML 1.2 core schema",
                escape(&tag_name(tag))
            ),
        ))
    }
}

fn collection_tag(tag: Option<&Tag>, expected: &str, what: &str, at: Pos) -> Result<(), Error> {
    let Some(tag) = tag else { return Ok(()) };
    if core_tag(tag, at)? == expected {
        Ok(())
    } else {
        Err(Error::new(
            at,
            format!("the tag {} cannot be put on {what}", tag_name(tag)),
        ))
    }
}

fn scalar(value: &str, style: ScalarStyle, tag: Option<&Tag>, at: Pos) -> Result<Scalar, Error> {
    let plain = style == ScalarStyle::Plain;
    let ty = match tag {
        None if plain => resolve(value),
        None => ScalarType::Str,
        Some(tag) => {
            let (ty, fits) = match core_tag(tag, at)? {
                "str" => (ScalarType::Str, true),
                "int" => (ScalarType::Int, is_int(value)),
                "float" => (ScalarType::Float, is_float(value) || is_int(value)),
                "bool" => (ScalarType::Bool, is_bool(value)),
                "null" => (ScalarType::Null, is_null(value)),
                _ => {
                    return Err(Error::new(
                        at,
                        format!("the tag {} cannot be put on a scalar", tag_name(tag)),
                    ));
                }
            };
            if !fits {
                return Err(Error::new(
                    at,
                    format!(
                        "the value \"{}\" cannot be read as {}",
                        escape(value),
                        tag_name(tag)
                    ),
                ));
            }
            ty
        }
    };
    Ok(Scalar {
        value: value.to_owned(),
        ty,
        plain,
        tagged: tag.is_some(),
    })
}

/// Resolves a plain scalar by the core schema of YAML 1.2.
fn resolve(value: &str) -> ScalarType {
    if is_null(value) {
        ScalarType::Null
    } else if is_bool(value) {
        ScalarType::Bool
    } else if is_int(value) {
        ScalarType::Int
    } else if is_float(value) {
        ScalarType::Float
    } else {
        ScalarType::Str
    }
}

fn is_null(value: &str) -> bool {
    matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

fn is_bool(value: &str) -> bool {
    matches!(
        value,
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
    )
}

fn all_digits(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii_digit())
}

fn is_int(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix("0x") {
        return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit());
    }
    if let Some(rest) = value.strip_prefix("0o") {
        return !rest.is_empty() && rest.chars().all(|c| ('0'..='7').contains(&c));
    }
    let rest = value.strip_prefix(['+', '-']).unwrap_or(value);
    !rest.is_empty() && all_digits(rest)
}

fn is_float(value: &str) -> bool {
    if matches!(value, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    let rest = value.strip_prefix(['+', '-']).unwrap_or(value);
    if matches!(rest, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(index) => (&rest[..index], Some(&rest[index + 1..])),
        None => (rest, None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if digits.is_empty() || !all_digits(digits) {
            return false;
        }
    }
    match mantissa.split_once('.') {
        None => !mantissa.is_empty() && all_digits(mantissa),
        Some(("", frac)) => !frac.is_empty() && all_digits(frac),
        Some((int, frac)) => all_digits(int) && all_digits(frac),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(text: &str) -> String {
        parse(text.as_bytes()).unwrap_err().to_string()
    }

    fn string_err(text: &str) -> String {
        let root = parse(text.as_bytes()).unwrap();
        root.as_map("the document").unwrap()[0]
            .value
            .as_str("`a`")
            .unwrap_err()
            .to_string()
    }

    fn string_of(text: &str) -> String {
        let root = parse(text.as_bytes()).unwrap();
        root.as_map("the document").unwrap()[0]
            .value
            .as_str("`a`")
            .unwrap()
            .to_owned()
    }

    #[test]
    fn a_file_without_a_document_is_an_empty_mapping() {
        for text in ["", "\n", "# only a comment\n\n", "\u{feff}# c\n"] {
            let root = parse(text.as_bytes()).unwrap();
            assert!(root.as_map("the document").unwrap().is_empty(), "{text:?}");
        }
    }

    #[test]
    fn accepts_quotes_flow_style_block_scalars_comments_and_crlf() {
        let texts = [
            "a: x\n",
            "a: 'x'\n",
            "a: \"x\"\n",
            "{a: x}\n",
            "a: x # comment\r\n",
            "a: |\r\n  x\r\n",
            "\u{feff}a: x\n",
            "a: !!str x\n",
        ];
        for text in texts {
            let root = parse(text.as_bytes()).unwrap();
            let entries = root.as_map("the document").unwrap();
            assert_eq!(entries[0].key, "a", "{text:?}");
            assert!(entries[0].value.as_str("`a`").unwrap().starts_with('x'));
        }
    }

    #[test]
    fn plain_strings_that_look_like_other_words_are_strings() {
        for value in [
            "yes",
            "on",
            "2026-09-30T12:34:56Z",
            "1.2.3",
            "0b1",
            "1_000",
            "-x",
            "e3",
        ] {
            assert_eq!(string_of(&format!("a: {value}\n")), value);
        }
    }

    #[test]
    fn quoted_and_str_tagged_values_are_strings() {
        assert_eq!(string_of("a: \"true\"\n"), "true");
        assert_eq!(string_of("a: '010'\n"), "010");
        assert_eq!(string_of("a: !!str true\n"), "true");
        assert_eq!(string_of("a: !!str 010\n"), "010");
        assert_eq!(string_of("a: !!str\n"), "");
    }

    #[test]
    fn plain_non_strings_ask_for_quotes() {
        assert_eq!(
            string_err("a: true\n"),
            "line 1, column 4: `a` must be a string, but the plain value \"true\" is read as a boolean; write it in quotes to use it as a string"
        );
        assert!(string_err("a: 010\n").contains("\"010\" is read as an integer"));
        for (value, what) in [
            ("False", "a boolean"),
            ("null", "null"),
            ("~", "null"),
            ("1e3", "a float"),
            ("0x1F", "an integer"),
            (".inf", "a float"),
            ("+5", "an integer"),
            ("0o17", "an integer"),
            ("-.5", "a float"),
        ] {
            let text = format!("a: {value}\n");
            assert!(
                string_err(&text).contains(&format!("\"{value}\" is read as {what}")),
                "{value}"
            );
        }
    }

    #[test]
    fn a_key_without_a_value_is_an_error() {
        assert_eq!(
            string_err("a:\n"),
            "line 1, column 2: `a` must be a string, but it has no value"
        );
    }

    #[test]
    fn collections_and_tagged_non_strings_are_not_strings() {
        assert!(string_err("a: {b: c}\n").contains("but it is a mapping"));
        assert!(string_err("a: [b]\n").contains("but it is a sequence"));
        assert!(string_err("a: !!int 5\n").contains("its tag makes it an integer"));
    }

    #[test]
    fn invalid_utf8_is_reported_at_the_first_bad_byte() {
        let err = parse(b"a: x\nb: \x82\xa0\n").unwrap_err().to_string();
        assert_eq!(err, "line 2, column 4: the file is not valid UTF-8");
        let err = parse(b"\xff\xfea\0").unwrap_err().to_string();
        assert_eq!(err, "line 1, column 1: the file is not valid UTF-8");
    }

    #[test]
    fn a_file_over_1_mib_is_reported_by_size() {
        let text = format!("a: {}\n", "x".repeat(MAX_BYTES));
        assert_eq!(
            parse(text.as_bytes()).unwrap_err().to_string(),
            format!(
                "the file is {} bytes, over the limit of 1048576 bytes (1 MiB)",
                text.len()
            )
        );
        let text = format!("a: {}\n", "x".repeat(MAX_BYTES - 10));
        assert!(parse(text.as_bytes()).is_ok());
    }

    #[test]
    fn scan_errors_show_line_and_column() {
        assert!(err("a: 'x\n").starts_with("line "));
        assert!(err("a:\n  b: 1\n c: 2\n").starts_with("line 3, column "));
        assert!(err("a:\n\tb: 1\n").starts_with("line 2, column "));
    }

    #[test]
    fn a_second_document_is_an_error() {
        assert_eq!(
            err("a: x\n---\nb: y\n"),
            "line 2, column 1: a second document is not allowed"
        );
        assert!(err("a: x\n...\n---\nb: y\n").contains("a second document"));
    }

    #[test]
    fn anchors_and_aliases_are_errors() {
        assert_eq!(
            err("a: &x v\nb: *x\n"),
            "line 1, column 4: anchors (&) are not allowed"
        );
        assert!(err("a: &x [1]\n").contains("anchors (&)"));
        assert!(err("a: &x {b: 1}\n").contains("anchors (&)"));
        assert!(err("&x a: v\n").contains("anchors (&)"));
        assert_eq!(
            err("a: *x\n"),
            "line 1, column 4: aliases (*) are not allowed"
        );
        assert_eq!(
            err("*x : 1\n"),
            "line 1, column 1: aliases (*) are not allowed"
        );
    }

    #[test]
    fn tags_outside_the_core_schema_are_errors() {
        assert_eq!(
            err("a: !x v\n"),
            "line 1, column 4: the tag !x is not in the YAML 1.2 core schema"
        );
        assert_eq!(
            err("a: !!binary v\n"),
            "line 1, column 4: the tag !!binary is not in the YAML 1.2 core schema"
        );
        assert!(err("a: !!set {b}\n").contains("!!set is not in the YAML 1.2 core schema"));
        assert!(err("!x a: v\n").contains("!x is not in"));
    }

    #[test]
    fn core_schema_tags_must_fit_the_value() {
        assert!(parse(b"a: !!int 5\nb: !!float 1.5\nc: !!bool true\nd: !!null ~\n").is_ok());
        assert!(parse(b"a: !!int \"5\"\ne: !!float 1\n").is_ok());
        assert!(parse(b"a: !!map {b: c}\nb: !!seq [c]\n").is_ok());
        assert_eq!(
            err("a: !!int x\n"),
            "line 1, column 4: the value \"x\" cannot be read as !!int"
        );
        assert!(err("a: !!bool yes\n").contains("cannot be read as !!bool"));
        assert!(err("a: !!map x\n").contains("cannot be put on a scalar"));
        assert!(err("a: !!str {b: c}\n").contains("!!str cannot be put on a mapping"));
        assert!(err("a: !!map [b]\n").contains("!!map cannot be put on a sequence"));
    }

    #[test]
    fn the_merge_key_is_an_error_unless_quoted() {
        assert!(err("<<: x\n").starts_with("line 1, column 1: the merge key <<"));
        assert!(err("a: {<<: x}\n").contains("merge key"));
        let root = parse(b"\"<<\": x\n").unwrap();
        assert_eq!(root.as_map("d").unwrap()[0].key, "<<");
    }

    #[test]
    fn duplicate_keys_are_errors_even_when_written_differently() {
        assert_eq!(err("a: 1\na: 2\n"), "line 2, column 1: duplicate key \"a\"");
        assert!(err("a: 1\n\"a\": 2\n").starts_with("line 2, column 1: duplicate key"));
        assert!(err("{a: 1, 'a': 2}\n").contains("duplicate key"));
        assert!(parse(b"a: {b: 1}\nb: {b: 2}\n").is_ok());
    }

    #[test]
    fn mapping_keys_must_be_strings() {
        assert!(
            err("true: x\n")
                .contains("a mapping key must be a string, but the plain value \"true\"")
        );
        assert!(err("1: x\n").contains("read as an integer"));
        assert!(err("? [a]\n: x\n").contains("a mapping key must be a string"));
        assert!(err("? {a: b}\n: x\n").contains("a mapping key must be a string"));
        assert!(parse(b"\"true\": x\n\"null\": y\n").is_ok());
    }

    #[test]
    fn nesting_is_limited_to_8_levels() {
        let nested = |levels: usize| {
            let mut text = String::new();
            for level in 0..levels - 1 {
                text.push_str(&" ".repeat(level));
                text.push_str("a:\n");
            }
            text.push_str(&" ".repeat(levels - 1));
            text.push_str("a: x\n");
            text
        };
        assert!(parse(nested(8).as_bytes()).is_ok());
        assert_eq!(
            err(&nested(9)),
            "line 9, column 9: mappings and sequences are nested deeper than 8 levels"
        );
        assert!(parse(b"a: [[[[[[[x]]]]]]]\n").is_ok());
        assert!(err("a: [[[[[[[[x]]]]]]]]\n").contains("nested deeper than 8 levels"));
        assert!(err(&format!("{}x{}", "[".repeat(9), "]".repeat(9))).contains("deeper"));
    }

    #[test]
    fn typed_access_names_the_wrong_shape() {
        let root = parse(b"a: x\n").unwrap();
        assert_eq!(
            root.as_seq("the document").unwrap_err().to_string(),
            "line 1, column 1: the document must be a sequence, but it is a mapping"
        );
        let root = parse(b"- a\n").unwrap();
        assert_eq!(
            root.as_map("the document").unwrap_err().to_string(),
            "line 1, column 1: the document must be a mapping, but it is a sequence"
        );
    }
}
