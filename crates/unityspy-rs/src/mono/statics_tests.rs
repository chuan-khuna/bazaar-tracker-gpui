use super::super::Mono;
use super::super::field::type_code;
use super::super::testing::{ClassSpec, Fake, FieldSpec, OFFSETS};
use super::*;

#[test]
fn a_static_is_read_through_the_vtable_at_its_offset() {
    let fake = Fake::new();
    let image = fake.image(2);
    let data = fake.class(
        image,
        ClassSpec::new("Data").field(FieldSpec::new("Count", type_code::I4, 0).stat()).field(FieldSpec::new("Level", type_code::I4, 8).stat()).statics(16),
    );
    fake.memory.write_i32(data.statics, 42);
    fake.memory.write_i32(data.statics + 8, -7);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(data.address).unwrap();
    assert_eq!(mono.read_static(&class, "Count").unwrap().as_i64(), Some(42));
    assert_eq!(mono.read_static(&class, "Level").unwrap().as_i64(), Some(-7));
}

#[test]
fn a_null_vtable_is_a_distinct_not_initialised_error() {
    let fake = Fake::new();
    let image = fake.image(2);
    let lazy = fake.class(image, ClassSpec::new("Lazy").field(FieldSpec::new("Count", type_code::I4, 0).stat()).uninitialised());
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(lazy.address).unwrap();
    let error = mono.read_static(&class, "Count").unwrap_err();
    assert!(matches!(&error, Error::NotInitialised(name) if name == "Lazy"), "{error}");
}

#[test]
fn a_class_cached_before_it_was_initialised_is_read_once_it_is() {
    let fake = Fake::new();
    let image = fake.image(2);
    let lazy = fake.class(image, ClassSpec::new("Lazy").field(FieldSpec::new("Count", type_code::I4, 0).stat()).uninitialised());
    // What the runtime sets up later: the vtable and the static block.
    let later = fake.class_struct(&ClassSpec::new("Lazy").statics(8));
    fake.memory.write_i32(later.statics, 42);
    let runtime_info = fake.memory.read_ptr(later.address + OFFSETS.class_runtime_info).unwrap();
    let vtable_size = fake.memory.read_i32(later.address + OFFSETS.class_vtable_size).unwrap();
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(lazy.address).unwrap();
    assert!(matches!(mono.read_static(&class, "Count"), Err(Error::NotInitialised(_))));

    mono.memory().write_u64(lazy.address + OFFSETS.class_runtime_info, runtime_info);
    mono.memory().write_i32(lazy.address + OFFSETS.class_vtable_size, vtable_size);
    let class = mono.class_at(lazy.address).unwrap();
    assert_eq!(class.vtable, 0, "the cached class is the one from before");
    assert_eq!(mono.read_static(&class, "Count").unwrap().as_i64(), Some(42));
}

#[test]
fn a_class_with_a_vtable_but_no_static_block_is_not_initialised_either() {
    let fake = Fake::new();
    let image = fake.image(2);
    let half = fake.class(image, ClassSpec::new("Half").field(FieldSpec::new("Count", type_code::I4, 0).stat()));
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(half.address).unwrap();
    assert!(matches!(mono.read_static(&class, "Count"), Err(Error::NotInitialised(_))));
}

#[test]
fn a_singleton_is_reached_through_its_base_class() {
    let fake = Fake::new();
    let image = fake.image(4);
    let singleton = fake.class(image, ClassSpec::new("Singleton").field(FieldSpec::new("Instance", type_code::CLASS, 0).stat()).statics(8));
    let mut child = ClassSpec::new("BoardManager");
    child.parent = singleton.address;
    let child = fake.class(image, child);
    let instance = fake.object(child, 0);
    fake.memory.write_u64(singleton.statics, instance);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(child.address).unwrap();
    let value = mono.read_static(&class, "Instance").unwrap();
    assert_eq!(value.as_object().unwrap().address, instance);
}

#[test]
fn an_instance_field_or_a_const_is_not_a_readable_static() {
    let fake = Fake::new();
    let image = fake.image(2);
    let data = fake.class(
        image,
        ClassSpec::new("Data").field(FieldSpec::new("name", type_code::I4, 16)).field(FieldSpec::new("Limit", type_code::I4, 0).constant()).statics(8),
    );
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(data.address).unwrap();
    assert!(matches!(mono.read_static(&class, "name"), Err(Error::NotAStaticField { .. })));
    assert!(matches!(mono.read_static(&class, "Limit"), Err(Error::NotAStaticField { .. })));
    assert!(matches!(mono.read_static(&class, "missing"), Err(Error::FieldNotFound { .. })));
}
