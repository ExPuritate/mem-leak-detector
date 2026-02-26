use crate::LeakDetector;

pub struct LeakDetectorScope<'a, T> {
    detector: &'a LeakDetector<T>,
    start: usize,
}

impl<T> LeakDetector<T> {
    pub fn scope<'a>(&'a self) -> LeakDetectorScope<'a, T> {
        LeakDetectorScope {
            detector: self,
            start: self.get_used(),
        }
    }
    pub fn scope_with<F: FnOnce<Args, Output = R>, Args: std::marker::Tuple, R>(
        &self,
        f: F,
        args: Args,
    ) -> R {
        let _guard = self.scope();
        f.call_once(args)
    }
}

#[cfg(debug_assertions)]
impl<'a, T> Drop for LeakDetectorScope<'a, T> {
    fn drop(&mut self) {
        let end = self.detector.get_used();
        assert_eq!(self.start, end);
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::System;

    use super::*;

    static _GLOBAL: LeakDetector<System> = LeakDetector::system();

    #[test]
    fn scope() {
        _GLOBAL.scope_with(
            || {
                let _boxed1 = Box::new_in(10, &_GLOBAL);
                let _boxed2 = Box::new_in(130, &_GLOBAL);
                let _boxed3 = Box::new_in(120, &_GLOBAL);
                let _boxed4 = Box::new_in(140, &_GLOBAL);
            },
            (),
        );
    }
}
