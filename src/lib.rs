#![no_std]
#![feature(alloc)]

#[macro_use]
extern crate alloc;

use alloc::{String, Vec};

pub use self::file::EcFile;
pub use self::flash::EcFlash;

mod file;
mod flash;

pub trait Ec {
    fn size(&mut self) -> usize;
    fn project(&mut self) -> String;
    fn version(&mut self) -> String;
}
