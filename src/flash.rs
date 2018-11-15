use alloc::string::String;

use super::Ec;
use super::io::{inb, outb};

const TIMEOUT: usize = 100000;

pub struct EcFlash {
    primary: bool,
    data_port: u16,
    cmd_port: u16
}

impl EcFlash {
    pub unsafe fn cmd(&mut self, data: u8) -> Result<(), ()> {
        let mut i = TIMEOUT;
        while inb(self.cmd_port) & 0x2 == 0x2 && i > 0 {
            i -= 1;
        }

        if i == 0 {
            Err(())
        } else {
            Ok(outb(self.cmd_port, data))
        }
    }

    pub unsafe fn read(&mut self) -> Result<u8, ()> {
        let mut i = TIMEOUT;
        while inb(self.cmd_port) & 0x1 == 0  && i > 0 {
            i -= 1;
        }

        if i == 0 {
            Err(())
        } else {
            Ok(inb(self.data_port))
        }
    }

    pub unsafe fn write(&mut self, data: u8) -> Result<(), ()> {
        let mut i = TIMEOUT;
        while inb(self.cmd_port) & 0x2 == 0x2 && i > 0 {
            i -= 1;
        }

        if i == 0 {
            Err(())
        } else {
            Ok(outb(self.data_port, data))
        }
    }

    pub unsafe fn flush(&mut self) -> Result<(), ()> {
        let mut i = TIMEOUT;
        while inb(self.cmd_port) & 0x1 == 0x1 && i > 0 {
            inb(self.data_port);
            i -= 1;
        }

        if i == 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    pub unsafe fn get_param(&mut self, param: u8) -> Result<u8, ()> {
        self.cmd(0x80)?;
        self.write(param)?;
        self.read()
    }

    pub unsafe fn set_param(&mut self, param: u8, data: u8) -> Result<(), ()> {
        self.cmd(0x81)?;
        self.write(param)?;
        self.write(data)
    }

    pub unsafe fn get_str(&mut self, index: u8) -> Result<String, ()> {
        let mut string = String::new();

        self.cmd(index)?;
        for _i in 0..16 {
            let byte = self.read()?;
            if byte == b'$' {
                break;
            } else {
                string.push(byte as char);
            }
        }

        Ok(string)
    }

    pub fn new(primary: bool) -> Result<Self, String> {
        // Probe for Super I/O chip
        let id = unsafe {
            outb(0x2e, 0x20);
            let a = inb(0x2f);
            outb(0x2e, 0x21);
            let b = inb(0x2f);
            ((a as u16) << 8) | (b as u16)
        };

        if id != 0x8587 {
            return Err(format!("Unknown EC ID: {:>04X}", id));
        }

        let (data_port, cmd_port) = if primary {
            (0x62, 0x66)
        } else {
            (0x68, 0x6c)
        };

        let ec = Self {
            primary: primary,
            data_port: data_port,
            cmd_port: cmd_port,
        };

        Ok(ec)
    }
}

impl Ec for EcFlash {
    fn size(&mut self) -> usize {
        let _ = unsafe { self.flush() };

        if self.primary && unsafe { self.get_param(0xE5) } == Ok(0x80) {
            128 * 1024
        } else {
            64 * 1024
        }
    }

    fn project(&mut self) -> String {
        let _ = unsafe { self.flush() };

        unsafe { self.get_str(0x92) }.unwrap_or(String::new())
    }

    fn version(&mut self) -> String {
        let _ = unsafe { self.flush() };

        let mut version = unsafe { self.get_str(0x93) }.unwrap_or(String::new());
        version.insert_str(0, "1.");
        version
    }
}
