#[cfg(test)]
pub mod tests {
    use core::{
        mem::{self, MaybeUninit},
        slice,
    };
    use winapi::um::{
        memoryapi::{VirtualAlloc, VirtualProtect, VirtualQuery},
        winnt::{MEM_COMMIT, MEM_FREE, MEM_RESERVE, PAGE_EXECUTE_READWRITE},
    };

    const ALLOCATION_GRANULARITY: usize = 0x10000;

    pub fn unprotect(address: usize, size: usize) {
        assert_ne!(
            unsafe { VirtualProtect(address as _, size, PAGE_EXECUTE_READWRITE, &mut 0) },
            0,
        );
    }

    pub fn allocate(address: usize, size: usize) -> &'static mut [u8] {
        let mut address = address & !(ALLOCATION_GRANULARITY - 1);

        let mut info = MaybeUninit::uninit();

        loop {
            assert_eq!(
                unsafe { VirtualQuery(address as _, info.as_mut_ptr(), mem::size_of_val(&info)) },
                mem::size_of_val(&info),
            );

            if unsafe { info.assume_init().State } & MEM_FREE != 0 {
                break;
            }

            address += ALLOCATION_GRANULARITY;
        }

        let region = unsafe {
            VirtualAlloc(
                address as _,
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        assert!(!region.is_null());

        unsafe { slice::from_raw_parts_mut(region as _, size) }
    }
}
