//! Request-local permission lifecycle. Responders are consumed exactly once.
use std::collections::VecDeque;
pub trait PermissionPort {
    fn respond(self, allow: bool);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(u64);
pub struct Permissions<R: PermissionPort> {
    pending: VecDeque<(RequestId, String, String, R)>,
    next: u64,
}
impl<R: PermissionPort> Default for Permissions<R> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            next: 0,
        }
    }
}
impl<R: PermissionPort> Permissions<R> {
    pub fn push(&mut self, origin: String, feature: String, responder: R) {
        let id = RequestId(self.next);
        self.next = self.next.wrapping_add(1);
        self.pending.push_back((id, origin, feature, responder));
    }
    pub fn front(&self) -> Option<(RequestId, &str, &str)> {
        self.pending
            .front()
            .map(|(id, origin, feature, _)| (*id, origin.as_str(), feature.as_str()))
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    /// A stale UI action must not answer the next site's request.
    pub fn answer(&mut self, id: RequestId, allow: bool) -> bool {
        if self
            .pending
            .front()
            .is_some_and(|(current, _, _, _)| *current == id)
        {
            let (_, _, _, responder) = self.pending.pop_front().unwrap();
            responder.respond(allow);
            true
        } else {
            false
        }
    }
    pub fn clear(&mut self) {
        while let Some((_, _, _, responder)) = self.pending.pop_front() {
            responder.respond(false);
        }
    }
}
impl<R: PermissionPort> Drop for Permissions<R> {
    fn drop(&mut self) {
        self.clear();
    }
}
#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
