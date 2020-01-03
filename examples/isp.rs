use serialport::{Error, ErrorKind, Result, SerialPortSettings, posix::TTYPort};
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::thread;

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

    /// Set EC-indirect flash address
    fn flash_address(&mut self, address: u32) -> Result<()> {
        self.write_at(Address::INDAR3, &[(address >> 24) as u8])?;
        self.write_at(Address::INDAR2, &[(address >> 16) as u8])?;
        self.write_at(Address::INDAR1, &[(address >> 8) as u8])?;
        self.write_at(Address::INDAR0, &[(address) as u8])?;

        Ok(())
    }

    /// Read data from flash using EC-indirect mode
    fn flash_read(&mut self, address: u32, data: &mut [u8]) -> Result<usize> {
        self.flash_address(address)?;
        self.read_at(Address::INDDR, data)
    }

    /// Write data to flash using EC-indirect mode
    fn flash_write(&mut self, address: u32, data: &[u8]) -> Result<usize> {
        self.flash_address(address)?;
        self.write_at(Address::INDDR, data)
    }
}

pub struct SpiFollow<'a, T: Debugger> {
    port: &'a mut T,
    eflash: bool,
}

impl<'a, T: Debugger> SpiFollow<'a, T> {
    pub fn new(port: &'a mut T, eflash: bool) -> Result<Self> {
        let mut spi = Self { port, eflash };
        spi.reset()?;
        Ok(spi)
    }

    /// Disable SPI chip - should be done before and after each transaction
    pub fn reset(&mut self) -> Result<()> {
        self.port.flash_write(
            if self.eflash { 0x7FFF_FE00 } else { 0xFFFF_FE00 },
            &[0]
        )?;
        Ok(())
    }

    /// Read from SPI chip directly using follow mode
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        self.port.flash_read(
            if self.eflash { 0x7FFF_FD00 } else { 0xFFFF_FD00 },
            data
        )
    }

    /// Write to SPI chip directly using follow mode
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.port.flash_write(
            if self.eflash { 0x7FFF_FD00 } else { 0xFFFF_FD00 },
            data
        )
    }
}

impl<'a, T: Debugger> Drop for SpiFollow<'a, T> {
    fn drop(&mut self) {
        let _ = self.reset();
    }
}

pub struct ParallelArduino {
    tty: TTYPort,
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
            timeout: Duration::new(5, 0),
        })?;

        let mut port = Self { tty };
        // Wait until programmer is ready
        thread::sleep(Duration::new(1, 0));
        // Check that programmer is ready
        port.echo()?;

        Ok(port)
    }

    fn ack(&mut self) -> Result<()> {
        let mut b = [0];
        self.tty.read_exact(&mut b)?;
        if b[0] == b'\r' {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("received ack of {:02X} instead of {:02X}", b[0], b'K')
            ))
        }
    }

    fn echo(&mut self) -> Result<()> {
        self.tty.write_all(&[b'E', 0, 0x76])?;
        let mut b = [0];
        self.tty.read_exact(&mut b)?;
        if b[0] != 0x76 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("received echo of {:02X} instead of {:02X}", b[0], 0x76)
            ));
        }
        self.ack()?;
        Ok(())
    }
}

impl Debugger for ParallelArduino {
    fn address(&mut self, address: u8) -> Result<()> {
        self.tty.write_all(&[b'A', 0, address])?;
        self.ack()?;
        Ok(())
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        for chunk in data.chunks_mut(256) {
            let length = chunk.len().checked_sub(1).unwrap();
            self.tty.write_all(&[b'R', length as u8])?;
            self.tty.read_exact(chunk)?;
            self.ack()?;
        }
        Ok(data.len())
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        for chunk in data.chunks(256) {
            let length = chunk.len().checked_sub(1).unwrap();
            self.tty.write_all(&[b'W', length as u8])?;
            self.tty.write_all(chunk)?;
            self.ack()?;
        }
        Ok(data.len())
    }
}

fn isp() -> Result<()> {
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

    println!("ID: {:02X}{:02X} VER: {}", id[0], id[1], id[2]);

    // Read entire ROM
    {
        let mut spi = SpiFollow::new(&mut port, true)?;

        let mut buf = vec![0; 128 * 1024];
        spi.write(&[0x0B, 0x00, 0x00, 0x00, 0x00])?;
        spi.read(&mut buf)?;
        // Print signature
        for i in 0x50..0x60 {
            println!("{:02X}", buf[i]);
        }
    }

    Ok(())
}

fn main() {
    isp().unwrap();
}
