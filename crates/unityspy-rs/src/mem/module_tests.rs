use super::*;

#[test]
fn module_names_match_ignoring_case() {
    let path = Path::new(r"C:\Game\TheBazaar_Data\Mono\EmbedRuntime\mono-2.0-bdwgc.dll");
    assert!(name_matches(path, "mono-2.0-bdwgc.dll"));
    assert!(name_matches(path, "MONO-2.0-BDWGC.DLL"));
    assert!(!name_matches(path, "mono-2.0"));
}

#[test]
fn a_loaded_system_module_is_found_with_its_base_and_size() {
    let process = Process::open(std::process::id()).unwrap();
    let module = find_module(&process, "KERNEL32.DLL").unwrap();
    assert!(module.base != 0 && module.size > 0);
    assert!(modules(&process).unwrap().len() > 3);
}

#[test]
fn a_missing_module_is_named_in_the_error() {
    let process = Process::open(std::process::id()).unwrap();
    let error = find_module(&process, "mono-2.0-bdwgc.dll").unwrap_err();
    assert!(matches!(&error, Error::ModuleNotFound(name) if name == "mono-2.0-bdwgc.dll"));
}
