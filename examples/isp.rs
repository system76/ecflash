use hwio::{Io, Pio};
use serialport::{Error, ErrorKind, Result, SerialPortSettings, posix::TTYPort};
use std::any::Any;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;
use std::time::Duration;
use std::thread;

use ecflash::{EcFlash, Flasher};

#[repr(u8)]
pub enum Address {
    CHIPID0 = 0,
    CHIPID1 = 1,
    CHIPVER = 2,
    INDAR0 = 4,
    INDAR1 = 5,
    INDAR2 = 6,
    INDAR3 = 7,
    INDDR = 8,
    ECMSADDR0 = 0x2E,
    ECMSADDR1 = 0x2F,
    ECMSDATA = 0x30,
}

pub trait Debugger {
    /// Set the debugger address
    fn address(&mut self, address: u8) -> Result<()>;
    /// Read data from the debugger port
    fn read(&mut self, data: &mut [u8]) -> Result<usize>;
    /// Write data to the debugger port
    fn write(&mut self, data: &[u8]) -> Result<usize>;

    /// Read data at a debugger address
    fn read_at(&mut self, address: Address, data: &mut [u8]) -> Result<usize> {
        self.address(address as u8)?;
        self.read(data)
    }

    /// Write data at a debugger address
    fn write_at(&mut self, address: Address, data: &[u8]) -> Result<usize> {
        self.address(address as u8)?;
        self.write(data)
    }

    /// Set EC memory snoop address
    fn ecms_address(&mut self, address: u16) -> Result<()> {
        self.write_at(Address::ECMSADDR1, &[(address >> 8) as u8])?;
        self.write_at(Address::ECMSADDR0, &[(address) as u8])?;

        Ok(())
    }

    /// Read data from memory using EC-indirect mode
    fn ecms_read(&mut self, data: &mut [u8]) -> Result<usize> {
        self.read_at(Address::ECMSDATA, data)
    }

    /// Write data to memory using EC memory snoop
    fn ecms_write(&mut self, data: &[u8]) -> Result<usize> {
        self.write_at(Address::ECMSDATA, data)
    }

    /// Read data from memory at address using EC memory snoop
    fn ecms_read_at(&mut self, address: u16, data: &mut [u8]) -> Result<usize> {
        self.ecms_address(address)?;
        self.ecms_read(data)
    }

    /// Write data to memory at address using EC memory snoop
    fn ecms_write_at(&mut self, address: u16, data: &[u8]) -> Result<usize> {
        self.ecms_address(address)?;
        self.ecms_write(data)
    }
}

pub trait Smfi {
    /// Set indar1 register (special case for follow mode)
    fn flash_indar1(&mut self, data: u8) -> Result<()>;

    /// Set EC-indirect flash address
    fn flash_address(&mut self, address: u32) -> Result<()>;

    /// Read data from flash using EC-indirect mode
    fn flash_read(&mut self, data: &mut [u8]) -> Result<usize>;

    /// Write data to flash using EC-indirect mode
    fn flash_write(&mut self, data: &[u8]) -> Result<usize>;

    /// Read data from flash at address using EC-indirect mode
    fn flash_read_at(&mut self, address: u32, data: &mut [u8]) -> Result<usize> {
        self.flash_address(address)?;
        self.flash_read(data)
    }

    /// Write data to flash at address using EC-indirect mode
    fn flash_write_at(&mut self, address: u32, data: &[u8]) -> Result<usize> {
        self.flash_address(address)?;
        self.flash_write(data)
    }
}

impl<T> Smfi for T where T: Debugger {
    /// Set indar1 register (special case for follow mode)
    fn flash_indar1(&mut self, data: u8) -> Result<()> {
        self.write_at(Address::INDAR1, &[data])?;
        Ok(())
    }

    /// Set EC-indirect flash address
    fn flash_address(&mut self, address: u32) -> Result<()> {
        self.write_at(Address::INDAR3, &[(address >> 24) as u8])?;
        self.write_at(Address::INDAR2, &[(address >> 16) as u8])?;
        self.write_at(Address::INDAR1, &[(address >> 8) as u8])?;
        self.write_at(Address::INDAR0, &[(address) as u8])?;
        Ok(())
    }

