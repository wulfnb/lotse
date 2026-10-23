//! Request-local permission lifecycle. Responders are consumed exactly once.
use std::collections::VecDeque;
pub trait PermissionPort {
    fn respond(self, allow: bool);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestId(u64);
/// Upper bound for every queue of site-initiated dialogs.
pub const MAX_PENDING: usize = 16;
/// One origin cannot occupy more than this many queued dialogs.
pub const MAX_PER_ORIGIN: usize = 3;
/// Whether one more dialog from `origin` may queue behind dialogs from `pending` origins.
/// Callers deny (or drop, which denies) requests that are not admitted.
pub fn admit<'a>(pending: impl IntoIterator<Item = &'a str>, origin: &str) -> bool {
    let (mut total, mut same) = (0, 0);
    for queued in pending {
        total += 1;
        same += usize::from(queued == origin);
    }
    total < MAX_PENDING && same < MAX_PER_ORIGIN
}
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
    /// Queues a request. Duplicates of a pending origin/feature pair and requests beyond
    /// the queue bounds are denied immediately; returns whether the request was queued.
    pub fn push(&mut self, origin: String, feature: String, responder: R) -> bool {
        if self
            .pending
            .iter()
            .any(|(_, o, f, _)| *o == origin && *f == feature)
            || !admit(self.pending.iter().map(|(_, o, _, _)| o.as_str()), &origin)
        {
            responder.respond(false);
            return false;
        }
        let id = RequestId(self.next);
        self.next = self.next.wrapping_add(1);
        self.pending.push_back((id, origin, feature, responder));
        true
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
    /// Resolve queued requests affected by a changed site setting without touching other sites.
    pub fn resolve(&mut self, origin: &str, feature: &str, allow: bool) {
        let mut remaining = VecDeque::new();
        while let Some((id, queued_origin, queued_feature, responder)) = self.pending.pop_front() {
            if queued_origin == origin && queued_feature == feature {
                responder.respond(allow);
            } else {
                remaining.push_back((id, queued_origin, queued_feature, responder));
            }
        }
        self.pending = remaining;
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
