use super::super::Mono;
use super::super::field::type_code;
use super::super::testing::{ClassSpec, Fake, FieldSpec, OFFSETS};
use super::*;

#[test]
fn fields_are_read_on_the_runtime_class_not_the_declared_one() {
    let fake = Fake::new();
    let image = fake.image(4);
    let base = fake.class(image, ClassSpec::new("Card").field(FieldSpec::new("id", type_code::I4, 16)));
    let derived = fake.class(image, ClassSpec::new("ItemCard").parent(base.address).field(FieldSpec::new("tier", type_code::I4, 20)));
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("card", type_code::CLASS, 16).data(base.address)));
    let card = fake.object(derived, 8);
    fake.memory.write_i32(card + 16, 5);
    fake.memory.write_i32(card + 20, 3);
    let holder_object = fake.object(holder, 8);
    fake.memory.write_u64(holder_object + 16, card);
    let mono = Mono::new(fake.memory, OFFSETS, 0);

    let holder = mono.object_at(holder_object).unwrap();
    let card = mono.read_field(&holder, "card").unwrap();
    let card = card.as_object().unwrap();
    assert_eq!(card.class.full_name, "ItemCard", "the runtime class, not Card");
    assert_eq!(mono.read_field(card, "tier").unwrap().as_i64(), Some(3));
    assert_eq!(mono.read_field(card, "id").unwrap().as_i64(), Some(5));
}

#[test]
fn a_null_reference_field_is_null() {
    let fake = Fake::new();
    let image = fake.image(2);
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("card", type_code::CLASS, 16)));
    let object = fake.object(holder, 8);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(mono.read_field(&object, "card").unwrap().is_null());
}

#[test]
fn an_address_with_no_vtable_is_not_an_object() {
    let fake = Fake::new();
    let nothing = fake.alloc(32);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert!(matches!(mono.object_at(nothing), Err(Error::Unexpected(_))));
}

#[test]
fn a_static_field_has_no_value_on_an_object() {
    let fake = Fake::new();
    let image = fake.image(2);
    let class = fake.class(image, ClassSpec::new("Data").field(FieldSpec::new("Count", type_code::I4, 0).stat()).statics(8));
    let object = fake.object(class, 0);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(matches!(mono.read_field(&object, "Count"), Err(Error::NotAnInstanceField { .. })));
}

#[test]
fn an_inline_struct_field_is_read_at_its_offset_minus_the_header() {
    let fake = Fake::new();
    let image = fake.image(4);
    let point = fake.class(image, ClassSpec::new("Point").value_type().size(24).field(FieldSpec::new("x", type_code::I4, 16)).field(FieldSpec::new("y", type_code::I4, 20)));
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("tag", type_code::I4, 16)).field(FieldSpec::new("p", type_code::VALUETYPE, 24).data(point.address)));
    let object = fake.object(holder, 16);
    // The struct's data starts at object + 24 and has no header: x is its first 4 bytes.
    fake.memory.write_i32(object + 24, 11);
    fake.memory.write_i32(object + 28, 22);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    let p = mono.read_field(&object, "p").unwrap();
    let p = p.as_struct().unwrap();
    assert_eq!(p.address, object.address + 24);
    assert_eq!(mono.read_struct_field(p, "x").unwrap().as_i64(), Some(11));
    assert_eq!(mono.read_struct_field(p, "y").unwrap().as_i64(), Some(22));
}
