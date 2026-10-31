use super::super::Mono;
use super::super::field::type_code;
use super::super::testing::{ClassSpec, Fake, FieldSpec, OFFSETS};
use super::*;

/// A runtime over `fake`'s memory; the domain is not used by class lookup.
fn mono(fake: Fake) -> Mono<crate::mem::reader::BufferMemory> {
    Mono::new(fake.memory, OFFSETS, 0)
}

#[test]
fn a_class_is_found_by_namespace_and_name_among_many_in_one_bucket() {
    let fake = Fake::new();
    let image = fake.image(2);
    for name in ["A", "B", "C", "D", "E"] {
        fake.class(image, ClassSpec::new(name).namespace("Other"));
    }
    let data = fake.class(image, ClassSpec::new("Data").namespace("TheBazaar"));
    fake.class(image, ClassSpec::new("Data").namespace("Elsewhere"));
    let mono = mono(fake);
    let found = mono.find_class(image, "TheBazaar.Data").unwrap();
    assert_eq!(found.address, data.address);
    assert_eq!(found.full_name, "TheBazaar.Data");
    assert_eq!((found.name.as_str(), found.namespace.as_str()), ("Data", "TheBazaar"));
}

#[test]
fn a_class_in_the_global_namespace_is_found_by_its_bare_name() {
    let fake = Fake::new();
    let image = fake.image(4);
    let board = fake.class(image, ClassSpec::new("BoardManager"));
    let mono = mono(fake);
    assert_eq!(mono.find_class(image, "BoardManager").unwrap().address, board.address);
}

#[test]
fn a_missing_class_is_an_error_naming_it() {
    let fake = Fake::new();
    let image = fake.image(4);
    fake.class(image, ClassSpec::new("Data").namespace("TheBazaar"));
    let mono = mono(fake);
    let error = mono.find_class(image, "TheBazaar.Nope").unwrap_err();
    assert!(matches!(&error, Error::ClassNotFound(name) if name == "TheBazaar.Nope"));
    assert!(mono.find_class(image, "Data").is_err(), "the namespace matters");
}

#[test]
fn the_lookup_stops_when_the_class_is_found() {
    let fake = Fake::new();
    let image = fake.image(1);
    // The cache is one chain, newest first. After the wanted class comes a link into unmapped
    // memory, which would fail the walk if it went on.
    let broken = fake.class(image, ClassSpec::new("Broken"));
    let wanted = fake.class(image, ClassSpec::new("Wanted"));
    fake.memory.write_u64(broken.address + OFFSETS.class_next_class_cache, 0xDEAD_0000);
    let mono = mono(fake);
    assert_eq!(mono.find_class(image, "Wanted").unwrap().address, wanted.address);
    assert!(mono.find_class(image, "Missing").is_err());
}

