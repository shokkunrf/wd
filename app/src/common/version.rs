pub const VERSION: &str = match option_env!("WD_VERSION") {
    Some(version) => version,
    None => "0.0.0-dev",
};
