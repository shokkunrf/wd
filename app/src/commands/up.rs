//! `wd up <URL> --identity <key>`: brings a workspace up on the host.

mod precheck;

use std::ffi::OsString;

use crate::control::ControlCopy;
use crate::error::Error;
use crate::location::Runs;
use crate::name::Name;
use crate::repo_url::RepoUrl;
use crate::root;

pub const RUNS: Runs = Runs::Host;

#[derive(Debug)]
struct Args {
    target: String,
    identity: Option<Name>,
}

/// `parser` has consumed the command name.
pub fn run(parser: &mut lexopt::Parser) -> Result<(), Error> {
    let args = parse(parser)?;
    let url = target(&args.target)?;
    let root = root::resolve()?;
    let Some(identity) = args.identity else {
        return Err(Error::IdentityRequired {
            url: url.as_str().to_owned(),
        });
    };
    let auth = precheck::run(&identity, &url)?;
    let copy = ControlCopy::create(&root, url, auth)?;
    copy.fetch()
}

fn parse(parser: &mut lexopt::Parser) -> Result<Args, Error> {
    use lexopt::prelude::*;

    let mut target = None;
    let mut identity = None;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("identity") if identity.is_some() => {
                return Err(Error::Repeated {
                    option: "--identity",
                });
            }
            Long("identity") => {
                let value = utf8(parser.value()?, "the value of --identity")?;
                identity =
                    Some(Name::parse(&value).map_err(|reason| Error::BadIdentity { reason })?);
            }
            Value(value) if target.is_none() => {
                target = Some(utf8(value, "the argument of wd up")?);
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    Ok(Args {
        target: target.ok_or(Error::MissingTarget)?,
        identity,
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
    match Name::parse(arg) {
        Ok(_) => Err(Error::UpByName {
            name: arg.to_owned(),
        }),
        Err(name_reason) => Err(Error::NeitherUrlNorName {
            arg: arg.to_owned(),
            url_reason,
            name_reason,
        }),
    }
}
