/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Routines for handling measuring the memory usage of arbitrary DOM nodes.

// Modified by Wega contributors, 2026: distinguish Box and Rc DOM allocations.
use std::{os::raw::c_void, rc::Rc};

use malloc_size_of::{MallocSizeOf, MallocSizeOfOps, MallocUnconditionalSizeOf};

/// Used by codegen to include the pointer to the `MallocSizeOf` implementation of each
/// IDL interface. This way we don't have to find the most-derived interface of DOM
/// objects by hand in code.
#[expect(unsafe_code)]
pub(crate) unsafe fn malloc_size_of_including_raw_self<T: MallocSizeOf, const IS_RC: bool>(
    ops: &mut MallocSizeOfOps,
    obj: *const c_void,
) -> usize {
    if IS_RC {
        // Rc-backed DOM objects are leaked with Rc::into_raw by
        // MaybeUnreflectedDom::from_rc. Their value pointer is not an allocation
        // base. Borrow ownership through an extra strong reference and let the
        // Rc memory-measurement implementation locate the allocation header.
        let object = unsafe {
            Rc::increment_strong_count(obj.cast::<T>());
            Rc::from_raw(obj.cast::<T>())
        };
        object.unconditional_size_of(ops)
    } else {
        unsafe { ops.malloc_size_of(obj) + (*(obj as *const T)).size_of(ops) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static VALUE: AtomicUsize = AtomicUsize::new(0);
    #[repr(align(64))]
    struct Sample;
    impl MallocSizeOf for Sample {
        fn size_of(&self, _: &mut MallocSizeOfOps) -> usize {
            7
        }
    }

    unsafe extern "C" fn rc_size(pointer: *const c_void) -> usize {
        assert!(
            (pointer as usize) < VALUE.load(Ordering::Relaxed),
            "Allocator must receive the Rc allocation base, not its value pointer"
        );
        128
    }

    #[test]
    fn rc_dom_measurement_preserves_ownership_and_measures_the_allocation_base() {
        let owner = Rc::new(Sample);
        let pointer = Rc::into_raw(owner.clone());
        VALUE.store(pointer as usize, Ordering::Relaxed);
        let mut ops = MallocSizeOfOps::new(rc_size, None, None);
        assert_eq!(
            unsafe { malloc_size_of_including_raw_self::<Sample, true>(&mut ops, pointer.cast()) },
            135
        );
        assert_eq!(Rc::strong_count(&owner), 2);
        unsafe {
            drop(Rc::from_raw(pointer));
        }
        assert_eq!(Rc::strong_count(&owner), 1);
    }

    unsafe extern "C" fn box_size(_: *const c_void) -> usize {
        64
    }

    #[test]
    fn boxed_dom_measurement_keeps_the_direct_allocation_path() {
        let object = Box::new(42_u64);
        let pointer = (&*object as *const u64).cast();
        let mut ops = MallocSizeOfOps::new(box_size, None, None);
        assert_eq!(
            unsafe { malloc_size_of_including_raw_self::<u64, false>(&mut ops, pointer) },
            64
        );
        assert_eq!(*object, 42);
    }
}
