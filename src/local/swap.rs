use crate::util;

use core::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
};

#[repr(C)]
pub struct Hook<T: 'static> {
    offset: isize,
    scratch: [u8; 5],
    phantom: PhantomData<T>,
}

impl<T> Hook<T> {
    #[doc(hidden)]
    pub const unsafe fn empty() -> Self {
        Self {
            offset: 0,
            scratch: [0xE9, 0, 0, 0, 0],
            phantom: PhantomData,
        }
    }
}

impl<T: Copy> Hook<T> {
    pub unsafe fn new(detour: T) -> Self {
        let mut hook = Self::empty();
        hook.offset = util::transmute(detour);
        hook
    }

    #[doc(hidden)]
    pub unsafe fn init(&mut self, offset: isize) {
        self.offset = offset;
    }

    pub unsafe fn hook(&mut self, target: T) {
        let target: isize = util::transmute(target);

        let offset = i32::try_from(self.offset - target - 5).unwrap();
        self.scratch[1..].copy_from_slice(&offset.to_ne_bytes());

        self.offset = target - self as *mut _ as isize;
    }

    pub unsafe fn unhook(&mut self) {
        let target = self.offset + self as *mut _ as isize;
        let offset = i32::from_ne_bytes(self.scratch[1..].try_into().unwrap()) as isize;

        self.offset = offset + target + 5;
    }

    #[inline(always)]
    #[doc(hidden)]
    #[allow(clippy::manual_swap)]
    pub unsafe fn toggle_inline(&mut self) {
        let target = (self.offset + self as *mut _ as isize) as *mut [u8; 5];

        let scratch = self.scratch;
        self.scratch = *target;
        *target = scratch;
    }

    pub unsafe fn toggle(&mut self) {
        self.toggle_inline()
    }

    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn target_inline(&self) -> T {
        util::transmute(self.offset + self as *const _ as isize)
    }

    pub unsafe fn target(&self) -> T {
        self.target_inline()
    }
}

#[doc(hidden)]
#[repr(C)]
pub struct ConstHook<T: 'static> {
    detour: T,
    scratch: [u8; 5],
}

impl<T> ConstHook<T> {
    #[doc(hidden)]
    pub const unsafe fn new(detour: T) -> Self {
        Self {
            detour,
            scratch: [0xE9, 0, 0, 0, 0],
        }
    }
}

impl<T: Copy> ConstHook<T> {
    #[doc(hidden)]
    pub unsafe fn raw(&mut self) -> &mut Hook<T> {
        &mut *(self as *mut _ as *mut _)
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
            mod hook {
                #[allow(unused_imports)]
                use super::super::*;

                #[allow(unused_macros)]
                macro_rules! toggle {
                    () => {
                        #[allow(unused_unsafe)]
                        unsafe { super::HOOK.raw().toggle() }
                    }
                }

                #[allow(unused_macros)]
                macro_rules! orig {
                    ($dollar($arg:tt)*) => {
                        {
                            #[allow(unused_unsafe)]
                            let target = unsafe {
                                super::HOOK.raw().toggle();

                                super::HOOK.raw().target()
                            };

                            let result = target($dollar($arg)*);

                            #[allow(unused_unsafe)]
                            unsafe {
                                super::HOOK.raw().toggle();
                            }

                            result
                        }
                    }
                }

                $(#[$attr])* pub
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn $name($($param)*) $(-> $ret)? $body
            }

            #[allow(unused_imports)]
            use super::*;

            type Func =
                $(unsafe $($unsafe)?)? $(extern $($abi)?)?
                fn($($param)*) $(-> $ret)?
            ;

            static mut HOOK: $crate::local::swap::ConstHook<Func> = unsafe {
                $crate::local::swap::ConstHook::new(hook::$name)
            };

            pub unsafe fn hook(target: Func) {
                HOOK.raw().hook(target)
            }

            #[allow(dead_code)]
            pub unsafe fn unhook() {
                HOOK.raw().unhook()
            }

            #[allow(dead_code)]
            pub unsafe fn toggle() {
                HOOK.raw().toggle()
            }

            #[allow(dead_code)]
            pub unsafe fn target() -> Func {
                HOOK.raw().target()
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
