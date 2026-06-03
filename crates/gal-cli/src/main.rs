use gal_core::parse_command;

fn main() {
    let _ = parse_command("doctor");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_doctor_mode() {
        assert!(matches!(parse_command("doctor"), gal_core::CommandKind::Doctor));
    }

    #[test]
    fn parses_install_mode() {
        assert!(matches!(parse_command("install"), gal_core::CommandKind::Install));
    }
}
