use core::alloc::Layout;
use core::{mem, ptr};

use alloc::alloc::dealloc;
use alloc::boxed::Box;

// SAFETY:
// - T and U must have identical size and alignment (checked by debug_assertions)
// - T and U must be POD types (no destructors, trivial copy/drop)
// - If `f` panics, memory is safely deallocated without dropping elements
pub unsafe fn map_boxed_slice<T, U, F>(boxed_slice: Box<[T]>, f: F) -> Box<[U]>
where
    F: Fn(T) -> U,
{
    // Verify size and alignment requirements
    debug_assert_eq!(mem::size_of::<T>(), mem::size_of::<U>());
    debug_assert_eq!(mem::align_of::<T>(), mem::align_of::<U>());

    // Decompose Box<[T]> into raw parts
    let len = boxed_slice.len();
    let layout = Layout::array::<T>(len).expect("Invalid layout");
    let data_ptr = Box::into_raw(boxed_slice) as *mut T;

    // Guard to handle panic: deallocates memory without dropping elements
    let guard = Guard {
        data: data_ptr as *mut u8,
        layout,
    };

    // Process each element
    for i in 0..len {
        // SAFETY: Valid index in allocated memory
        let t = ptr::read(data_ptr.add(i));
        let u = f(t);
        // SAFETY: Same layout as T, memory is initialized
        ptr::write(data_ptr.add(i) as *mut U, u);
    }

    // Prevent guard from running (success case)
    core::mem::forget(guard);

    // Reconstruct as Box<[U]>
    // SAFETY: We've initialized all elements as U with correct layout
    Box::from_raw(ptr::slice_from_raw_parts_mut(data_ptr as *mut U, len))
}

// Guard for panic safety: deallocates memory without dropping elements
struct Guard {
    data: *mut u8,
    layout: Layout,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: Original allocation parameters match layout
        unsafe { dealloc(self.data, self.layout) };
    }
}
