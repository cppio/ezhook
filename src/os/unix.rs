#[cfg(test)]
pub mod tests {
    extern crate std;

    use core::slice;
    use libc::{
        mmap, mprotect, MAP_ANONYMOUS, MAP_FAILED, MAP_FIXED, MAP_PRIVATE, PROT_EXEC, PROT_READ,
        PROT_WRITE,
    };
    use std::{
        fs::File,
        io::{BufRead, BufReader},
        vec::Vec,
    };

    const PAGE_SIZE: usize = 0x1000;

    pub fn unprotect(address: usize, size: usize) {
        let page = address & !(PAGE_SIZE - 1);

        assert_eq!(
            unsafe {
                mprotect(
                    page as _,
                    address - page + size,
                    PROT_READ | PROT_WRITE | PROT_EXEC,
                )
            },
            0,
        );
    }

    pub fn allocate(mut address: usize, size: usize) -> &'static mut [u8] {
        let maps = File::open("/proc/self/maps").unwrap();

        for line in BufReader::new(maps).lines() {
            let range = line.as_ref().unwrap().split(' ').next().unwrap();

            if let [from, to] = &*range.split('-').collect::<Vec<_>>() {
                let from = usize::from_str_radix(from, 16).unwrap();
                let to = usize::from_str_radix(to, 16).unwrap();

                if from <= address && address < to {
                    address = to;
                }
            }
        }

        let region = unsafe {
            mmap(
                address as _,
                size,
                PROT_READ | PROT_WRITE | PROT_EXEC,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED,
                -1,
                0,
            )
        };
        assert_ne!(region, MAP_FAILED);

        unsafe { slice::from_raw_parts_mut(region as _, size) }
    }
}
