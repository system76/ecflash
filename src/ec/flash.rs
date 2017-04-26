use std::io::{self, Error, ErrorKind, Result, Write};

use super::Ec;

extern crate libc;
extern crate x86;

use self::libc::c_int;
use self::x86::io::{inb, outb};

pub struct EcFlash {
    data_port: u16,
    cmd_port: u16
}

impl EcFlash {
    fn cmd(&mut self, data: u8) {
        unsafe {
            while inb(self.cmd_port) & 0x2 == 0x2 {}
            outb(self.cmd_port, data)
        }
    }

    fn read(&mut self) -> u8 {
        unsafe {
            while inb(self.cmd_port) & 0x1 == 0 {}
            inb(self.data_port)
        }
    }

    fn write(&mut self, data: u8) {
        unsafe {
            while inb(self.cmd_port) & 0x2 == 0x2 {}
            outb(self.data_port, data)
        }
    }

    fn get_str(&mut self, index: u8) -> String {
        let mut string = String::new();

        self.cmd(index);
        for _i in 0..256 {
            let byte = self.read();
            if byte == b'$' {
                break;
            } else {
                string.push(byte as char);
            }
        }

        string
    }

    pub fn new(number: u8) -> Result<Self> {
        extern {
            fn iopl(level: c_int) -> c_int;
        }

        // Get I/O Permission
        unsafe {
            if iopl(3) < 0 {
                return Err(Error::last_os_error());
            }
        }

        // Probe for Super I/O chip
        let id = unsafe {
            outb(0x2e, 0x20);
            let a = inb(0x2f);
            outb(0x2e, 0x21);
            let b = inb(0x2f);
            ((a as u16) << 8) | (b as u16)
        };

        if id != 0x8587 {
            return Err(Error::new(ErrorKind::NotFound, format!("Unknown EC ID: {:>04X}", id)));
        }

        let (data_port, cmd_port) = match number {
            0 => (0x60, 0x64),
            1 => (0x62, 0x66),
            2 => (0x68, 0x6c),
            3 => (0x6a, 0x6e),
            _ => {
                return Err(Error::new(ErrorKind::NotFound, format!("Unknown EC number: {}", number)));
            }
        };

        Ok(Self {
            data_port: data_port,
            cmd_port: cmd_port,
        })
    }
}

impl Ec for EcFlash {
    fn size(&mut self) -> usize {
        self.cmd(0x80);
        self.write(0xE5);
        if self.read() == 0x80 {
            128 * 1024
        } else {
            64 * 1024
        }
    }

    fn project(&mut self) -> String {
        self.get_str(0x92)
    }

    fn version(&mut self) -> String {
        let mut version = self.get_str(0x93);
        version.insert_str(0, "1.");
        version
    }

    unsafe fn dump(&mut self) -> Vec<u8> {
        // This is really, really dangerous to run from userspace right now!
        // The procedure needs to be improved!
        // Lockups have happened requiring battery removal or burnout to fix!

        let mut stderr = io::stderr();

        let size = self.size();

        let mut data = Vec::with_capacity(size);

        self.cmd(0xde);
        self.cmd(0xdc);

        let _ = writeln!(stderr, "Reading from ROM");

        for i in 0..size/65536 {
            self.cmd(0x03);
            self.cmd(i as u8);

            let _ = writeln!(stderr, "Block {}", i);

            for j in 0..0x100 {
                for _i in 0..0x100 {
                    data.push(self.read());
                }
                let _ = write!(stderr, "\r{}/{}", j + 1, 0x100);
            }

            let _ = writeln!(stderr, "");
        }

        self.cmd(0xfe);

        let _ = writeln!(stderr, "Read complete");

        data
    }
}