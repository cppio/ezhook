use crate::raw::Hook;
use core::{mem, slice};

#[doc(hidden)]
pub unsafe fn len<T: Copy>(start: usize, end: *const Hook<T>) -> usize {
    end as usize - start + mem::size_of::<Hook<T>>()
}

#[doc(hidden)]
pub unsafe fn copy_to<T: Copy>(start: usize, end: *const Hook<T>, dest: &mut [u8]) -> &mut Hook<T> {
    dest.copy_from_slice(slice::from_raw_parts(start as _, len(start, end)));

    let remote = &mut *((end as usize - start + dest.as_ptr() as usize) as *mut Hook<T>);
    remote.init(dest.as_ptr() as _);
    remote
}

#[macro_export]
macro_rules! remote_hook {
    {
        @dollar($dollar:tt)

        #[hook]
        $(#[$attr:meta])* $vis:vis
        $(unsafe $($unsafe:lifetime)?)? $(extern $($abi:literal)?)?
        fn $name:ident($($param:tt)*) $(-> $ret:ty)? $body:block

        $($item:item)*
    } => {
        $vis mod $name {
            mod hook {
                #[allow(unused_imports)]
                use super::super::*;

                #[allow(unused_macros)]
                macro_rules! toggle {
                    () => {
                        #[allow(unused_unsafe)]
                        unsafe { __REMOTE_HOOK_RAW.toggle_inline() }
                    }
                }

                #[allow(unused_macros)]
                macro_rules! orig {
                    ($dollar($arg:tt)*) => {
                        {
                            #[allow(unused_unsafe)]
                            let target = unsafe {
                                __REMOTE_HOOK_RAW.toggle_inline();

                                __REMOTE_HOOK_RAW.target_inline()
                            };

                            let result = target($dollar($arg)*);

                            #[allow(unused_unsafe)]
                            unsafe {
                                __REMOTE_HOOK_RAW.toggle_inline();
                            }

                            result
                        }
                    }
                }

                #[link_section = "remotehk"]
                $(#[$attr])* pub
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn $name($($param)*) $(-> $ret)? $body

                $(#[link_section = "remotehk"] $item)*

                #[link_section = "remotehk"]
                pub static mut __REMOTE_HOOK_RAW: $crate::raw::Hook<super::Func> = unsafe {
                    $crate::raw::Hook::empty()
                };
            }

            #[allow(unused_imports)]
            use super::*;

            type Func =
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn($($param)*) $(-> $ret)?
            ;

            #[allow(dead_code)]
            pub unsafe fn len() -> usize {
                $crate::remote::len(hook::$name as usize, &hook::__REMOTE_HOOK_RAW)
            }

            pub unsafe fn copy_to(dest: &mut [u8]) -> &mut $crate::raw::Hook<Func> {
                $crate::remote::copy_to(hook::$name as usize, &hook::__REMOTE_HOOK_RAW, dest)
            }
        }
    };

    ($($tt:tt)*) => { $crate::remote_hook! { @dollar($) $($tt)* } };
}

#[cfg(test)]
mod tests {
    use crate::{os::tests as os, util::tests as util};

    #[inline(never)]
    fn square(x: i32) -> i32 {
        util::black_box(x * x)
    }

    remote_hook! {
        #[hook]
        fn add_one_before(x: i32) -> i32 {
            orig!(x + 1)
        }
    }

    remote_hook! {
        #[hook]
        fn delayed(x: i32) -> i32 {
            update_last(orig!(x))
        }

        fn update_last(x: i32) -> i32 {
            let last = unsafe { LAST };
            unsafe { LAST = x };
            last
        }

        static mut LAST: i32 = 0;
    }

    fn setup(size: usize) -> &'static mut [u8] {
        os::unprotect(square as _, 5);

        os::allocate(square as _, size)
    }

    #[test]
    fn hook_macro() {
        let dest = setup(unsafe { add_one_before::len() });

        let hook = unsafe { add_one_before::copy_to(dest) };

        for _ in 0..2 {
            unsafe { hook.hook(square) };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { hook.toggle() };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 25);
            assert_eq!(square(5), 36);

            unsafe { hook.toggle() };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { hook.unhook() };

            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);
        }
    }

    #[test]
    fn hook_macro_state() {
        let dest = setup(unsafe { delayed::len() });

        let hook = unsafe { delayed::copy_to(dest) };

        unsafe { hook.hook(square) };
        unsafe { hook.toggle() };

        assert_eq!(square(4), 0);
        assert_eq!(square(5), 16);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 16);
        assert_eq!(square(5), 25);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 25);
        assert_eq!(square(5), 16);
    }
}
