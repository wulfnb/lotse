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
