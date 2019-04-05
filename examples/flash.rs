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

        let data = fs::read("flash.rom").expect("Failed to open flash.rom");

        let mut flasher = Flasher::new(ec);

        if flasher.start() == Ok(51) {
            if let Ok(original) = flasher.read(|x| eprint!("\rRead: {} KB", x / 1024)) {
                eprintln!("");

                let _ = fs::write("original.rom", &original);

                if flasher.erase(|x| eprint!("\rErase: {} KB", x / 1024)).is_ok() {
                    eprintln!("");

                    if let Ok(erased) = flasher.read(|x| eprint!("\rRead: {} KB", x / 1024)) {
                        eprintln!("");

                        let _ = fs::write("erased.rom", &erased);

                        if flasher.write(&data, |x| eprint!("\rWrite {} KB", x / 1024)).is_ok() {
                            eprintln!("");

                            if let Ok(written) = flasher.read(|x| eprint!("\rRead: {} KB", x / 1024)) {
                                eprintln!("");

                                let _ = fs::write("written.rom", &written);
                            } else {
                                eprintln!("Failed to read written data");
                            }
                        } else {
                            eprintln!("Failed to write data");
                        }
                    } else {
                        eprintln!("Failed to read erased data");
                    }
                } else {
                    eprintln!("Failed to erase data");
                }
            } else {
                eprintln!("Failed to read original data");
            }

            flasher.stop();
        } else {
            eprintln!("Failed to start flasher");
        }
    }
}
