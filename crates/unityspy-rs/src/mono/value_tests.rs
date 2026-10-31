use super::super::Mono;
use super::super::field::type_code;
use super::super::testing::{ClassSpec, Fake, FieldSpec, OFFSETS};
use super::*;

/// An object whose fields are one of each primitive, at the offsets named.
fn primitives() -> (Mono<crate::mem::reader::BufferMemory>, Object) {
    let fake = Fake::new();
    let image = fake.image(2);
    let class = fake.class(
        image,
        ClassSpec::new("Prims")
            .field(FieldSpec::new("flag", type_code::BOOLEAN, 16))
            .field(FieldSpec::new("ch", type_code::CHAR, 18))
            .field(FieldSpec::new("i1", type_code::I1, 20))
            .field(FieldSpec::new("u1", type_code::U1, 21))
            .field(FieldSpec::new("i2", type_code::I2, 22))
            .field(FieldSpec::new("u2", type_code::U2, 24))
            .field(FieldSpec::new("i4", type_code::I4, 28))
            .field(FieldSpec::new("u4", type_code::U4, 32))
            .field(FieldSpec::new("i8", type_code::I8, 40))
            .field(FieldSpec::new("u8", type_code::U8, 48))
            .field(FieldSpec::new("native", type_code::I, 56))
            .field(FieldSpec::new("unative", type_code::U, 64))
            .field(FieldSpec::new("r4", type_code::R4, 72))
            .field(FieldSpec::new("r8", type_code::R8, 80)),
    );
    let object = fake.object(class, 80);
    let m = &fake.memory;
    m.write_u8(object + 16, 1);
    m.write_u16(object + 18, u16::from(b'Z'));
    m.write_u8(object + 20, 0xFE); // -2 as I1
    m.write_u8(object + 21, 0xFE); // 254 as U1
    m.write(object + 22, &(-300i16).to_le_bytes());
    m.write_u16(object + 24, 60_000);
    m.write_i32(object + 28, -123_456);
    m.write_u32(object + 32, 4_000_000_000);
    m.write(object + 40, &(-5_000_000_000i64).to_le_bytes());
    m.write_u64(object + 48, 18_000_000_000_000_000_000);
    m.write(object + 56, &(-0x1_0000_0001i64).to_le_bytes());
    m.write_u64(object + 64, 0xFFFF_FFFF_0000_0001);
    m.write_f32(object + 72, 3.75);
    m.write_f64(object + 80, -1234.5678);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    (mono, object)
}

#[test]
fn primitives_are_read_at_their_correct_widths_and_signedness() {
    let (mono, object) = primitives();
    let get = |name: &str| mono.read_field(&object, name).unwrap();
    assert_eq!(get("flag").as_bool(), Some(true));
    assert!(matches!(get("ch"), Value::Char(0x5A)));
    assert!(matches!(get("i2"), Value::I16(-300)));
    assert!(matches!(get("u2"), Value::U16(60_000)));
    assert!(matches!(get("i4"), Value::I32(-123_456)));
    assert!(matches!(get("u4"), Value::U32(4_000_000_000)));
    assert!(matches!(get("i8"), Value::I64(-5_000_000_000)));
    assert!(matches!(get("u8"), Value::U64(18_000_000_000_000_000_000)));
}

#[test]
fn i1_is_signed_and_u1_is_unsigned() {
    // UnitySpy had these swapped.
    let (mono, object) = primitives();
    assert!(matches!(mono.read_field(&object, "i1").unwrap(), Value::I8(-2)));
    assert!(matches!(mono.read_field(&object, "u1").unwrap(), Value::U8(254)));
}

#[test]
fn floats_read_four_and_eight_bytes() {
    // UnitySpy read two bytes of each and returned garbage.
    let (mono, object) = primitives();
    assert_eq!(mono.read_field(&object, "r4").unwrap().as_f64(), Some(3.75));
    assert_eq!(mono.read_field(&object, "r8").unwrap().as_f64(), Some(-1234.5678));
}

#[test]
fn native_ints_are_eight_bytes_on_x64() {
    // UnitySpy read four bytes of each.
    let (mono, object) = primitives();
    assert!(matches!(mono.read_field(&object, "native").unwrap(), Value::IntPtr(-0x1_0000_0001)));
    assert!(matches!(mono.read_field(&object, "unative").unwrap(), Value::UIntPtr(0xFFFF_FFFF_0000_0001)));
}

/// A holder with one string field, pointing at the string `make` builds.
fn string_field(make: impl FnOnce(&Fake) -> u64) -> Result<Value> {
    let fake = Fake::new();
    let image = fake.image(2);
    let class = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("text", type_code::STRING, 16)));
    let object = fake.object(class, 8);
    let string = make(&fake);
    fake.memory.write_u64(object + 16, string);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    mono.read_field(&object, "text")
}

#[test]
fn strings_are_read_as_utf16() {
    let value = string_field(|fake| fake.string("Rosé Pine 🌲")).unwrap();
    assert_eq!(value.as_str(), Some("Rosé Pine 🌲"));
}

#[test]
fn an_empty_string_is_empty_and_a_null_one_is_null() {
    assert_eq!(string_field(|fake| fake.string("")).unwrap().as_str(), Some(""));
    assert!(string_field(|_| 0).unwrap().is_null());
}

