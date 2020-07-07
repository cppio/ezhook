use crate::util;

use core::convert::{TryFrom, TryInto};

pub struct Hook<T: 'static> {
    detour_target: T,
    scratch: [u8; 5],
}

impl<T> Hook<T> {
    pub const unsafe fn new(detour: T) -> Self {
        Self {
            detour_target: detour,
            scratch: [0xE9, 0, 0, 0, 0],
        }
    }
}

impl<T: Copy> Hook<T> {
    pub unsafe fn set_detour(&mut self, detour: T) {
        self.detour_target = detour;
    }

    pub unsafe fn hook(&mut self, target: T) {
        let detour: isize = util::transmute(self.detour_target);
        let target: isize = util::transmute(target);

        let offset = i32::try_from(detour - target - 5).unwrap();
        self.scratch[1..].copy_from_slice(&offset.to_ne_bytes());

        self.detour_target = util::transmute(target - self as *mut _ as isize);
    }

    pub unsafe fn unhook(&mut self) {
        let offset = i32::from_ne_bytes(self.scratch[1..].try_into().unwrap()) as isize;

        let target: isize = util::transmute(self.detour_target);
        self.detour_target = util::transmute(offset + target + self as *mut _ as isize + 5);
    }

    #[inline(always)]
    #[allow(clippy::manual_swap)]
    pub unsafe fn toggle_inline(&mut self) {
        let target: isize = util::transmute(self.detour_target);
        let target = (target + self as *mut _ as isize) as *mut _;

        let scratch = self.scratch;
        self.scratch = *target;
        *target = scratch;
    }

    pub unsafe fn toggle(&mut self) {
        self.toggle_inline()
    }

    #[inline(always)]
    pub unsafe fn target_inline(&self) -> T {
        let target: isize = util::transmute(self.detour_target);
        util::transmute(target + self as *const _ as isize)
    }

    pub unsafe fn target(&self) -> T {
        self.target_inline()
    }
}

#[macro_export]
macro_rules! local_swap_hook {
    {
        @dollar($dollar:tt)

        $(#[$attr:meta])* $vis:vis
        $(unsafe $($unsafe:lifetime)?)? $(extern $($abi:literal)?)?
        fn $name:ident($($param:tt)*) $(-> $ret:ty)? $body:block
    } => {
        $vis mod $name {
            mod __ez_hook {
                #[allow(unused_imports)]
                use super::super::*;

                #[allow(unused_macros)]
                macro_rules! toggle {
                    () => {
                        #[allow(unused_unsafe)]
                        unsafe { super::toggle() }
                    };
                }

                #[allow(unused_macros)]
                macro_rules! orig {
                    ($dollar($arg:tt)*) => {
                        {
                            #[allow(unused_unsafe)]
                            let target = unsafe {
                                super::toggle();

                                super::target()
                            };

                            let result = target($dollar($arg)*);

                            #[allow(unused_unsafe)]
                            unsafe {
                                super::toggle();
                            }

                            result
                        }
                    };
                }

                $(#[$attr])* pub
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn $name($($param)*) $(-> $ret)? $body
            }

            #[allow(unused_imports)]
            use super::*;

            #[allow(non_camel_case_types)]
            type __ez_Func =
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn($($param)*) $(-> $ret)?
            ;

            #[allow(non_upper_case_globals)]
            static mut __ez_HOOK: $crate::local::swap::Hook<__ez_Func> = unsafe {
                $crate::local::swap::Hook::new(__ez_hook::$name)
            };

            pub unsafe fn hook(target: __ez_Func) {
                __ez_HOOK.hook(target)
            }

            #[allow(dead_code)]
            pub unsafe fn unhook() {
                __ez_HOOK.unhook()
            }

            #[allow(dead_code)]
            pub unsafe fn toggle() {
                __ez_HOOK.toggle()
            }

            #[allow(dead_code)]
            pub unsafe fn target() -> __ez_Func {
                __ez_HOOK.target()
            }
        }
    };

    ($($tt:tt)*) => { $crate::local_swap_hook! { @dollar($) $($tt)* } };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util;

    #[inline(never)]
    fn square(x: i32) -> i32 {
        util::black_box(x * x)
    }

    fn identity(x: i32) -> i32 {
        x
    }

    local_swap_hook! {
        fn add_one_before(x: i32) -> i32 {
            orig!(x + 1)
        }
    }

    fn setup() {
        util::unprotect(square as _, 5);
    }

    #[test]
    fn hook() {
        setup();

        let mut hook = unsafe { Hook::<fn(i32) -> i32>::new(identity) };

        for _ in 0..2 {
            unsafe { hook.hook(square) };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { hook.toggle() };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 4);
            assert_eq!(square(5), 5);

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
    fn hook_macro() {
        setup();

        for _ in 0..2 {
            unsafe { add_one_before::hook(square) };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { add_one_before::toggle() };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 25);
            assert_eq!(square(5), 36);

            unsafe { add_one_before::toggle() };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { add_one_before::unhook() };

            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);
        }
    }

    #[test]
    fn hook_multiple() {
        setup();

        let mut hook = unsafe { Hook::<fn(i32) -> i32>::new(identity) };

        unsafe { add_one_before::hook(square) };
        unsafe { hook.hook(square) };

        assert_eq!(square(4), 16);
        assert_eq!(square(5), 25);

        unsafe { add_one_before::toggle() };

        assert_eq!(square(4), 25);
        assert_eq!(square(5), 36);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 4);
        assert_eq!(square(5), 5);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 25);
        assert_eq!(square(5), 36);

        unsafe { add_one_before::toggle() };

        assert_eq!(square(4), 16);
        assert_eq!(square(5), 25);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 4);
        assert_eq!(square(5), 5);

        unsafe { add_one_before::toggle() };

        assert_eq!(square(4), 5);
        assert_eq!(square(5), 6);

        unsafe { add_one_before::toggle() };

        assert_eq!(square(4), 4);
        assert_eq!(square(5), 5);

        unsafe { hook.toggle() };

        assert_eq!(square(4), 16);
        assert_eq!(square(5), 25);
    }
}
