//! `wd up <URL> --identity <key>`: brings a workspace up on the host.

use std::ffi::OsString;

use crate::control::ControlCopy;
use crate::error::Error;
use crate::location::Runs;
use crate::name;
use crate::repo_url::RepoUrl;
use crate::root;

pub const RUNS: Runs = Runs::Host;

#[derive(Debug)]
struct Args {
    target: String,
    /// The value itself is checked but not used yet.
    has_identity: bool,
}

/// `parser` has consumed the command name.
pub fn run(parser: &mut lexopt::Parser) -> Result<(), Error> {
    let args = parse(parser)?;
    let url = target(&args.target)?;
    let root = root::resolve()?;
    if !args.has_identity {
        return Err(Error::IdentityRequired {
            url: url.as_str().to_owned(),
        });
    }
    let copy = ControlCopy::create(&root, url)?;
    copy.fetch()
}

fn parse(parser: &mut lexopt::Parser) -> Result<Args, Error> {
    use lexopt::prelude::*;

    let mut target = None;
    let mut has_identity = false;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("identity") if has_identity => {
                return Err(Error::Repeated {
                    option: "--identity",
                });
            }
            Long("identity") => {
                let value = utf8(parser.value()?, "the value of --identity")?;
                name::check(&value).map_err(|reason| Error::BadIdentity { reason })?;
                has_identity = true;
            }
            Value(value) if target.is_none() => {
                target = Some(utf8(value, "the argument of wd up")?);
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    Ok(Args {
        target: target.ok_or(Error::MissingTarget)?,
        has_identity,
    })
}

fn utf8(value: OsString, what: &'static str) -> Result<String, Error> {
    value
        .into_string()
        .map_err(|value| Error::NotUtf8 { what, value })
}

/// The control's URL; a workspace name is recognized only to say that it is not supported yet.
fn target(arg: &str) -> Result<RepoUrl, Error> {
    let url_reason = match RepoUrl::parse(arg) {
        Ok(url) => return Ok(url),
        Err(reason) => reason,
    };
    match name::check(arg) {
        Ok(()) => Err(Error::UpByName {
            name: arg.to_owned(),
        }),
        Err(name_reason) => Err(Error::NeitherUrlNorName {
            arg: arg.to_owned(),
            url_reason,
            name_reason,
        }),
    }
}
