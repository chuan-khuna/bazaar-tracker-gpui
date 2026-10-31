//! The memory-reader spike: attach to the running game and print what it finds.
//!
//! `just spike [process-name]` (default `TheBazaar`). It only reads: the process is opened with
//! `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION`.

use unityspy_rs::mem::process::Process;
use unityspy_rs::mono::Mono;

const DEFAULT_PROCESS: &str = "TheBazaar";

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_PROCESS.to_owned());
    if let Err(error) = run(&name) {
        eprintln!("dump-classes: {error}");
        std::process::exit(1);
    }
}

fn run(name: &str) -> unityspy_rs::error::Result<()> {
    let process = Process::open_by_name(name)?;
    let (mono, info) = Mono::attach(process)?;
    println!("process {name} (pid {})", info.pid);
    println!("exe version {} -> Unity {}", info.file_version, info.unity_version);
    let offsets = mono.offsets();
    println!("offsets: {}{}", offsets.name, if offsets.verified { "" } else { " [not verified against a real game]" });
    println!("{} at {:#x}, root domain at {:#x}", info.mono_module.name, info.mono_module.base, mono.root_domain());
    println!("assemblies:");
    for assembly in mono.assemblies()? {
        println!("  {} (image {:#x})", assembly.name, assembly.image);
    }
    Ok(())
}
