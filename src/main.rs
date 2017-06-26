use std::{env, process};
use std::fmt::Display;
use std::io::{stdout, stderr, BufWriter, Write};

use ec::{Ec, EcFile, EcFlash};

mod ec;

fn validate<T: PartialEq + Display, F: FnMut() -> T>(mut f: F, attempts: usize) -> Option<T> {
    for _attempt_i in 0..attempts {
        let a = f();
        let b = f();
        if a == b {
            return Some(a);
        } else {
            let _ = writeln!(stderr(), "Attempt {}: {} != {}", _attempt_i, a, b);
        }
    }
    None
}

fn main() {
    let mut ecs: Vec<(String, Box<Ec>)> = Vec::new();

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "-0" => match EcFlash::new(0) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                },
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 0: {}", err);
                    process::exit(1);
                }
            },
            "-1" => match EcFlash::new(1) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                },
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 1: {}", err);
                    process::exit(1);
                }
            },
            "-2" => match EcFlash::new(2) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                },
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 2: {}", err);
                    process::exit(1);
                }
            },
            "-3" => match EcFlash::new(3) {
                Ok(ec_flash) => {
                    ecs.push((String::new(), Box::new(ec_flash)));
                },
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC flash 3: {}", err);
                    process::exit(1);
                }
            },
            _ => match EcFile::new(&arg) {
                Ok(ec_file) => {
                    ecs.push((arg, Box::new(ec_file)));
                },
                Err(err) => {
                    let _ = writeln!(stderr(), "Failed to open EC file '{}': {}", arg, err);
                    process::exit(1);
                }
            }
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
            Some(project) => {
                let _ = writeln!(stdout, "  Project: {}", project);
            },
            None => {
                let _ = writeln!(stderr(), "Failed to read EC project");
                process::exit(1);
            }
        }

        match validate(|| ec.version(), 8) {
            Some(version) => {
                let _ = writeln!(stdout, "  Version: {}", version);
            },
            None => {
                let _ = writeln!(stderr(), "Failed to read EC version");
                process::exit(1);
            }
        }

        match validate(|| ec.size(), 8) {
            Some(size) => {
                let _ = writeln!(stdout, "  Size: {} KB", size/1024);
            },
            None => {
                let _ = writeln!(stderr(), "Failed to read EC size");
                process::exit(1);
            }
        }
    }

    let _ = stdout.flush();
}
