use crate::util;

use core::convert::{TryFrom, TryInto};

#[cfg(target_arch = "x86")]
use lde::X86;

#[cfg(target_arch = "x86_64")]
use lde::X64 as X86;

pub struct Hook<T: 'static> {
    detour_trampoline: T,
}

impl<T> Hook<T> {
    pub const unsafe fn new(detour: T) -> Self {
        Self {
            detour_trampoline: detour,
        }
    }
}

impl<T: Copy> Hook<T> {
    pub unsafe fn set_detour(&mut self, detour: T) {
        self.detour_trampoline = detour;
    }

    pub unsafe fn hook(&mut self, target: T, trampoline: &'static mut [u8; 24]) {
        let detour: isize = util::transmute(self.detour_trampoline);
        let target: isize = util::transmute(target);

        let offset = i32::try_from(detour - target - 5).unwrap();
        let offset2 = i32::try_from(target - trampoline.as_ptr() as isize - 5).unwrap();

        let target = &mut *(target as *mut [u8; 19]);

        let mut count = 0;

        while count < 5 {
            count += X86.ld(&target[count..]) as usize;
        }

        trampoline[..count].copy_from_slice(&target[..count]);

        target[0] = 0xE9;
        target[1..5].copy_from_slice(&offset.to_ne_bytes());

        let jump = &mut trampoline[count..];

        jump[0] = 0xE9;
        jump[1..5].copy_from_slice(&offset2.to_ne_bytes());
        jump[5..].copy_from_slice(&[0xCC; 14][..19 - count]);

        self.detour_trampoline = util::transmute(trampoline);
    }

    pub unsafe fn unhook(&mut self) {
        let trampoline: *const [u8; 24] = util::transmute(self.detour_trampoline);

        let jump = X86
            .iter(&*trampoline, 0)
            .fold(&[0; 5], |jump, (opcode, _)| {
                (**opcode).try_into().unwrap_or(jump)
            });

        let offset2 = i32::from_ne_bytes(jump[1..].try_into().unwrap()) as isize;
        let target = (offset2 + trampoline as isize + 5) as *mut [u8; 5];

        let offset = i32::from_ne_bytes((*target)[1..].try_into().unwrap()) as isize;
        self.detour_trampoline = util::transmute(offset + target as isize + 5);

        *target = *(trampoline as *const _);
    }

    #[inline(always)]
    pub unsafe fn trampoline_inline(&self) -> T {
        self.detour_trampoline
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

            pub unsafe fn hook(target: __ez_Func, trampoline: &'static mut [u8; 24]) {
                __ez_HOOK.hook(target, trampoline)
            }

            #[allow(dead_code)]
            pub unsafe fn unhook() {
                __ez_HOOK.unhook()
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

    fn setup() -> *mut [u8; 24] {
        util::unprotect(square as _, 5);

        util::allocate(square as _, 24).as_mut_ptr() as _
    }

    #[test]
    fn hook() {
        let trampoline = setup();

        let mut hook = unsafe { Hook::<fn(i32) -> i32>::new(identity) };

        for _ in 0..2 {
            unsafe { hook.hook(square, &mut *trampoline) };

            assert_eq!(square(4), 4);
            assert_eq!(square(5), 5);

            assert_eq!(unsafe { hook.trampoline() }(4), 16);
            assert_eq!(unsafe { hook.trampoline() }(5), 25);

            unsafe { hook.unhook() };

            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);
        }
    }

    #[test]
    fn hook_macro() {
        let trampoline = setup();

        for _ in 0..2 {
            unsafe { add_one_before::hook(square, &mut *trampoline) };

            assert_eq!(square(4), 25);
            assert_eq!(square(5), 36);

            assert_eq!(unsafe { add_one_before::trampoline() }(4), 16);
            assert_eq!(unsafe { add_one_before::trampoline() }(5), 25);

            unsafe { add_one_before::unhook() };

            assert_eq!(square(4), 16);
            assert_eq!(square(5), 25);
        }
    }
}
