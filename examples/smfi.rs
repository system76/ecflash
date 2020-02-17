use hwio::{Io, Pio};
use std::{io, process, thread, time};

#[repr(u8)]
enum SmfiCmd {
    None = 0,
    Probe = 1,
    Board = 2,
    Version = 3,
}

#[repr(u8)]
enum SmfiRes {
    Ok = 0,
    Err = 1,
}

unsafe fn get(offset: u8) -> u8 {
    Pio::<u8>::new(
        0xC00 + (offset as u16)
    ).read()
}

unsafe fn set(offset: u8, value: u8) {
    Pio::<u8>::new(
        0xC00 + (offset as u16)
    ).write(value)
}

unsafe fn dbg(offset: u8) -> u8 {
    Pio::<u8>::new(
        0xD00 + (offset as u16)
    ).read()
}

unsafe fn smfi() {
    set(0, SmfiCmd::Probe as u8);
    while get(0) != SmfiCmd::None as u8 {
        thread::sleep(time::Duration::from_millis(1));
    }
    if get(1) == SmfiRes::Ok as u8 {
        let signature = (get(2), get(3));
        let protocol = get(4);
        println!(
            "signature {:02X}{:02X} protocol {}",
            signature.0,
            signature.1,
            protocol
        );
    }

    set(0, SmfiCmd::Board as u8);
    while get(0) != SmfiCmd::None as u8 {
        thread::sleep(time::Duration::from_millis(1));
    }
    if get(1) == SmfiRes::Ok as u8 {
        print!("board ");
        for i in 2..=255 {
            let b = get(i);
            if b == 0 {
                break;
            }
            print!("{}", b as char);
        }
        println!();
    }

    set(0, SmfiCmd::Version as u8);
    while get(0) != SmfiCmd::None as u8 {
        thread::sleep(time::Duration::from_millis(1));
    }
    if get(1) == SmfiRes::Ok as u8 {
        print!("version ");
        for i in 2..=255 {
            let b = get(i);
            if b == 0 {
                break;
            }
            print!("{}", b as char);
        }
        println!();
    }

    let mut head = dbg(0) as usize;
    loop {
        let tail = dbg(0) as usize;
        if tail == 0 || head == tail {
            thread::sleep(time::Duration::from_millis(1));
        } else {
            while head != tail {
                head += 1;
                if head >= 256 { head = 1; }
                let c = dbg(head as u8);
                print!("{}", c as char);
            }
        }
    }
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

        smfi();
    }
}
