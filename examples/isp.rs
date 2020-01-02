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

pub trait Parallel {
    /// Set the parallel address
    fn address(&mut self, address: u8) -> Result<()>;
    /// Read data from the parallel port
    fn read(&mut self, data: &mut [u8]) -> Result<usize>;
    /// Write data to the parallel port
    fn write(&mut self, data: &[u8]) -> Result<usize>;

    /// Read data at a parallel address
    fn read_at(&mut self, address: Address, data: &mut [u8]) -> Result<usize> {
        self.address(address as u8)?;
        self.read(data)
    }

    /// Write data at a parallel address
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

    /// Disable SPI chip - should be done before and after each transaction
    fn spi_reset(&mut self, eflash: bool) -> Result<()> {
        self.flash_write(
            if eflash { 0x7FFF_FE00 } else { 0xFFFF_FE00 },
            &[0]
        )?;
        Ok(())
    }

    /// Read from SPI chip directly using follow mode
    fn spi_read(&mut self, eflash: bool, data: &mut [u8]) -> Result<usize> {
        self.flash_read(
            if eflash { 0x7FFF_FD00 } else { 0xFFFF_FD00 },
            data
        )
    }

    /// Write to SPI chip directly using follow mode
    fn spi_write(&mut self, eflash: bool, data: &[u8]) -> Result<usize> {
        self.flash_write(
            if eflash { 0x7FFF_FD00 } else { 0xFFFF_FD00 },
            data
        )
    }
}

pub struct ParallelArduino {
    tty: TTYPort,
}

impl ParallelArduino {
    /// Connect to parallel port arduino using provided port
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        //TODO: do not unwrap
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
        self.tty.read(&mut b)?;
        if b[0] == b'\r' {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!("received {:02X} instead of {:02X}", b[0], b'K')
            ))
        }
    }

    fn echo(&mut self) -> Result<()> {
        self.tty.write(&[b'E'])?;
        self.ack()?;
        Ok(())
    }
}

impl Parallel for ParallelArduino {
    fn address(&mut self, address: u8) -> Result<()> {
        self.tty.write(&[b'A', address])?;
        self.ack()?;
        Ok(())
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        let mut i = 0;
        while i < data.len() {
            self.tty.write(&[b'R'])?;
            self.tty.read(&mut data[i..i+1])?;
            self.ack()?;
            i+=1;
        }
        Ok(i)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        let mut i = 0;
        while i < data.len() {
            self.tty.write(&[b'W', data[i]])?;
            self.ack()?;
            i+=1;
        }
        Ok(i)
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

    // Read signature
    let mut buf = vec![0; 1024];
    port.spi_reset(true)?;
    port.spi_write(true, &[0x0B, 0x00, 0x00, 0x00, 0x00])?;
    port.spi_read(true, &mut buf)?;
    for i in 0x50..0x60 {
        println!("{:02X}", buf[i]);
    }
    port.spi_reset(true)?;

    Ok(())
}

fn main() {
    isp().unwrap();
}
