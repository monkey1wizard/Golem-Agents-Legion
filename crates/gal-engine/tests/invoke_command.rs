use gal_engine::{classify_args, Action, CommandKind};

#[test]
fn invoke_round_trips_through_the_registry() {
    assert_eq!(CommandKind::parse("invoke"), Some(CommandKind::Invoke));
    assert_eq!(CommandKind::Invoke.as_str(), "invoke");
    assert!(CommandKind::ALL.contains(&CommandKind::Invoke));
}

#[test]
fn invoke_is_registered_exactly_once_and_classified() {
    let count = CommandKind::ALL
        .iter()
        .filter(|kind| **kind == CommandKind::Invoke)
        .count();
    assert_eq!(count, 1);

    assert_eq!(
        classify_args(&["invoke".to_string()]),
        Action::NotWired(CommandKind::Invoke)
    );
}

#[test]
fn unknown_commands_remain_unknown() {
    assert_eq!(
        classify_args(&["frobnicate".to_string()]),
        Action::UnknownCommand("frobnicate".to_string())
    );
}