#[test]
fn a_negative_string_length_is_rejected() {
    // UnitySpy would have asked for a negative size.
    let error = string_field(|fake| fake.string_with_length(-1)).unwrap_err();
    assert!(matches!(error, Error::Implausible { what: "string length", value: -1 }), "{error}");
}

#[test]
fn a_string_longer_than_the_cap_is_rejected_before_it_is_read() {
    let error = string_field(|fake| fake.string_with_length(MAX_STRING_UNITS + 1)).unwrap_err();
    assert!(matches!(error, Error::Implausible { what: "string length", .. }), "{error}");
}

#[test]
fn a_string_at_the_cap_is_accepted_as_far_as_its_length_goes() {
    // The length is fine, so the failure (no memory for 8 MiB of characters) is a read error,
    // not an implausible length.
    let error = string_field(|fake| fake.string_with_length(MAX_STRING_UNITS)).unwrap_err();
    assert!(matches!(error, Error::Read { .. }), "{error}");
}

#[test]
fn an_array_of_ints_is_read_element_by_element() {
    let fake = Fake::new();
    let image = fake.image(4);
    let int = fake.class(image, ClassSpec::new("Int32").namespace("System").value_type().byval(type_code::I4));
    let array_class = fake.array_class(int, 4);
    let data: Vec<u8> = [10i32, -20, 30].iter().flat_map(|v| v.to_le_bytes()).collect();
    let array = fake.array(array_class, 3, &data);
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("numbers", type_code::SZARRAY, 16)));
    let object = fake.object(holder, 8);
    fake.memory.write_u64(object + 16, array);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    let numbers = mono.read_field(&object, "numbers").unwrap();
    let numbers = numbers.as_array().unwrap();
    assert_eq!(numbers.length, 3);
    let values: Vec<_> = mono.array_items(numbers, usize::MAX).unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(values, [10, -20, 30]);
    assert_eq!(mono.array_items(numbers, 2).unwrap().len(), 2);
}

#[test]
fn an_array_of_references_holds_strings_and_nulls() {
    let fake = Fake::new();
    let image = fake.image(4);
    let string = fake.class(image, ClassSpec::new("String").namespace("System").byval(type_code::STRING));
    let array_class = fake.array_class(string, 8);
    let (a, b) = (fake.string("alpha"), fake.string("beta"));
    let data: Vec<u8> = [a, 0, b].iter().flat_map(|v| v.to_le_bytes()).collect();
    let array = fake.array(array_class, 3, &data);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let array = mono.array_at(array).unwrap();
    let items = mono.array_items(&array, usize::MAX).unwrap();
    assert_eq!(items[0].as_str(), Some("alpha"));
    assert!(items[1].is_null());
    assert_eq!(items[2].as_str(), Some("beta"));
}

#[test]
fn an_implausible_array_length_is_rejected() {
    let fake = Fake::new();
    let image = fake.image(2);
    let int = fake.class(image, ClassSpec::new("Int32").value_type().byval(type_code::I4));
    let array_class = fake.array_class(int, 4);
    let array = fake.array(array_class, -3, &[]);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert!(matches!(mono.array_at(array), Err(Error::Implausible { what: "array length", .. })));
}

#[test]
fn an_enum_field_reads_as_its_underlying_integer() {
    let fake = Fake::new();
    let image = fake.image(4);
    let tier = fake.class(image, ClassSpec::new("ETier").enum_type().field(FieldSpec::new("value__", type_code::I4, 16)));
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("tier", type_code::VALUETYPE, 16).data(tier.address)));
    let object = fake.object(holder, 8);
    fake.memory.write_i32(object + 16, 3);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(matches!(mono.read_field(&object, "tier").unwrap(), Value::I32(3)));
}

#[test]
fn a_value_type_that_cannot_be_read_is_an_error_not_a_silent_int() {
    // UnitySpy swallowed the failure and returned the first four bytes as an int.
    let fake = Fake::new();
    let image = fake.image(2);
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("broken", type_code::VALUETYPE, 16).data(0xDEAD_0000)));
    let object = fake.object(holder, 8);
    fake.memory.write_i32(object + 16, 7);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(mono.read_field(&object, "broken").is_err());
}

#[test]
fn a_type_that_cannot_be_read_is_an_error_naming_its_code() {
    let fake = Fake::new();
    let image = fake.image(2);
    let holder = fake.class(image, ClassSpec::new("Holder").field(FieldSpec::new("p", type_code::PTR, 16)));
    let object = fake.object(holder, 8);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(matches!(mono.read_field(&object, "p"), Err(Error::UnsupportedType(0x0F))));
}

#[test]
fn a_field_of_a_generic_parameter_type_is_an_error_naming_it() {
    let fake = Fake::new();
    let image = fake.image(2);
    let holder = fake.class(image, ClassSpec::new("Box`1").field(FieldSpec::new("value", type_code::VAR, 16)));
    let object = fake.object(holder, 8);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let object = mono.object_at(object).unwrap();
    assert!(matches!(mono.read_field(&object, "value"), Err(Error::UnresolvedGeneric { .. })));
}
