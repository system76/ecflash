extern crate ecflash;

use ecflash::{Ec, EcFlash};
use std::{fs, io, process};

pub struct Flasher{
    ec: EcFlash
}

impl Flasher {
    unsafe fn enter_follow_mode(&mut self) -> Result<(), ()> {
        self.ec.cmd(1)
    }

    unsafe fn spi_cmd(&mut self, cmd: u8) -> Result<(), ()> {
        self.ec.cmd(2)?;
        self.ec.cmd(cmd)
    }

    unsafe fn spi_write(&mut self, byte: u8) -> Result<(), ()> {
        self.ec.cmd(3)?;
        self.ec.cmd(byte)
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
        let size = self.ec.size();
        let mut buf = vec![0; size];

        self.spi_write_disable()?;
        self.spi_wait()?;

        self.enter_follow_mode()?;

        self.spi_cmd(11)?;
        self.spi_write(0)?;
        self.spi_write(0)?;
        self.spi_write(0)?;
        self.spi_write(0)?;

        for i in 0..buf.len() {
            buf[i] = self.spi_read()?;
        }

        self.spi_wait()?;

        Ok(buf)
    }
}

unsafe fn read() -> Result<(), &'static str> {
    let mut flasher = Flasher {
        ec: EcFlash::new(true).or(Err("Failed to find EC"))?
    };

    let data = flasher.read().or(Err("Failed to read EC SPI"))?;

    fs::write("dump.rom", data).or(Err("Failed to write dump.rom"))?;

    Ok(())
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

        if let Err(err) = read() {
            eprintln!("Failed to read EC data: {}", err);
            process::exit(1);
        }
    }
}
