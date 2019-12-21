use alloc::string::String;
use alloc::vec::Vec;

use super::Ec;

pub struct EcFile(Vec<u8>);

impl EcFile {
    pub unsafe fn get_str(&mut self, key: &[u8]) -> String {
        let mut string = String::new();

        let mut i = 0;
        let mut bytes = self.0.iter();
        while let Some(&byte) = bytes.next() {
            loop {
                if i < key.len() {
                    if byte == key[i] {
                        i += 1;
                        break;
                    } else if i == 0 {
                        break;
                    } else {
                        i = 0;
                    }
                } else if byte == b'$' {
                    return string;
                } else {
                    string.push(byte as char);
                    break;
                }
            }
        }

        string
    }

    pub fn new(data: Vec<u8>) -> Self {
        EcFile(data)
    }
}

impl Ec for EcFile {
    fn size(&mut self) -> usize {
        self.0.len()
    }

    fn project(&mut self) -> String {
        unsafe { self.get_str(b"PRJ:") }
    }

    fn version(&mut self) -> String {
        let version = unsafe { self.get_str(b"VER:") };
        version.trim_left_matches(' ')
    }
}
