#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    llvm_asm!("in $0, $1" : "={al}"(value) : "{dx}"(port) : "memory" : "intel", "volatile");
    value
}

#[inline(always)]
pub unsafe fn outb(port: u16, value: u8) {
    llvm_asm!("out $1, $0" : : "{al}"(value), "{dx}"(port) : "memory" : "intel", "volatile");
}
