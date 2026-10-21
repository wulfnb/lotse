use super::*;
#[test]
fn closing_keeps_selected_identity_and_handles_last_tab() {
    let mut session = Session::default();
    for tab in ['a', 'b', 'c'] {
        session.push(tab);
    }
    session.apply(Command::Select(1));
    assert_eq!(session.apply(Command::Close(0)), Event::Closed('a'));
    assert_eq!(session.tabs()[session.active()], 'b');
    session.apply(Command::Close(0));
    assert_eq!(session.tabs()[session.active()], 'c');
    session.apply(Command::Close(0));
    assert!(session.tabs().is_empty());
    assert_eq!(session.active(), 0);
    assert_eq!(session.apply(Command::Select(1)), Event::Unchanged);
}
#[test]
fn reordering_and_invalid_commands_preserve_selection() {
    for active in 0..4 {
        for from in 0..4 {
            for to in 0..4 {
                let mut session = Session::default();
                for tab in 0..4 {
                    session.push(tab);
                }
                session.apply(Command::Select(active));
                session.apply(Command::Move { from, to });
                assert_eq!(session.tabs()[session.active()], active);
                assert_eq!(session.apply(Command::Close(99)), Event::Unchanged);
                assert_eq!(
                    session.apply(Command::Move { from: 99, to: 0 }),
                    Event::Unchanged
                );
            }
        }
    }
}

#[test]
fn updating_tab_content_and_clearing_session_do_not_leave_stale_selection() {
    let mut session = Session::default();
    session.push(String::from("old"));
    session.push(String::from("other"));
    session.apply(Command::Select(1));
    session.tabs_mut()[1] = String::from("updated");
    assert_eq!(session.tabs()[session.active()], "updated");
    session.clear();
    assert!(session.tabs().is_empty());
    assert_eq!(session.active(), 0);
    session.push(String::from("new"));
    assert_eq!(session.tabs()[session.active()], "new");
}

#[test]
fn generic_session_transitions_obey_the_same_contract_for_each_payload_type() {
    fn exercise<T: Clone + PartialEq + std::fmt::Debug>(values: [T; 3]) {
        let mut session = Session::default();
        for value in &values {
            session.push(value.clone());
        }
        assert_eq!(session.apply(Command::Select(1)), Event::Selected);
        assert_eq!(
            session.apply(Command::Close(0)),
            Event::Closed(values[0].clone())
        );
        assert_eq!(session.tabs()[session.active()], values[1]);
        assert_eq!(
            session.apply(Command::Move { from: 0, to: 1 }),
            Event::Reordered
        );
        assert_eq!(session.tabs()[session.active()], values[1]);
        session.tabs_mut()[1] = values[0].clone();
        assert_eq!(session.apply(Command::Close(100)), Event::Unchanged);
        assert_eq!(session.apply(Command::Select(100)), Event::Unchanged);
        assert_eq!(
            session.apply(Command::Move { from: 100, to: 0 }),
            Event::Unchanged
        );
        session.clear();
        assert!(session.tabs().is_empty());
        assert_eq!(session.active(), 0);
    }
    exercise([0usize, 1, 2]);
    exercise(['a', 'b', 'c']);
    exercise([
        String::from("one"),
        String::from("two"),
        String::from("three"),
    ]);
}
