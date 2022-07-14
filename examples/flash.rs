#![allow(clippy::needless_range_loop)]

extern crate ecflash;

use ecflash::{EcFlash, Flasher};
use std::{env, fs, io, process, thread, time};

fn main() {
    extern "C" {
        fn iopl(level: isize) -> isize;
    }

    let path = env::args().nth(1).expect("no path argument");

    let mut data = fs::read(path).expect("Failed to open rom");

    // Wait for any key releases
    eprintln!("Waiting for all keys to be released");
    thread::sleep(time::Duration::new(1, 0));

    eprintln!("Sync");
    process::Command::new("sync")
        .status()
        .expect("failed to run sync");

    // Get I/O Permission
    unsafe {
        if iopl(3) < 0 {
            eprintln!(
                "Failed to get I/O permission: {}",
                io::Error::last_os_error()
            );
            process::exit(1);
        }

        let ec = EcFlash::new(true).expect("Failed to find EC");

        let mut flasher = Flasher::new(ec);

        while data.len() < flasher.size {
            data.push(0xFF);
        }

        if flasher.start() == Ok(51) {
            let mut success = false;

            if let Ok(_original) = flasher.read(|x| eprint!("\rRead: {} KB", x / 1024)) {
                eprintln!();

                if flasher
                    .erase(|x| eprint!("\rErase: {} KB", x / 1024))
                    .is_ok()
                {
                    eprintln!();

                    if let Ok(erased) = flasher.read(|x| eprint!("\rRead: {} KB", x / 1024)) {
                        eprintln!();

                        //TODO: retry erase on fail
                        for i in 0..erased.len() {
                            if erased[i] != 0xFF {
                                println!("0x{:X}: 0x{:02X} != 0xFF", i, erased[i]);
                            }
                        }

                        if flasher
                            .write(&data, |x| eprint!("\rWrite {} KB", x / 1024))
                            .is_ok()
                        {
                            eprintln!();

                            if let Ok(written) =
                                flasher.read(|x| eprint!("\rRead: {} KB", x / 1024))
                            {
                                eprintln!();

                                success = true;
                                for i in 0..written.len() {
                                    if written[i] != data[i] {
                                        println!(
                                            "0x{:X}: 0x{:02X} != 0x{:02X}",
                                            i, written[i], data[i]
                                        );
                                        success = false;
                                    }
                                }
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

            eprintln!("Sync");
            process::Command::new("sync")
                .status()
                .expect("failed to run sync");

            // Will currently power off system
            let _ = flasher.stop();

            if success {
                eprintln!("Successfully flashed EC");

                // Shut down
                process::Command::new("shutdown")
                    .status()
                    .expect("failed to run shutdown");
            } else {
                eprintln!("Failed to flash EC");
            }
        } else {
            eprintln!("Failed to start flasher");
        }
    }
}
