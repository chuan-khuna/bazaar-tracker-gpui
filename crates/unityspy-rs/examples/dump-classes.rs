//! The memory-reader spike: attach to the running game and print what it finds.
//!
//! `just spike [process-name]` (default `TheBazaar`). It only reads: the process is opened with
//! `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION`.
//!
//! After the assemblies, it prints the fields of the classes the tracker needs, with offsets, type
//! codes and whether each is static, including fields inherited from parent classes.

use unityspy_rs::mem::process::Process;
use unityspy_rs::mono::Mono;

const DEFAULT_PROCESS: &str = "TheBazaar";
const GAME_ASSEMBLY: &str = "TheBazaarRuntime";
const CLASSES: [&str; 4] = ["TheBazaar.Data", "TheBazaar.AppState", "BoardManager", "TheBazaar.ClientCache"];

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

    let image = mono.assembly(GAME_ASSEMBLY)?.image;
    for full_name in CLASSES {
        println!();
        match mono.find_class(image, full_name) {
            Err(error) => println!("{full_name}: {error}"),
            Ok(class) => {
                let initialised = if class.vtable == 0 { "not initialised" } else { "initialised" };
                println!("{} at {:#x} ({initialised}, size {}, {} own fields)", class.full_name, class.address, class.size, class.fields.len());
                for field in mono.fields_with_inherited(&class)? {
                    let flags = match (field.is_static(), field.is_constant()) {
                        (_, true) => "const ",
                        (true, _) => "static",
                        _ => "      ",
                    };
                    println!("  {flags} +{:#06x} type {:#04x}  {}  ({})", field.offset, field.type_code(), field.name, field.declaring_name);
                }
            }
        }
    }
    Ok(())
}
