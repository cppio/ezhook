#[inline(always)]
pub unsafe fn transmute<T: Copy + 'static, U: Copy + 'static>(value: T) -> U {
    *(&value as *const _ as *const _)
}

#[cfg(test)]
pub use test::*;

#[cfg(test)]
mod test {
    use core::{mem, ptr};

    pub fn black_box<T>(value: T) -> T {
        let result = unsafe { ptr::read_volatile(&value) };
        mem::forget(value);
        result
    }

    #[cfg(unix)]
    pub use unix::*;

    #[cfg(unix)]
    mod unix {
        use core::slice;
        use libc::{
            mmap, mprotect, MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, PROT_EXEC, PROT_READ,
            PROT_WRITE,
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

        #[cfg(target_os = "linux")]
        pub fn allocate(mut address: usize, size: usize) -> &'static mut [u8] {
            extern crate std;

            use libc::MAP_FIXED;
            use std::{
                fs::File,
                io::{BufRead, BufReader},
                vec::Vec,
            };

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

        #[cfg(not(target_os = "linux"))]
        pub fn allocate(address: usize, size: usize) -> &'static mut [u8] {
            let region = unsafe {
                mmap(
                    address as _,
                    size,
                    PROT_READ | PROT_WRITE | PROT_EXEC,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            assert_ne!(region, MAP_FAILED);

            unsafe { slice::from_raw_parts_mut(region as _, size) }
        }
    }

    #[cfg(windows)]
    pub use windows::*;

    #[cfg(windows)]
    mod windows {
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
                    unsafe {
                        VirtualQuery(address as _, info.as_mut_ptr(), mem::size_of_val(&info))
                    },
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
}