    /// Read data from flash using EC-indirect mode
    fn flash_read(&mut self, data: &mut [u8]) -> Result<usize> {
        self.read_at(Address::INDDR, data)
    }

    /// Write data to flash using EC-indirect mode
    fn flash_write(&mut self, data: &[u8]) -> Result<usize> {
        self.write_at(Address::INDDR, data)
    }
}

pub struct SpiBus<'a, T: Smfi> {
    port: &'a mut T,
    data: bool,
}

impl<'a, T: Smfi> SpiBus<'a, T> {
    pub fn new(port: &'a mut T, eflash: bool) -> Result<Self> {
        port.flash_address(
            if eflash { 0x7FFF_FE00 } else { 0xFFFF_FE00 },
        )?;

        let mut spi = Self { port, data: false };
        spi.reset()?;
        Ok(spi)
    }

    /// Disable SPI chip - should be done before and after each transaction
    pub fn reset(&mut self) -> Result<()> {
        if self.data {
            self.port.flash_indar1(0xFE)?;
            self.data = false;
        }
        self.port.flash_write(&[0])?;
        Ok(())
    }

    /// Read from SPI chip directly using follow mode
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        if !self.data {
            self.port.flash_indar1(0xFD)?;
            self.data = true;
        }
        self.port.flash_read(data)
    }

    /// Write to SPI chip directly using follow mode
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        if !self.data {
            self.port.flash_indar1(0xFD)?;
            self.data = true;
        }
        self.port.flash_write(data)
    }
}

impl<'a, T: Smfi> Drop for SpiBus<'a, T> {
    fn drop(&mut self) {
        let _ = self.reset();
    }
}

struct SpiRom<'a, 't, T: Smfi> {
    bus: &'a mut SpiBus<'t, T>,
}

impl<'a, 't, T: Smfi> SpiRom<'a, 't, T> {
    pub fn new(bus: &'a mut SpiBus<'t, T>) -> Self {
        Self { bus }
    }

    pub fn status(&mut self) -> Result<u8> {
        let mut status = [0];

        self.bus.reset()?;
        self.bus.write(&[0x05])?;
        self.bus.read(&mut status)?;

        Ok(status[0])
    }

    pub fn write_disable(&mut self) -> Result<()> {
        self.bus.reset()?;
        self.bus.write(&[0x04])?;

        // Poll status for busy and write enable flags
        //TODO: timeout
        while self.status()? & 3 != 0 {}

        Ok(())
    }

    pub fn write_enable(&mut self) -> Result<()> {
        self.bus.reset()?;
        self.bus.write(&[0x06])?;

        // Poll status for busy and write enable flags
        //TODO: timeout
        while self.status()? & 3 != 2 {}

        Ok(())
    }

    pub fn erase_chip(&mut self) -> Result<()> {
        self.write_enable()?;

        self.bus.reset()?;
        self.bus.write(&[0x60])?;

        // Poll status for busy flag
        //TODO: timeout
        while self.status()? & 1 != 0 {}

        self.write_disable()?;

        Ok(())
    }

