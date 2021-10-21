#![no_std]
#![feature(asm)]

#[macro_use]
extern crate alloc;

use alloc::string::String;

pub use self::file::EcFile;
pub use self::flash::EcFlash;
pub use self::flasher::Flasher;

mod file;
mod flash;
mod flasher;
mod io;

pub trait Ec {
    fn size(&mut self) -> usize;
    fn project(&mut self) -> String;
    fn version(&mut self) -> String;
}
