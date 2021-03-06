use crate::{local::trampoline::Hook, util};

use core::{convert::TryInto, mem, slice};

#[doc(hidden)]
pub unsafe fn len<T: Copy>(end: &'static Hook<T>, start: T) -> usize {
    let start: usize = util::transmute(start);
    end as *const _ as usize - start + mem::size_of_val(end) + 24
}

#[doc(hidden)]
pub unsafe fn copy_to<T: Copy>(
    end: &'static Hook<T>,
    start: T,
    dest: &'static mut [u8],
) -> &'static mut Hook<T> {
    let size = len(end, start) - 24;
    dest[..size].copy_from_slice(slice::from_raw_parts(util::transmute(start), size));

    let remote = &mut *(dest[size - mem::size_of_val(end)..].as_mut_ptr() as *mut Hook<T>);
    remote.set_detour(util::transmute(dest.as_ptr()));
    remote.set_trampoline((&mut dest[size..]).try_into().unwrap());
    remote
}

#[macro_export]
macro_rules! remote_trampoline_hook {
    {
        @dollar($dollar:tt)

        #[hook]
        $(#[$attr:meta])* $vis:vis
        $(unsafe $($unsafe:lifetime)?)? $(extern $($abi:literal)?)?
        fn $name:ident($($param:tt)*) $(-> $ret:ty)? $body:block

        $($item:item)*
    } => {
        $vis mod $name {
            mod __ez_hook {
                #[allow(unused_imports)]
                use super::super::*;

                #[allow(unused_macros)]
                macro_rules! toggle {
                    () => {
                        #[allow(unused_unsafe)]
                        unsafe { __ez_HOOK.toggle_inline() }
                    };
                }

                #[allow(unused_macros)]
                macro_rules! orig {
                    ($dollar($arg:tt)*) => {
                        {
                            #[allow(unused_unsafe)]
                            let trampoline = unsafe {
                                __ez_HOOK.trampoline_inline()
                            };

                            trampoline($dollar($arg)*)
                        }
                    };
                }

                #[link_section = "ezhk,rem"]
                $(#[$attr])* pub
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn $name($($param)*) $(-> $ret)? $body

                $(#[link_section = "ezhk,rem"] $item)*

                #[link_section = "ezhk,rem"]
                #[allow(non_upper_case_globals)]
                pub static mut __ez_HOOK: $crate::local::trampoline::Hook<super::__ez_Func> =
                    unsafe { $crate::local::trampoline::Hook::new($name) };
            }

            #[allow(unused_imports)]
            use super::*;

            #[allow(non_camel_case_types)]
            type __ez_Func =
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn($($param)*) $(-> $ret)?
            ;

            #[allow(dead_code)]
            pub unsafe fn len() -> usize {
                $crate::remote::trampoline::len(&__ez_hook::__ez_HOOK, __ez_hook::$name)
            }

            pub unsafe fn copy_to(
                dest: &'static mut [u8],
            ) -> &mut $crate::local::trampoline::Hook<__ez_Func> {
                $crate::remote::trampoline::copy_to(&__ez_hook::__ez_HOOK, __ez_hook::$name, dest)
            }
        }
    };

    ($($tt:tt)*) => { $crate::remote_trampoline_hook! { @dollar($) $($tt)* } };
}

#[cfg(test)]
#[cfg(not(all(target_arch = "x86", windows)))]
#[allow(clippy::fn_address_comparisons)]
mod tests {
    use crate::util;

    #[inline(never)]
    fn square(x: i32) -> i32 {
        util::black_box(x * x)
    }

    remote_trampoline_hook! {
        #[hook]
        fn add_one_before(x: i32) -> i32 {
            orig!(x + 1)
        }
    }

    remote_trampoline_hook! {
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
        util::unprotect(square as _, 5);

        util::allocate(square as _, size)
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
            assert_eq!(unsafe { hook.trampoline() }(4), 16);
            assert_eq!(unsafe { hook.trampoline() }(5), 25);

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

        unsafe { hook.toggle() };

        assert_eq!(square(4), 16);
        assert_eq!(square(5), 25);
    }
}
