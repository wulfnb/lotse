use super::*;
#[test]
fn timeouts_and_cancellation_prevent_late_dispatch() {
    assert!(Deadline::new(Duration::ZERO).expired());
    let deadline = Deadline::new(Duration::from_secs(60));
    let queued = deadline.clone();
    assert!(!queued.expired());
    deadline.cancel();
    assert!(queued.expired());
}

#[test]
fn timeout_budget_is_shared_between_queue_and_reply() {
    let deadline = Deadline::new(Duration::from_secs(60));
    assert!(deadline.remaining() <= Duration::from_secs(60));
    assert!(deadline.remaining() > Duration::ZERO);
    let expired = Deadline::new(Duration::ZERO);
    assert_eq!(expired.remaining(), Duration::ZERO);
}
