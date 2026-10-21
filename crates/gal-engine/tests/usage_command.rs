use gal_engine::{classify_args, Action, CommandKind, ExitCode};

#[test]
fn usage_round_trips_through_the_registry() {
    assert_eq!(CommandKind::parse("usage"), Some(CommandKind::Usage));
    assert_eq!(CommandKind::parse(" USAGE "), Some(CommandKind::Usage));
    assert_eq!(CommandKind::Usage.as_str(), "usage");
}

#[test]
fn usage_is_registered_exactly_once() {
    let count = CommandKind::ALL
        .iter()
        .filter(|kind| **kind == CommandKind::Usage)
        .count();
    assert_eq!(count, 1);
    assert_eq!(CommandKind::ALL.len(), 34);
}

#[test]
fn classify_routes_usage_to_the_registered_command() {
    assert_eq!(
        classify_args(&["usage".to_string()]),
        Action::NotWired(CommandKind::Usage)
    );
    assert_eq!(
        classify_args(&[
            "usage".to_string(),
            "report".to_string(),
            "--run".to_string(),
            "run-a".to_string()
        ]),
        Action::NotWired(CommandKind::Usage)
    );
}

#[test]
fn unknown_commands_still_classify_as_usage_error() {
    assert_eq!(
        classify_args(&["usag".to_string()]),
        Action::UnknownCommand("usag".to_string())
    );
    assert_eq!(
        classify_args(&["frobnicate".to_string()]),
        Action::UnknownCommand("frobnicate".to_string())
    );
    assert_eq!(ExitCode::Usage as u8, 64);
}
