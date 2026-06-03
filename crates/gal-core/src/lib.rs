#[derive(Debug, PartialEq, Eq)]
pub enum CommandKind {
    Install,
    Update,
    Doctor,
    Unknown,
}

pub fn parse_command(input: &str) -> CommandKind {
    match input.trim().to_ascii_lowercase().as_str() {
        "install" => CommandKind::Install,
        "update" => CommandKind::Update,
        "doctor" => CommandKind::Doctor,
        _ => CommandKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_commands() {
        assert_eq!(parse_command("install"), CommandKind::Install);
        assert_eq!(parse_command("update"), CommandKind::Update);
        assert_eq!(parse_command("doctor"), CommandKind::Doctor);
    }
}