    pub fn erase_sector(&mut self, address: u32) -> Result<usize> {
        if (address & 0xFF00_0000) > 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("address {:X} exceeds 24 bits", address)
            ));
        }

        self.write_enable()?;

        self.bus.reset()?;
        self.bus.write(&[
            0xD7,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
        ])?;

        // Poll status for busy flag
        //TODO: timeout
        while self.status()? & 1 != 0 {}

        self.write_disable()?;

        Ok(1024)
    }

    pub fn read_at(&mut self, address: u32, data: &mut [u8]) -> Result<usize> {
        if (address & 0xFF00_0000) > 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("address {:X} exceeds 24 bits", address)
            ));
        }

        self.bus.reset()?;
        self.bus.write(&[
            0x0B,
            (address >> 16) as u8,
            (address >> 8) as u8,
            address as u8,
            0,
        ])?;
        self.bus.read(data)
    }

    pub fn write_at(&mut self, address: u32, data: &[u8]) -> Result<usize> {
        if (address & 0xFF00_0000) > 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("address {:X} exceeds 24 bits", address)
            ));
        }

        //TODO: Support programming with any length
        if (data.len() % 2) != 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("length {} is not a multiple of 2", data.len())
            ));
        }

        self.write_enable()?;

        for (i, word) in data.chunks_exact(2).enumerate() {
            self.bus.reset()?;
            if i == 0 {
                self.bus.write(&[
                    0xAD,
                    (address >> 16) as u8,
                    (address >> 8) as u8,
                    address as u8,
                    word[0],
                    word[1]
                ])?;
            } else {
                self.bus.write(&[
                    0xAD,
                    word[0],
                    word[1]
                ])?;
            }

            // Poll status for busy flag
            //TODO: timeout
            while self.status()? & 1 != 0 {}
        }

        self.write_disable()?;

        Ok(data.len())
    }
}

impl<'a, 't, T: Smfi> Drop for SpiRom<'a, 't, T> {
    fn drop(&mut self) {
        let _ = self.write_disable();
    }
}

pub struct ParallelArduino {
    tty: TTYPort,
    buffer_size: usize,
}

impl ParallelArduino {
    /// Connect to parallel port arduino using provided port
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let tty = TTYPort::open(path.as_ref(), &SerialPortSettings {
            baud_rate: 1000000,
            data_bits: serialport::DataBits::Eight,
            flow_control: serialport::FlowControl::None,
            parity: serialport::Parity::None,
            stop_bits: serialport::StopBits::One,
            timeout: Duration::new(1, 0),
        })?;

        let mut port = Self { tty, buffer_size: 0 };
        // Wait until programmer is ready
        thread::sleep(Duration::new(1, 0));
        // Check that programmer is ready
        port.echo()?;
        // Read buffer size
        port.update_buffer_size()?;

        Ok(port)
    }

    fn echo(&mut self) -> Result<()> {
        self.tty.write_all(&[
            b'E',
            0,
            0x76,
        ])?;

        let mut b = [0];
        self.tty.read_exact(&mut b)?;
        if b[0] != 0x76 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("received echo of {:02X} instead of {:02X}", b[0], 0x76)
            ));
        }
        Ok(())
    }

    fn update_buffer_size(&mut self) -> Result<()> {
        self.tty.write_all(&[
            b'B',
            0,
        ])?;

        let mut b = [0; 1];
        self.tty.read_exact(&mut b)?;
        // Size is recieved data + 1
        self.buffer_size = (b[0] as usize) + 1;

        eprintln!("Buffer size: {}", self.buffer_size);
        Ok(())
    }
}

impl Debugger for ParallelArduino {
    fn address(&mut self, address: u8) -> Result<()> {
        self.tty.write_all(&[
            b'A',
            address,
        ])?;

        Ok(())
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        for chunk in data.chunks_mut(self.buffer_size) {
            let param = (chunk.len() - 1) as u8;
            self.tty.write_all(&[
                b'R',
                param,
            ])?;
            self.tty.read_exact(chunk)?;
        }

        Ok(data.len())
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        for chunk in data.chunks(self.buffer_size) {
            let param = (chunk.len() - 1) as u8;
            self.tty.write_all(&[
                b'W',
                param,
            ])?;
            self.tty.write_all(chunk)?;

            let mut b = [0];
            self.tty.read_exact(&mut b)?;
            if b[0] != param {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("received ack of {:02X} instead of {:02X}", b[0], param)
                ));
            }
        }

        Ok(data.len())
    }
}

pub struct I2EC {
    address: Pio<u8>,
    data: Pio<u8>,
}

impl I2EC {
    pub fn new() -> Result<Self> {
        //TODO: check EC ID using super i/o
        if unsafe { libc::iopl(3) } != 0 {
            return Err(Error::from(
                io::Error::last_os_error()
            ));
        }

        Ok(Self {
            address: Pio::new(0x2E),
            data: Pio::new(0x2F),
        })
    }

