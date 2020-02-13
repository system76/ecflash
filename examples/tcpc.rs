extern crate ecflash;

use ecflash::EcFlash;
use std::{io, process};

fn tcpc_read(ec: &mut EcFlash, command: u8) -> Result<u16, ()> {
    let mut buf = [
        0x2c,
        command,
        0x00,
        0x00
    ];

    unsafe {
        ec.fcommand(
            0x76,
            0x10,
            &mut buf
        )?;
    }

    Ok(
        (buf[2] as u16) |
        (buf[3] as u16) << 8
    )
}

fn tcpc_test() -> Result<(), ()> {
        let mut ec = EcFlash::new(true).map_err(|_| ())?;

        let mut i = 0;
        while i < 256 {
            if i % 16 == 0 {
                if i == 0 {
                    print!("   ");
                    for j in 0 .. 16 {
                        print!(" _{:01X}", j);
                    }
                }
                println!();
                print!("{:02X}:", i);
            }

            let word = tcpc_read(&mut ec, i as u8)?;

            print!(" {:02X}", word as u8);
            print!(" {:02X}", (word >> 8) as u8);

            i += 2;
        }
        println!();

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

    }

    tcpc_test().expect("Failed to run TCPM test");
}
