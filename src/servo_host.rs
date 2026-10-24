//! Build Servo only when a Servo tab is first loaded. It remains process-scoped.
use std::{
    cell::{OnceCell, RefCell},
    ops::Deref,
    rc::Rc,
};
pub(crate) struct Host {
    servo: OnceCell<servo::Servo>,
    builder: RefCell<Option<servo::ServoBuilder>>,
    delegate: Rc<crate::engine::Delegate>,
}
impl Host {
    pub fn new(builder: servo::ServoBuilder, delegate: Rc<crate::engine::Delegate>) -> Self {
        Self {
            servo: OnceCell::new(),
            builder: RefCell::new(Some(builder)),
            delegate,
        }
    }
    pub fn initialized(&self) -> bool {
        self.servo.get().is_some()
    }
}
impl Deref for Host {
    type Target = servo::Servo;
    fn deref(&self) -> &Self::Target {
        self.servo.get_or_init(|| {
            let servo = self
                .builder
                .borrow_mut()
                .take()
                .expect("Servo initializes once")
                .build();
            servo.set_delegate(self.delegate.clone());
            servo
        })
    }
}