    fn super_io_read(&mut self, reg: u8) -> u8 {
        self.address.write(reg);
        self.data.read()
    }

    fn super_io_write(&mut self, reg: u8, value: u8) {
        self.address.write(reg);
        self.data.write(value);
    }

    fn d2_read(&mut self, reg: u8) -> u8 {
        self.super_io_write(0x2E, reg);
        self.super_io_read(0x2F)
    }

    fn d2_write(&mut self, reg: u8, value: u8) {
        self.super_io_write(0x2E, reg);
        self.super_io_write(0x2F, value);
    }

    fn i2ec_read(&mut self, addr: u16) -> u8 {
        self.d2_write(0x11, (addr >> 8) as u8);
        self.d2_write(0x10, addr as u8);
        self.d2_read(0x12)
    }

    fn i2ec_write(&mut self, addr: u16, value: u8) {
        self.d2_write(0x11, (addr >> 8) as u8);
        self.d2_write(0x10, addr as u8);
        self.d2_write(0x12, value);
    }
}

impl Smfi for I2EC {
    /// Set indar1 register (special case for follow mode)
    fn flash_indar1(&mut self, data: u8) -> Result<()> {
        self.i2ec_write(0x103C, data);
        Ok(())
    }

    /// Set EC-indirect flash address
    fn flash_address(&mut self, address: u32) -> Result<()> {
        self.i2ec_write(0x103E, (address >> 24) as u8);
        self.i2ec_write(0x103D, (address >> 16) as u8);
        self.i2ec_write(0x103C, (address >> 8) as u8);
        self.i2ec_write(0x103B, (address) as u8);
        Ok(())
    }

    /// Read data from flash using EC-indirect mode
    fn flash_read(&mut self, data: &mut [u8]) -> Result<usize> {
        for b in data.iter_mut() {
            *b = self.i2ec_read(0x103F);
        }
        Ok(data.len())
    }

    /// Write data to flash using EC-indirect mode
    fn flash_write(&mut self, data: &[u8]) -> Result<usize> {
        for b in data.iter() {
            self.i2ec_write(0x103F, *b);
        }
        Ok(data.len())
    }
}

fn isp_inner<T: Any + Smfi>(mut port: T, firmware: &[u8]) -> Result<()> {
    let rom_size = 128 * 1024;

    let mut spi_bus = SpiBus::new(&mut port, true)?;
    let mut spi = SpiRom::new(&mut spi_bus);

    let mut rom = vec![0; rom_size];
    {
        // Read entire ROM
        eprintln!("SPI read");
        spi.read_at(0, &mut rom)?;
    }

    eprintln!("Saving ROM to backup.rom");
    fs::write("backup.rom", &rom)?;

    let mut matches = true;
    for i in 0..rom.len() {
        if &rom[i] != firmware.get(i).unwrap_or(&0xFF) {
            matches = false;
            break;
        }
    }

    if matches {
        eprintln!("ROM matches specified firmware");
        return Ok(());
    }

    {
        // Chip erase
        // eprintln!("SPI chip erase");
        // spi.erase_chip()?;

        // Sector erase
        let mut address = 0;
        while address < rom_size {
            let mut erased = true;
            for &b in &rom[address..address + 1024] {
                if b != 0xFF {
                    erased =false;
                    break;
                }
            }

            if erased {
                eprintln!("SPI sector already erased {:06X}", address);
                address += 1024;
            } else {
                eprintln!("SPI sector erase {:06X}", address);
                address += spi.erase_sector(address as u32)?;
            }
        }

        // Read entire ROM
        eprintln!("SPI read");
        spi.read_at(0, &mut rom)?;
    }

    // Verify chip erase
    for i in 0..rom.len() {
        if rom[i] != 0xFF {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Failed to erase: {:X} is {:X} instead of {:X}", i, rom[i], 0xFF)
            ));
        }
    }

    //TODO: Set write disable on error
    // Program
    {
        // Auto address increment word program
        if (spi.bus.port as &mut dyn Any).is::<ParallelArduino>() {
            spi.write_enable()?;

            {
                eprintln!("SPI AAI word program (accelerated)");
                let port = (spi.bus.port as &mut dyn Any).downcast_mut::<ParallelArduino>().unwrap();
                for (i, chunk) in firmware.chunks(port.buffer_size).enumerate() {
                    eprint!("  program {} / {}\r", i * port.buffer_size, firmware.len());

                    let param = (chunk.len() - 1) as u8;
                    port.tty.write_all(&[
                        b'P',
                        param
                    ])?;
                    port.tty.write_all(chunk)?;

                    let mut b = [0];
                    port.tty.read_exact(&mut b)?;
                    if b[0] != param {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            format!("received ack of {:02X} instead of {:02X}", b[0], param)
                        ));
                    }
                }
                eprintln!("  program {} / {}", firmware.len(), firmware.len());
            }

            spi.write_disable()?;
        } else {
            eprintln!("SPI AAI word program");
            spi.write_at(0, &firmware)?;
        }


        // Read entire ROM
        eprintln!("SPI read");
        spi.read_at(0, &mut rom)?;
    }

    // Verify program
    for i in 0..rom.len() {
        if &rom[i] != firmware.get(i).unwrap_or(&0xFF) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Failed to program: {:X} is {:X} instead of {:X}", i, rom[i], firmware[i])
            ));
        }
    }

    eprintln!("Successfully programmed SPI ROM");

    Ok(())
}

