use core::arch::asm;

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port, options(nostack));
    value
}

#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("al") value, in("dx") port, options(nostack));
}