#[test]
fn a_found_class_is_cached_by_full_name() {
    let fake = Fake::new();
    let image = fake.image(1);
    let wanted = fake.class(image, ClassSpec::new("Wanted").namespace("N"));
    let mono = mono(fake);
    let first = mono.find_class(image, "N.Wanted").unwrap();
    // Unlink it from the cache: only the remembered answer can still find it.
    let bucket = mono.memory().read_ptr(mono.memory().read_ptr(image + OFFSETS.image_class_cache + OFFSETS.hash_table_table).unwrap()).unwrap();
    assert_eq!(bucket, wanted.address);
    mono.memory().write_u64(mono.memory().read_ptr(image + OFFSETS.image_class_cache + OFFSETS.hash_table_table).unwrap(), 0);
    let second = mono.find_class(image, "N.Wanted").unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn nested_classes_are_named_with_a_plus_and_found_through_their_outer_class() {
    let fake = Fake::new();
    let image = fake.image(3);
    let outer = fake.class(image, ClassSpec::new("Outer").namespace("Ns"));
    let inner = fake.class(image, ClassSpec::new("Inner").nested_in(outer.address));
    // A top-level class that happens to share the nested class's name is not the nested one.
    fake.class(image, ClassSpec::new("Inner").namespace("Ns"));
    let mono = mono(fake);
    let found = mono.find_class(image, "Ns.Outer+Inner").unwrap();
    assert_eq!(found.address, inner.address);
    assert_eq!(found.full_name, "Ns.Outer+Inner");
    assert_eq!(found.nested_in, outer.address);
    assert!(mono.find_class(image, "Ns.Outer+Missing").is_err());
}

#[test]
fn the_class_model_reads_its_members_in_one_go() {
    let fake = Fake::new();
    let image = fake.image(4);
    let parent = fake.class(image, ClassSpec::new("Base").namespace("N").size(24));
    let child = fake.class(
        image,
        ClassSpec::new("Child")
            .namespace("N")
            .parent(parent.address)
            .size(40)
            .field(FieldSpec::new("count", type_code::I4, 16))
            .field(FieldSpec::new("Instance", type_code::CLASS, 0).stat())
            .statics(16),
    );
    let mono = mono(fake);
    let class = mono.class_at(child.address).unwrap();
    assert_eq!(class.parent, parent.address);
    assert_eq!(class.size, 40);
    assert_eq!(class.field_count, 2);
    assert_eq!(class.kind, ClassKind::Def);
    assert!(!class.is_value_type && !class.is_enum);
    assert_eq!(class.vtable, child.vtable);
    assert_eq!(class.vtable_size, 4);
    assert_eq!(class.byval.type_code(), type_code::CLASS);
    let names: Vec<_> = class.fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(names, ["count", "Instance"]);
}

#[test]
fn value_type_and_enum_flags_are_decoded() {
    let fake = Fake::new();
    let image = fake.image(2);
    let point = fake.class(image, ClassSpec::new("Point").value_type());
    let colour = fake.class(image, ClassSpec::new("Colour").enum_type());
    let mono = mono(fake);
    assert!(mono.class_at(point.address).unwrap().is_value_type);
    let colour = mono.class_at(colour.address).unwrap();
    assert!(colour.is_value_type && colour.is_enum);
}

#[test]
fn a_class_the_runtime_has_not_initialised_has_no_vtable() {
    let fake = Fake::new();
    let image = fake.image(2);
    let lazy = fake.class(image, ClassSpec::new("Lazy").uninitialised());
    let mono = mono(fake);
    assert_eq!(mono.class_at(lazy.address).unwrap().vtable, 0);
}

#[test]
fn field_model_gives_name_type_code_attributes_offset_and_type_data() {
    let fake = Fake::new();
    let image = fake.image(2);
    let other = fake.class(image, ClassSpec::new("Other"));
    let data = fake.class(
        image,
        ClassSpec::new("Data")
            .field(FieldSpec::new("Entities", type_code::CLASS, 8).data(other.address).stat())
            .field(FieldSpec::new("Limit", type_code::I4, 0).constant())
            .field(FieldSpec::new("name", type_code::STRING, 24)),
    );
    let mono = mono(fake);
    let class = mono.class_at(data.address).unwrap();
    let entities = mono.find_field(&class, "Entities", None).unwrap();
    assert_eq!((entities.type_code(), entities.offset), (type_code::CLASS, 8));
    assert!(entities.is_static() && !entities.is_constant());
    assert_eq!(entities.type_data(), other.address);
    assert_eq!(entities.declaring_name, "Data");
    let limit = mono.find_field(&class, "Limit", None).unwrap();
    assert!(limit.is_constant());
    let name = mono.find_field(&class, "name", None).unwrap();
    assert!(!name.is_static());
    assert_eq!(name.attrs() & 0xFFFF, 0);
}

#[test]
fn a_field_lookup_climbs_to_the_parent_and_the_nearest_declaration_wins() {
    let fake = Fake::new();
    let image = fake.image(4);
    let grand = fake.class(image, ClassSpec::new("Grand").namespace("N").field(FieldSpec::new("id", type_code::I4, 16)).field(FieldSpec::new("deep", type_code::I8, 24)));
    let base = fake.class(image, ClassSpec::new("Base").namespace("N").parent(grand.address).field(FieldSpec::new("id", type_code::I8, 32)));
    let child = fake.class(image, ClassSpec::new("Child").namespace("N").parent(base.address).field(FieldSpec::new("own", type_code::I4, 40)));
    let mono = mono(fake);
    let class = mono.class_at(child.address).unwrap();

    assert_eq!(mono.find_field(&class, "own", None).unwrap().declaring_name, "N.Child");
    assert_eq!(mono.find_field(&class, "deep", None).unwrap().declaring_name, "N.Grand");
    // `id` is declared twice: the nearest is Base's, and the declaring type reaches Grand's.
    assert_eq!(mono.find_field(&class, "id", None).unwrap().offset, 32);
    assert_eq!(mono.find_field(&class, "id", Some("N.Grand")).unwrap().offset, 16);
    assert!(mono.find_field(&class, "own", Some("N.Base")).is_err());
}

#[test]
fn a_missing_field_is_an_error_naming_the_class_and_field() {
    let fake = Fake::new();
    let image = fake.image(2);
    let data = fake.class(image, ClassSpec::new("Data").namespace("N").field(FieldSpec::new("a", type_code::I4, 16)));
    let mono = mono(fake);
    let class = mono.class_at(data.address).unwrap();
    let error = mono.find_field(&class, "b", None).unwrap_err();
    assert!(matches!(&error, Error::FieldNotFound { class, field } if class == "N.Data" && field == "b"));
}

#[test]
fn all_fields_with_inherited_lists_the_child_first() {
    let fake = Fake::new();
    let image = fake.image(2);
    let base = fake.class(image, ClassSpec::new("Base").field(FieldSpec::new("b", type_code::I4, 16)));
    let child = fake.class(image, ClassSpec::new("Child").parent(base.address).field(FieldSpec::new("c", type_code::I4, 20)));
    let mono = mono(fake);
    let class = mono.class_at(child.address).unwrap();
    let names: Vec<_> = mono.fields_with_inherited(&class).unwrap().into_iter().map(|field| field.name).collect();
    assert_eq!(names, ["c", "b"]);
}

#[test]
fn a_generic_instance_takes_its_field_count_from_the_definition() {
    let fake = Fake::new();
    let image = fake.image(2);
    let definition = fake.class(image, ClassSpec::new("Pair`2").field(FieldSpec::new("first", type_code::I4, 16)).field(FieldSpec::new("second", type_code::I4, 20)));
    let generic_class = fake.alloc(32);
    fake.memory.write_u64(generic_class, definition.address);
    let mut spec = ClassSpec::new("Pair`2");
    spec.kind = 3;
    spec.generic_class = generic_class;
    let instance = fake.class_struct(&spec);
    // The instance shares the definition's field array.
    let fields = fake.memory.read_ptr(definition.address + OFFSETS.class_fields).unwrap();
    fake.memory.write_u64(instance.address + OFFSETS.class_fields, fields);
    let mono = mono(fake);
    let class = mono.class_at(instance.address).unwrap();
    assert_eq!(class.kind, ClassKind::GenericInstance);
    assert_eq!(class.field_count, 2);
    assert_eq!(class.fields.len(), 2);
}
