use crate::util;

use core::convert::{TryFrom, TryInto};

#[cfg(target_arch = "x86")]
use lde::X86;

#[cfg(target_arch = "x86_64")]
use lde::X64 as X86;

pub struct Hook<T: 'static> {
    detour_target: T,
    trampoline: isize,
}

impl<T> Hook<T> {
    pub const unsafe fn new(detour: T) -> Self {
        Self {
            detour_target: detour,
            trampoline: 0,
        }
    }
}

impl<T: Copy> Hook<T> {
    pub unsafe fn set_detour(&mut self, detour: T) {
        self.detour_target = detour;
    }

    pub unsafe fn set_trampoline(&mut self, trampoline: &'static mut [u8; 24]) {
        trampoline[0] = 0xE9;
        self.trampoline = trampoline.as_ptr() as isize - self as *mut _ as isize;
    }

    pub unsafe fn hook(&mut self, target: T) {
        let detour: isize = util::transmute(self.detour_target);
        let target: isize = util::transmute(target);

        let trampoline = &mut *((self.trampoline + self as *mut _ as isize) as *mut [u8; 24]);

        let offset = i32::try_from(detour - target - 5).unwrap();
        trampoline[1..5].copy_from_slice(&offset.to_ne_bytes());

        let offset = i32::try_from(target - trampoline.as_ptr() as isize - 5).unwrap();

        self.detour_target = util::transmute(target - self as *mut _ as isize);
        let target = &*(target as *const [u8; 19]);

        let mut count = 0;
        while count < 5 {
            count += X86.ld(&target[count..]) as usize;
        }

        trampoline[5..count].copy_from_slice(&target[5..count]);

        let jump = &mut trampoline[count..];
        jump[0] = 0xE9;
        jump[1..5].copy_from_slice(&offset.to_ne_bytes());
    }

    pub unsafe fn unhook(&mut self) {
        let trampoline = &*((self.trampoline + self as *mut _ as isize) as *const [u8; 5]);

        let offset = i32::from_ne_bytes(trampoline[1..].try_into().unwrap()) as isize;

        let target: isize = util::transmute(self.detour_target);
        self.detour_target = util::transmute(offset + target + self as *mut _ as isize + 5);
    }

    #[inline(always)]
    pub unsafe fn toggle_inline(&mut self) {
        let trampoline = (self.trampoline + self as *mut _ as isize) as *mut [u8; 5];

        let target: isize = util::transmute(self.detour_target);
        let target = (target + self as *mut _ as isize) as *mut _;

        let scratch = *trampoline;
        *trampoline = *target;
        *target = scratch;
    }

    pub unsafe fn toggle(&mut self) {
        self.toggle_inline()
    }

    pub unsafe fn target(&self) -> T {
        let target: isize = util::transmute(self.detour_target);
        util::transmute(target + self as *const _ as isize)
    }

    #[inline(always)]
    pub unsafe fn trampoline_inline(&self) -> T {
        util::transmute(self.trampoline + self as *const _ as isize)
    }

    pub unsafe fn trampoline(&self) -> T {
        self.trampoline_inline()
    }
}

#[macro_export]
macro_rules! local_trampoline_hook {
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
                            let trampoline = unsafe {
                                super::trampoline()
                            };

                            trampoline($dollar($arg)*)
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
            static mut __ez_HOOK: $crate::local::trampoline::Hook<__ez_Func> = unsafe {
                $crate::local::trampoline::Hook::new(__ez_hook::$name)
            };

            #[allow(dead_code)]
            pub unsafe fn set_trampoline(trampoline: &'static mut [u8; 24]) {
                __ez_HOOK.set_trampoline(trampoline)
            }

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

            #[allow(dead_code)]
            pub unsafe fn trampoline() -> __ez_Func {
                __ez_HOOK.trampoline()
            }
        }
    };

    ($($tt:tt)*) => { $crate::local_trampoline_hook! { @dollar($) $($tt)* } };
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

    local_trampoline_hook! {
        fn add_one_before(x: i32) -> i32 {
            orig!(x + 1)
        }
    }

    fn setup() -> &'static mut [u8; 24] {
        util::unprotect(square as _, 5);

        util::allocate(square as _, 24).try_into().unwrap()
    }

    #[test]
    fn hook() {
        let trampoline = setup();

        let mut hook = unsafe { Hook::<fn(i32) -> i32>::new(identity) };
        unsafe { hook.set_trampoline(trampoline) };

        for _ in 0..2 {
            unsafe { hook.hook(square) };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { hook.toggle() };

            assert!(unsafe { hook.target() } == square);
            assert_eq!(square(4), 4);
            assert_eq!(square(5), 5);
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
    fn hook_macro() {
        let trampoline = setup();

        unsafe { add_one_before::set_trampoline(trampoline) };

        for _ in 0..2 {
            unsafe { add_one_before::hook(square) };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { add_one_before::toggle() };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 25);
            assert_eq!(square(5), 36);
            assert_eq!(unsafe { add_one_before::trampoline() }(4), 16);
            assert_eq!(unsafe { add_one_before::trampoline() }(5), 25);

            unsafe { add_one_before::toggle() };

            assert!(unsafe { add_one_before::target() } == square);
            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);

            unsafe { add_one_before::unhook() };

            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);
        }
    }
}
