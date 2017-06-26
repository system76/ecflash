use std::fs::File;
use std::io::{Result, Read};
use std::path::Path;

use super::Ec;

pub struct EcFile(Vec<u8>);

impl EcFile {
    fn get_str(&mut self, key: &[u8]) -> String {
        let mut string = String::new();

        let mut i = 0;
        let mut bytes = self.0.iter();
        while let Some(&byte) = bytes.next() {
            if i < key.len() {
                if byte == key[i] {
                    i += 1;
                } else {
                    i = 0;
                }
            } else if byte == b'$' {
                break;
            } else {
                string.push(byte as char);
            }
        }

        string
    }

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        Ok(EcFile(data))
    }
}

impl Ec for EcFile {
    fn size(&mut self) -> usize {
        self.0.len()
    }

    fn project(&mut self) -> String {
        self.get_str(b"PRJ:")
    }

    fn version(&mut self) -> String {
        let mut version = self.get_str(b"VER:");
        while version.chars().next() == Some(' ') {
            version.remove(0);
        }
        version
    }

    unsafe fn dump(&mut self) -> Vec<u8> {
        self.0.clone()
    }
}