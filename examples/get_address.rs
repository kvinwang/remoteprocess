extern crate remoteprocess;
extern crate env_logger;
extern crate goblin;
#[cfg(target_os="linux")]
extern crate nix;

#[cfg(feature="unwind")]
fn get_addr(pid: remoteprocess::Pid, name: &str) -> Result<(), remoteprocess::Error> {
    // Create a new handle to the process
    let symbolicator = remoteprocess::Symbolicator::new(pid)?;
    let addr = symbolicator.address_of(name);
    match addr {
        Some(addr) => {
            println!("{}: 0x{:016x}", name, addr);
        }
        None => {
            println!("{}: N/A", name);
        }
    }
    Ok(())
}

#[cfg(feature="unwind")]
fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    let pid = if args.len() > 1 {
        args[1].parse().expect("invalid pid")
    } else {
        std::process::id()
    };
    let name = if args.len() > 2 {
        &args[2]
    } else {
        "my_alloc"
    };

    if let Err(e) = get_addr(pid as remoteprocess::Pid, name) {
        println!("Failed to get backtrace {:?}", e);
    }
}

#[cfg(not(feature="unwind"))]
fn main() {
    panic!("unwind not supported!");
}
