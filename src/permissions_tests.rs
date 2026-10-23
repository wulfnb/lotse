use super::*;
use std::{cell::RefCell, rc::Rc};
struct Responder(usize, Rc<RefCell<Vec<(usize, bool)>>>);
impl PermissionPort for Responder {
    fn respond(self, allow: bool) {
        self.1.borrow_mut().push((self.0, allow));
    }
}
#[test]
fn permissions_are_request_local_fifo_and_consumed_once() {
    let answers = Rc::new(RefCell::new(Vec::new()));
    let mut requests = Permissions::default();
    assert!(requests.is_empty());
    assert!(requests.front().is_none());
    assert!(!requests.answer(RequestId(0), true));
    requests.push(
        "https://a.test".into(),
        "camera".into(),
        Responder(1, answers.clone()),
    );
    requests.push(
        "https://b.test".into(),
        "location".into(),
        Responder(2, answers.clone()),
    );
    assert_eq!(
        requests.front(),
        Some((RequestId(0), "https://a.test", "camera"))
    );
    assert!(requests.answer(RequestId(0), true));
    assert_eq!(
        requests.front(),
        Some((RequestId(1), "https://b.test", "location"))
    );
    assert!(
        !requests.answer(RequestId(0), true),
        "Stale allow must not grant the next site"
    );
    requests.clear();
    assert!(requests.is_empty());
    assert!(!requests.answer(RequestId(0), false));
    assert_eq!(*answers.borrow(), [(1, true), (2, false)]);
    requests.push(
        "https://a.test".into(),
        "camera".into(),
        Responder(3, answers.clone()),
    );
    drop(requests);
    assert_eq!(*answers.borrow(), [(1, true), (2, false), (3, false)]);
}

#[test]
fn duplicate_and_overflowing_requests_are_denied_immediately() {
    let answers = Rc::new(RefCell::new(Vec::new()));
    let mut requests = Permissions::default();
    assert!(requests.push(
        "https://a.test".into(),
        "camera".into(),
        Responder(0, answers.clone())
    ));
    assert!(
        !requests.push(
            "https://a.test".into(),
            "camera".into(),
            Responder(1, answers.clone())
        ),
        "A pending origin/feature pair is not queued twice"
    );
    for (index, feature) in ["microphone", "location"].into_iter().enumerate() {
        assert!(requests.push(
            "https://a.test".into(),
            feature.into(),
            Responder(index + 2, answers.clone())
        ));
    }
    assert!(
        !requests.push(
            "https://a.test".into(),
            "notifications".into(),
            Responder(4, answers.clone())
        ),
        "One origin cannot flood the queue"
    );
    assert_eq!(*answers.borrow(), [(1, false), (4, false)]);
    for index in 0..MAX_PENDING {
        requests.push(
            format!("https://{index}.test"),
            "camera".into(),
            Responder(100 + index, answers.clone()),
        );
    }
    let mut queued = 0;
    while let Some((id, ..)) = requests.front() {
        requests.answer(id, true);
        queued += 1;
    }
    assert_eq!(queued, MAX_PENDING, "The global queue is bounded");
}

#[test]
fn admission_bounds_total_and_per_origin_queues() {
    assert!(admit([], "a"));
    assert!(admit(["a", "a"], "a"));
    assert!(!admit(["a"; MAX_PER_ORIGIN], "a"));
    assert!(admit(["a"; MAX_PER_ORIGIN], "b"));
    let full: Vec<String> = (0..MAX_PENDING).map(|i| i.to_string()).collect();
    assert!(!admit(full.iter().map(String::as_str), "new"));
}
