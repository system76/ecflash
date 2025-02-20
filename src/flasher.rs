#![allow(clippy::missing_safety_doc)]
// TODO: Use real errors
#![allow(clippy::result_unit_err)]

use alloc::vec::Vec;

use super::{Ec, EcFlash};

pub struct Flasher {
    ec: EcFlash,
    pub size: usize,
}

impl Flasher {
    pub fn new(mut ec: EcFlash) -> Self {
        let size = ec.size();
        Self { ec, size }
    }

    unsafe fn enter_follow_mode(&mut self) -> Result<(), ()> {
        unsafe { self.ec.cmd(1) }
    }

    unsafe fn spi_cmd(&mut self, cmd: u8) -> Result<(), ()> {
        unsafe {
            self.ec.cmd(2)?;
            self.ec.cmd(cmd)
        }
    }

    unsafe fn spi_write(&mut self, value: u8) -> Result<(), ()> {
        unsafe {
            self.ec.cmd(3)?;
            self.ec.cmd(value)
        }
    }

    unsafe fn spi_read(&mut self) -> Result<u8, ()> {
        unsafe {
            self.ec.cmd(4)?;
            self.ec.read()
        }
    }

    unsafe fn exit_follow_mode(&mut self) -> Result<(), ()> {
        unsafe { self.ec.cmd(5) }
    }

    unsafe fn spi_wait(&mut self) -> Result<(), ()> {
        unsafe {
            self.enter_follow_mode()?;
            self.spi_cmd(5)?;
            while self.spi_read()? & 1 > 0 {}
            self.exit_follow_mode()
        }
    }

    unsafe fn spi_write_enable(&mut self) -> Result<(), ()> {
        unsafe {
            self.spi_wait()?;
            self.enter_follow_mode()?;
            self.spi_cmd(6)?;
            //TODO: extra spi command 80 based on device id 0xbf
            self.enter_follow_mode()?;
            self.spi_cmd(5)?;
            while self.spi_read()? & 3 != 2 {}
            self.exit_follow_mode()
        }
    }

    unsafe fn spi_write_disable(&mut self) -> Result<(), ()> {
        unsafe {
            self.spi_wait()?;
            self.enter_follow_mode()?;
            self.spi_cmd(4)?;
            self.enter_follow_mode()?;
            self.spi_cmd(5)?;
            while self.spi_read()? & 2 > 0 {}
            self.exit_follow_mode()
        }
    }

    pub unsafe fn start(&mut self) -> Result<u8, ()> {
        unsafe {
            self.ec.cmd(0xDC)?;
            self.ec.read()
        }
    }

    pub unsafe fn read<F: Fn(usize)>(&mut self, callback: F) -> Result<Vec<u8>, ()> {
        let mut buf = Vec::with_capacity(self.size);

        unsafe {
            for sector in 0..self.size / 65536 {
                self.spi_write_disable()?;
                self.spi_wait()?;

                self.enter_follow_mode()?;

                self.spi_cmd(0x0B)?;
                self.spi_write(sector as u8)?;
                self.spi_write(0)?;
                self.spi_write(0)?;
                self.spi_write(0)?;

                for _block in 0..64 {
                    for _ in 0..1024 {
                        buf.push(self.spi_read()?);
                    }
                    callback(buf.len());
                }

                self.spi_wait()?;
            }
        }

        Ok(buf)
    }

    pub unsafe fn erase<F: Fn(usize)>(&mut self, callback: F) -> Result<(), ()> {
        for sector in 0..self.size / 65536 {
            for block in 0..64 {
                let index = sector * 65536 + block * 1024;

                unsafe {
                    self.spi_write_enable()?;
                    self.enter_follow_mode()?;
                    self.spi_cmd(0xD7)?;
                    self.spi_write(sector as u8)?;
                    self.spi_write(block as u8)?;
                    self.spi_write(0)?;
                    self.exit_follow_mode()?;
                    self.spi_wait()?;
                }

                callback(index + 1024);
            }
        }

        Ok(())
    }

    pub unsafe fn write<F: Fn(usize)>(&mut self, buf: &[u8], callback: F) -> Result<(), ()> {
        for sector in 0..self.size / 65536 {
            unsafe {
                self.spi_write_enable()?;

                for block in 0..64 {
                    let index = sector * 65536 + block * 1024;

                    for word in 0..512 {
                        self.enter_follow_mode()?;
                        self.spi_cmd(0xAD)?;
                        if block == 0 && word == 0 {
                            self.spi_write(sector as u8)?;
                            self.spi_write((sector >> 8) as u8)?;
                            self.spi_write((sector >> 16) as u8)?;
                        }
                        self.spi_write(buf.get(index + word * 2).map_or(0xFF, |x| *x))?;
                        self.spi_write(buf.get(index + word * 2 + 1).map_or(0xFF, |x| *x))?;
                        self.spi_wait()?;
                    }

                    callback(index + 1024);
                }

                self.spi_write_disable()?;
                self.spi_wait()?;
            }
        }

        Ok(())
    }

    pub unsafe fn stop(&mut self) -> Result<(), ()> {
        unsafe {
            self.ec.cmd(0x95)?;
            self.ec.cmd(0xFC)
        }
    }
}
