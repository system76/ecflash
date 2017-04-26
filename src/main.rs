use std::{env, process};
use std::io::{stdout, stderr, BufWriter, Write};

use ec::{Ec, EcFile, EcFlash};

mod ec;

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
        let _ = writeln!(stdout, "  Project: {}", ec.project());
        let _ = writeln!(stdout, "  Version: {}", ec.version());
        let _ = writeln!(stdout, "  Size: {} KB", ec.size()/1024);
    }
}
