pub use self::file::EcFile;
pub use self::flash::EcFlash;

mod file;
mod flash;

pub trait Ec {
    fn size(&mut self) -> usize;
    fn project(&mut self) -> String;
    fn version(&mut self) -> String;
    unsafe fn dump(&mut self) -> Vec<u8>;
}