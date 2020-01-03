use serialport::{Error, ErrorKind, Result, SerialPortSettings, posix::TTYPort};
use std::env;
use std::fs;
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
    fn flash_read(&mut self, data: &mut [u8]) -> Result<usize> {
        self.read_at(Address::INDDR, data)
    }

    /// Write data to flash using EC-indirect mode
    fn flash_write(&mut self, data: &[u8]) -> Result<usize> {
        self.write_at(Address::INDDR, data)
    }

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

pub struct SpiFollow<'a, T: Debugger> {
    port: &'a mut T,
    data: bool,
}

impl<'a, T: Debugger> SpiFollow<'a, T> {
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
            self.port.write_at(Address::INDAR1, &[0xFE])?;
            self.data = false;
        }
        self.port.flash_write(&[0])?;
        Ok(())
    }

    /// Read from SPI chip directly using follow mode
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        if !self.data {
            self.port.write_at(Address::INDAR1, &[0xFD])?;
            self.data = true;
        }
        self.port.flash_read(data)
    }

    /// Write to SPI chip directly using follow mode
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        if !self.data {
            self.port.write_at(Address::INDAR1, &[0xFD])?;
            self.data = true;
        }
        self.port.flash_write(data)
    }
}

impl<'a, T: Debugger> Drop for SpiFollow<'a, T> {
    fn drop(&mut self) {
        println!("Drop");
        let _ = self.reset();
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

    fn ack(&mut self) -> Result<()> {
        println!("ACK");
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
        println!("E,0,0,76");
        self.tty.write_all(&[
            b'E',
            0,
            0,
            0x76,
        ])?;
        println!("Echo");
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

    fn update_buffer_size(&mut self) -> Result<()> {
        println!("B,1,0");
        self.tty.write_all(&[
            b'B',
            1,
            0,
        ])?;
        let mut b = [0; 2];
        self.tty.read_exact(&mut b)?;
        // Size is recieved data + 1
        self.buffer_size = (
            (b[0] as usize) |
            ((b[1] as usize) << 8)
        ) + 1;
        self.ack()?;
        println!("Buffer size: {}", self.buffer_size);
        Ok(())
    }
}

impl Debugger for ParallelArduino {
    fn address(&mut self, address: u8) -> Result<()> {
        println!("A,0,{:X}", address);
        self.tty.write_all(&[
            b'A',
            0,
            0,
            address,
        ])?;
        self.ack()?;
        Ok(())
    }

    fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        for chunk in data.chunks_mut(self.buffer_size) {
            let length = chunk.len().checked_sub(1).unwrap();
            println!("R,{:X}", length);
            self.tty.write_all(&[
                b'R',
                length as u8,
                (length >> 8) as u8,
            ])?;
            println!("Read");
            self.tty.read_exact(chunk)?;
            self.ack()?;
        }
        Ok(data.len())
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        for chunk in data.chunks(self.buffer_size) {
            let length = chunk.len().checked_sub(1).unwrap();
            println!("W,{:X}", length);
            self.tty.write_all(&[
                b'W',
                length as u8,
                (length >> 8) as u8,
            ])?;
            println!("Write");
            self.tty.write_all(chunk)?;
            self.ack()?;
        }
        Ok(data.len())
    }
}

fn isp(file: &str) -> Result<()> {
    let rom_size = 128 * 1024;

    // Read firmware data
    let firmware = {
        let mut firmware = fs::read(file)?;
        // Make sure firmware length is a multiple of word size
        while firmware.len() % 2 != 0 {
            firmware.push(0xFF);
        }
        firmware
    };

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
    let mut rom = vec![0; rom_size];
    {
        println!("SPI read");
        let mut spi = SpiFollow::new(&mut port, true)?;
        spi.write(&[0x0B, 0x00, 0x00, 0x00, 0x00])?;
        spi.read(&mut rom)?;
    }

    println!("Saving ROM to backup.rom");
    fs::write("backup.rom", &rom)?;

    let mut matches = true;
    for i in 0..rom.len() {
        if &rom[i] != firmware.get(i).unwrap_or(&0xFF) {
            matches = false;
            break;
        }
    }

    if matches {
        println!("ROM matches specified firmware");
        return;
    }

    //TODO: Set write disable on error
    // Chip erase
    {
        // Write enable
        println!("SPI write enable");
        let mut spi = SpiFollow::new(&mut port, true)?;
        spi.write(&[0x06])?;

        // Poll status for busy and write enable flags
        println!("SPI write enable wait");
        loop {
            let mut status = [0];
            spi.reset()?;
            spi.write(&[0x05])?;
            spi.read(&mut status)?;

            if status[0] & 3 == 2 {
                break;
            }
        }

        // Chip erase
        println!("SPI chip erase");
        spi.reset()?;
        spi.write(&[0x60])?;
        
        // Poll status for busy flag
        println!("SPI busy wait");
        loop {
            let mut status = [0];
            spi.reset()?;
            spi.write(&[0x05])?;
            spi.read(&mut status)?;

            if status[0] & 1 == 0 {
                break;
            }
        }

        // Write disable
        println!("SPI write disable");
        spi.reset()?;
        spi.write(&[0x04])?;

        // Poll status for busy and write enable flags
        println!("SPI write disable wait");
        loop {
            let mut status = [0];
            spi.reset()?;
            spi.write(&[0x05])?;
            spi.read(&mut status)?;

            if status[0] & 3 == 0 {
                break;
            }
        }

        // Read entire ROM
        println!("SPI read");
        spi.reset()?;
        spi.write(&[0x0B, 0x00, 0x00, 0x00, 0x00])?;
        spi.read(&mut rom)?;
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
        // Write enable
        println!("SPI write enable");
        let mut spi = SpiFollow::new(&mut port, true)?;
        spi.write(&[0x06])?;

        // Poll status for busy and write enable flags
        println!("SPI write enable wait");
        loop {
            let mut status = [0];
            spi.reset()?;
            spi.write(&[0x05])?;
            spi.read(&mut status)?;

            if status[0] & 3 == 2 {
                break;
            }
        }

        // Auto address increment word program
        println!("SPI AAI word program");
        for (i, word) in firmware.chunks_exact(2).enumerate() {
            println!("  program {} / {}", i * 2, firmware.len());
            spi.reset()?;
            if i == 0 {
                // Write address on first cycle
                spi.write(&[0xAD, 0, 0, 0, word[0], word[1]])?;
            } else {
                spi.write(&[0xAD, word[0], word[1]])?;
            }

            // Poll status for busy flag
            println!("SPI busy wait");
            loop {
                let mut status = [0];
                spi.reset()?;
                spi.write(&[0x05])?;
                spi.read(&mut status)?;

                if status[0] & 1 == 0 {
                    break;
                }
            }
        }
        

        // Write disable
        println!("SPI write disable");
        spi.reset()?;
        spi.write(&[0x04])?;

        // Poll status for busy and write enable flags
        println!("SPI write disable wait");
        loop {
            let mut status = [0];
            spi.reset()?;
            spi.write(&[0x05])?;
            spi.read(&mut status)?;

            if status[0] & 3 == 0 {
                break;
            }
        }

        // Read entire ROM
        println!("SPI read");
        spi.reset()?;
        spi.write(&[0x0B, 0x00, 0x00, 0x00, 0x00])?;
        spi.read(&mut rom)?;
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

    Ok(())
}

fn main() {
    //TODO: better errors
    let file = env::args().nth(1).expect("no firmware file provided");
    isp(&file).expect("failed to flash");
}
