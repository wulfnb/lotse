//! Engine-independent tab transitions. Only this model may change selection/order.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Select(usize),
    Close(usize),
    Move { from: usize, to: usize },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Event<T> {
    Selected,
    Closed(T),
    Reordered,
    Unchanged,
}
pub struct Session<T> {
    tabs: Vec<T>,
    active: usize,
}
impl<T> Default for Session<T> {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
        }
    }
}
impl<T> Session<T> {
    pub fn tabs(&self) -> &[T] {
        &self.tabs
    }
    pub fn tabs_mut(&mut self) -> &mut [T] {
        &mut self.tabs
    }
    pub fn active(&self) -> usize {
        self.active
    }
    pub fn push(&mut self, tab: T) {
        self.tabs.push(tab);
    }
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.active = 0;
    }
    pub fn apply(&mut self, command: Command) -> Event<T> {
        match command {
            Command::Select(index) if index < self.tabs.len() => {
                self.active = index;
                Event::Selected
            }
            Command::Close(index) if index < self.tabs.len() => {
                let tab = self.tabs.remove(index);
                if index < self.active {
                    self.active -= 1;
                }
                self.active = self.active.min(self.tabs.len().saturating_sub(1));
                Event::Closed(tab)
            }
            Command::Move { from, to }
                if from < self.tabs.len() && to < self.tabs.len() && from != to =>
            {
                crate::profile::move_tab(&mut self.tabs, &mut self.active, from, to);
                Event::Reordered
            }
            _ => Event::Unchanged,
        }
    }
}
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
