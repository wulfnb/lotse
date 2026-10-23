//! The only application permission adapter that owns Servo responders.
pub(crate) struct Permission(pub servo::PermissionRequest);
impl wega::permissions::PermissionPort for Permission {
    fn respond(self, allow: bool) {
        if allow {
            self.0.allow();
        } else {
            self.0.deny();
        }
    }
}