fn isp(internal: bool, file: &str) -> Result<()> {
    // Read firmware data
    let firmware = {
        let mut firmware = fs::read(file)?;

        // Truncate 0xFF bytes
        while firmware.last() == Some(&0xFF) {
            firmware.pop();
        }

        // Make sure firmware length is a multiple of word size
        while firmware.len() % 2 != 0 {
            firmware.push(0xFF);
        }

        firmware
    };

    if internal {
        unsafe {
            if libc::iopl(3) < 0 {
                eprintln!("Failed to get I/O permission: {}", io::Error::last_os_error());
                process::exit(1);
            }

            //TODO: return error
            let ec = EcFlash::new(true).expect("Failed to find EC");
            let mut flasher = Flasher::new(ec);
            if flasher.start() == Ok(51) {
                let res = isp_inner(I2EC::new()?, &firmware);

                eprintln!("Sync");
                let _ = process::Command::new("sync").status();

                eprintln!("System will shut off in 5 seconds");
                thread::sleep(Duration::new(5, 0));

                eprintln!("Sync");
                let _ = process::Command::new("sync").status();

                // Will currently power off system
                let _ = flasher.stop();

                match res {
                    Ok(()) => {
                        eprintln!("Successfully flashed EC");

                        // Shut down
                        process::Command::new("shutdown")
                            .status()
                            .expect("failed to run shutdown");

                        Ok(())
                    },
                    Err(err) => {
                        eprintln!("Failed to flash EC: {}", err);
                        Err(err)
                    }
                }
            } else {
                //TODO: return error
                panic!("Failed to start flasher")
            }
        }
    } else {
        // Open arduino console
        let mut port = ParallelArduino::new("/dev/ttyACM0")?;

        // Read ID
        let mut id = [0; 3];
        port.address(0)?;
        port.read(&mut id[0..1])?;
        port.address(1)?;
        port.read(&mut id[1..2])?;
        port.address(2)?;
        port.read(&mut id[2..3])?;

        eprintln!("ID: {:02X}{:02X} VER: {}", id[0], id[1], id[2]);

        isp_inner(port, &firmware)
    }
}

fn main() {
    let mut file_opt = None;
    let mut internal = false;
    for arg in env::args().skip(1) {
        if arg == "--internal" {
            internal = true;
        } else {
            file_opt = Some(arg);
        }
    }
    //TODO: better errors
    let file = file_opt.expect("no firmware file provided");
    isp(internal, &file).expect("failed to flash");
}
