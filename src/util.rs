#[inline(always)]
pub unsafe fn transmute<T: Copy + 'static, U: Copy + 'static>(value: T) -> U {
    *(&value as *const _ as *const _)
}

#[cfg(test)]
pub mod tests {
    use core::{mem, ptr};

    pub fn black_box<T>(value: T) -> T {
        let result = unsafe { ptr::read_volatile(&value) };
        mem::forget(value);
        result
    }
}
