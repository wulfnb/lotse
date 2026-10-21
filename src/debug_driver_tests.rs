use super::*;
#[test]
fn disconnected_reply_is_not_an_infinite_pending_request() {
    let (sender, receiver) = channel::<String>().unwrap();
    assert!(poll(&receiver).is_none());
    drop(sender);
    assert!(poll(&receiver).unwrap().is_err());
}
