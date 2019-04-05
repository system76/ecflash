extern crate ecflash;

use ecflash::{EcFlash, Flasher};
use std::{fs, io, process};

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

        let ec = EcFlash::new(true).expect("Failed to find EC");

        let mut flasher = Flasher::new(ec);

        if flasher.start() == Ok(51) {
            if let Ok(data) = flasher.read(|x| { eprint!("\r{} KB", x / 1024) }) {
                eprintln!("");
                let _ = fs::write("read.rom", data);
            } else {
                eprintln!("Failed to read data");
            }

            flasher.stop();
        } else {
            eprintln!("Failed to start flasher");
        }
    }
}
