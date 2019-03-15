extern crate ecflash;

use ecflash::{Ec, EcFlash};
use std::{fs, io, process};

pub struct Flasher{
    ec: EcFlash,
    size: usize,
}

impl Flasher {
    unsafe fn enter_follow_mode(&mut self) -> Result<(), ()> {
        self.ec.cmd(1)
    }

    unsafe fn spi_cmd(&mut self, cmd: u8) -> Result<(), ()> {
        self.ec.cmd(2)?;
        self.ec.cmd(cmd)
    }

    unsafe fn spi_write(&mut self, value: u8) -> Result<(), ()> {
        self.ec.cmd(3)?;
        self.ec.cmd(value)
    }

    unsafe fn spi_read(&mut self) -> Result<u8, ()> {
        self.ec.cmd(4)?;
        self.ec.read()
    }

    unsafe fn exit_follow_mode(&mut self) -> Result<(), ()> {
        self.ec.cmd(5)
    }

    unsafe fn spi_wait(&mut self) -> Result<(), ()> {
        self.enter_follow_mode()?;
        self.spi_cmd(5)?;
        while self.spi_read()? & 1 > 0 {}
        self.exit_follow_mode()
    }

    unsafe fn spi_write_enable(&mut self) -> Result<(), ()> {
        self.spi_wait()?;
        self.enter_follow_mode()?;
        self.spi_cmd(6)?;
        //TODO: extra spi command 80 based on device id 0xbf
        self.enter_follow_mode()?;
        self.spi_cmd(5)?;
        while self.spi_read()? & 3 != 2 {}
        self.exit_follow_mode()
    }

    unsafe fn spi_write_disable(&mut self) -> Result<(), ()> {
        self.spi_wait()?;
        self.enter_follow_mode()?;
        self.spi_cmd(4)?;
        self.enter_follow_mode()?;
        self.spi_cmd(5)?;
        while self.spi_read()? & 2 > 0 {}
        self.exit_follow_mode()
    }

    unsafe fn read(&mut self) -> Result<Vec<u8>, ()> {
        let mut buf = Vec::with_capacity(self.size);

        for block in 0..self.size/65536 {
            self.spi_write_disable()?;
            self.spi_wait()?;

            self.enter_follow_mode()?;

            self.spi_cmd(11)?;
            self.spi_write(block as u8)?;
            self.spi_write((block >> 8) as u8)?;
            self.spi_write((block >> 16) as u8)?;
            self.spi_write((block >> 24) as u8)?;

            for _ in 0..64 {
                for _ in 0..1024
                 {
                    buf.push(self.spi_read()?);
                }
                eprint!("\r{} KB", buf.len() / 1024);
            }

            self.spi_wait()?;
        }

        eprintln!("");

        Ok(buf)
    }

    unsafe fn start(&mut self) -> Result<u8, ()> {
        self.ec.cmd(0xDC)?;
        self.ec.read()
    }

    unsafe fn stop(&mut self) -> Result<(), ()> {
        self.ec.cmd(0x95)?;
        self.ec.cmd(0xFC)
    }
}

fn main() {
    extern {
        fn iopl(level: isize) -> isize;
    }

    // Get I/O Permission
    unsafe {
        if iopl(3) < 0 {
            eprintln!("Failed to get I/O permission: {}", io::Error::last_os_error());
            process::exit(1);
        }

        let mut ec = EcFlash::new(true).expect("Failed to find EC");

        let size = ec.size();

        let mut flasher = Flasher {
            ec: ec,
            size: size,
        };

        if flasher.start() == Ok(51) {
            if let Ok(data) = flasher.read() {
                let _ = fs::write("dump.rom", data);
            } else {
                eprintln!("Failed to read");
            }

            flasher.stop();
        }
    }
}
