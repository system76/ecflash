extern crate ecflash;

use std::fmt::Display;
use std::fs::File;
use std::io::{stderr, stdout, BufWriter, Error, Read, Write};
use std::{env, process};

use ecflash::{Ec, EcFile, EcFlash};

fn validate<T: PartialEq + Display, F: FnMut() -> T>(mut f: F, attempts: usize) -> Result<T, ()> {
    for _attempt_i in 0..attempts {
        let a = f();
        let b = f();
        if a == b {
            return Ok(a);
        } else {
            let _ = writeln!(stderr(), "Attempt {}: {} != {}", _attempt_i, a, b);
        }
    }
    Err(())
}

fn main() {
    extern "C" {
        fn iopl(level: isize) -> isize;
    }

    // Get I/O Permission
    unsafe {
        if iopl(3) < 0 {
            let _ = writeln!(
                stderr(),
                "Failed to get I/O permission: {}",
                Error::last_os_error()
            );
            process::exit(1);
        }
    }

    let mut ecs: Vec<(String, Box<dyn Ec>)> = Vec::new();

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "-1" => match EcFlash::new(true) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                }
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 1: {}", err);
                    process::exit(1);
                }
            },
            "-2" => match EcFlash::new(false) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                }
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 2: {}", err);
                    process::exit(1);
                }
            },
            _ => match File::open(&arg) {
                Ok(mut ec_file) => {
                    let mut data = Vec::new();
                    match ec_file.read_to_end(&mut data) {
                        Ok(_) => ecs.push((arg, Box::new(EcFile::new(data)))),
                        Err(err) => {
                            let _ = writeln!(stderr(), "Failed to read EC file '{}': {}", arg, err);
                            process::exit(1);
                        }
                    }
                }
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC file '{}': {}", arg, err);
                    process::exit(1);
                }
            },
        }
    }

    let mut stdout = BufWriter::new(stdout());

    for (name, mut ec) in ecs {
        if name.is_empty() {
            let _ = writeln!(stdout, "EC Flash");
        } else {
            let _ = writeln!(stdout, "EC File {}:", name);
        }

        match validate(|| ec.project(), 8) {
            Ok(project) => {
                let _ = writeln!(stdout, "  Project: {}", project);
            }
            Err(()) => {
                let _ = writeln!(stderr(), "Failed to read EC project");
                process::exit(1);
            }
        }

        match validate(|| ec.version(), 8) {
            Ok(version) => {
                let _ = writeln!(stdout, "  Version: {}", version);
            }
            Err(()) => {
                let _ = writeln!(stderr(), "Failed to read EC version");
                process::exit(1);
            }
        }

        match validate(|| ec.size(), 8) {
            Ok(size) => {
                let _ = writeln!(stdout, "  Size: {} KB", size / 1024);
            }
            Err(()) => {
                let _ = writeln!(stderr(), "Failed to read EC size");
                process::exit(1);
            }
        }
    }

    let _ = stdout.flush();
}
