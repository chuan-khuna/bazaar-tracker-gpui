use std::sync::Arc;

use super::super::Mono;
use super::super::field::type_code;
use super::super::testing::{Built, ClassSpec, Fake, FieldSpec, OFFSETS};
use super::*;
use crate::mem::reader::BufferMemory;

fn int_class(fake: &Fake, image: u64) -> Built {
    fake.class(image, ClassSpec::new("Int32").namespace("System").value_type().byval(type_code::I4))
}

fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn object(mono: &Mono<BufferMemory>, address: u64) -> Object {
    mono.object_at(address).unwrap()
}

/// A `List<int>` with room for `capacity` items, holding `values`; the names are those of `items`
/// and `size`.
fn list(fake: &Fake, image: u64, values: &[i32], capacity: usize, items: &'static str, size: &'static str) -> u64 {
    let int = int_class(fake, image);
    let array_class = fake.array_class(int, 4);
    let mut data = ints(values);
    data.resize(capacity * 4, 0);
    let array = fake.array(array_class, capacity as i32, &data);
    let class = fake.class(image, ClassSpec::new("List`1").field(FieldSpec::new(items, type_code::SZARRAY, 16)).field(FieldSpec::new(size, type_code::I4, 24)));
    let list = fake.object(class, 16);
    fake.memory.write_u64(list + 16, array);
    fake.memory.write_i32(list + 24, values.len() as i32);
    list
}

#[test]
fn a_list_reads_only_as_many_items_as_its_size() {
    let fake = Fake::new();
    let image = fake.image(4);
    let list = list(&fake, image, &[7, 8, 9], 8, "_items", "_size");
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let items = mono.read_list(&object(&mono, list)).unwrap();
    assert_eq!(items.iter().map(|item| item.as_i64().unwrap()).collect::<Vec<_>>(), [7, 8, 9]);
}

#[test]
fn field_names_are_looked_up_from_the_class_so_other_profiles_work() {
    let fake = Fake::new();
    let image = fake.image(4);
    let list = list(&fake, image, &[1, 2], 2, "items", "size");
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert_eq!(mono.read_list(&object(&mono, list)).unwrap().len(), 2);
}

#[test]
fn an_empty_list_with_no_array_yet_is_empty() {
    let fake = Fake::new();
    let image = fake.image(2);
    let class = fake.class(image, ClassSpec::new("List`1").field(FieldSpec::new("_items", type_code::SZARRAY, 16)).field(FieldSpec::new("_size", type_code::I4, 24)));
    let list = fake.object(class, 16);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert!(mono.read_list(&object(&mono, list)).unwrap().is_empty());
}

#[test]
fn a_list_with_a_negative_size_is_rejected() {
    let fake = Fake::new();
    let image = fake.image(4);
    let list = list(&fake, image, &[1], 1, "_items", "_size");
    fake.memory.write_i32(list + 24, -1);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert!(matches!(mono.read_list(&object(&mono, list)), Err(Error::Implausible { what: "collection count", .. })));
}

/// A `Dictionary<int,int>` whose entries are `(hash, key, value)`, laid out as the instantiated
/// `Entry` struct says: hash at 16, next at 20, key at 24, value at 28 (all including the header).
fn dictionary(fake: &Fake, image: u64, entries: &[(i32, i32, i32)]) -> u64 {
    let entry = fake.class(
        image,
        ClassSpec::new("Entry")
            .value_type()
            .size(32)
            .field(FieldSpec::new("hashCode", type_code::I4, 16))
            .field(FieldSpec::new("next", type_code::I4, 20))
            .field(FieldSpec::new("key", type_code::I4, 24))
            .field(FieldSpec::new("value", type_code::I4, 28)),
    );
    let array_class = fake.array_class(entry, 16);
    let data: Vec<u8> = entries.iter().flat_map(|&(hash, key, value)| ints(&[hash, -1, key, value])).collect();
    let array = fake.array(array_class, entries.len() as i32 + 2, &data);
    let class = fake.class(image, ClassSpec::new("Dictionary`2").field(FieldSpec::new("_entries", type_code::SZARRAY, 16)).field(FieldSpec::new("_count", type_code::I4, 24)));
    let dictionary = fake.object(class, 16);
    fake.memory.write_u64(dictionary + 16, array);
    fake.memory.write_i32(dictionary + 24, entries.len() as i32);
    dictionary
}

#[test]
fn a_dictionary_gives_its_pairs_and_skips_free_entries() {
    let fake = Fake::new();
    let image = fake.image(4);
    // The middle entry was removed: its hash code is negative.
    let dictionary = dictionary(&fake, image, &[(100, 10, 111), (-1, 20, 222), (300, 30, 333)]);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let pairs = mono.read_dictionary(&object(&mono, dictionary)).unwrap();
    let pairs: Vec<_> = pairs.iter().map(|(key, value)| (key.as_i64().unwrap(), value.as_i64().unwrap())).collect();
    assert_eq!(pairs, [(10, 111), (30, 333)]);
}

#[test]
fn struct_field_offsets_come_from_the_instantiated_entry_class() {
    // The offsets are those of the entry class the array holds, not guessed from the key size.
    // An entry with a 8-byte key moves the value to 32.
    let fake = Fake::new();
    let image = fake.image(4);
    let entry = fake.class(
        image,
        ClassSpec::new("Entry")
            .value_type()
            .size(40)
            .field(FieldSpec::new("hashCode", type_code::I4, 16))
            .field(FieldSpec::new("key", type_code::I8, 24))
            .field(FieldSpec::new("value", type_code::I4, 32)),
    );
    let array_class = fake.array_class(entry, 24);
    let mut data = Vec::new();
    data.extend(5i32.to_le_bytes());
    data.extend(0i32.to_le_bytes());
    data.extend(9_000_000_000i64.to_le_bytes());
    data.extend(77i32.to_le_bytes());
    data.extend(0i32.to_le_bytes());
    let array = fake.array(array_class, 1, &data);
    let class = fake.class(image, ClassSpec::new("Dictionary`2").field(FieldSpec::new("entries", type_code::SZARRAY, 16)).field(FieldSpec::new("count", type_code::I4, 24)));
    let dictionary = fake.object(class, 16);
    fake.memory.write_u64(dictionary + 16, array);
    fake.memory.write_i32(dictionary + 24, 1);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let pairs = mono.read_dictionary(&object(&mono, dictionary)).unwrap();
    assert_eq!((pairs[0].0.as_i64(), pairs[0].1.as_i64()), (Some(9_000_000_000), Some(77)));
}

#[test]
fn a_dictionary_entry_with_an_unresolved_generic_type_is_an_error() {
    let fake = Fake::new();
    let image = fake.image(2);
    let entry = fake.class(
        image,
        ClassSpec::new("Entry`2").value_type().size(32).field(FieldSpec::new("hashCode", type_code::I4, 16)).field(FieldSpec::new("key", type_code::VAR, 24)).field(FieldSpec::new("value", type_code::VAR, 28)),
    );
    let array_class = fake.array_class(entry, 16);
    let array = fake.array(array_class, 1, &ints(&[1, 0, 5, 6]));
    let class = fake.class(image, ClassSpec::new("Dictionary`2").field(FieldSpec::new("_entries", type_code::SZARRAY, 16)).field(FieldSpec::new("_count", type_code::I4, 24)));
    let dictionary = fake.object(class, 16);
    fake.memory.write_u64(dictionary + 16, array);
    fake.memory.write_i32(dictionary + 24, 1);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert!(matches!(mono.read_dictionary(&object(&mono, dictionary)), Err(Error::UnresolvedGeneric { .. })));
}

fn hash_set(fake: &Fake, image: u64, slots: &[(i32, i32)], slots_name: &'static str, count_name: &'static str) -> u64 {
    let slot = fake.class(
        image,
        ClassSpec::new("Slot")
            .value_type()
            .size(24)
            .field(FieldSpec::new("hashCode", type_code::I4, 16))
            .field(FieldSpec::new("next", type_code::I4, 20))
            .field(FieldSpec::new("value", type_code::I4, 24)),
    );
    let array_class = fake.array_class(slot, 12);
    let data: Vec<u8> = slots.iter().flat_map(|&(hash, value)| ints(&[hash, -1, value])).collect();
    let array = fake.array(array_class, slots.len() as i32, &data);
    let class = fake.class(image, ClassSpec::new("HashSet`1").field(FieldSpec::new(slots_name, type_code::SZARRAY, 16)).field(FieldSpec::new(count_name, type_code::I4, 24)));
    let set = fake.object(class, 16);
    fake.memory.write_u64(set + 16, array);
    fake.memory.write_i32(set + 24, slots.len() as i32);
    set
}

#[test]
fn a_hash_set_gives_its_values_and_skips_free_slots() {
    let fake = Fake::new();
    let image = fake.image(4);
    let set = hash_set(&fake, image, &[(1, 5), (-1, 6), (3, 7)], "_slots", "_count");
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let values = mono.read_hash_set(&object(&mono, set)).unwrap();
    assert_eq!(values.iter().map(|value| value.as_i64().unwrap()).collect::<Vec<_>>(), [5, 7]);
}

#[test]
fn a_hash_set_with_the_field_names_of_another_profile_is_read() {
    let fake = Fake::new();
    let image = fake.image(4);
    let set = hash_set(&fake, image, &[(2, 9)], "m_slots", "m_count");
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    assert_eq!(mono.read_hash_set(&object(&mono, set)).unwrap().len(), 1);
}

#[test]
fn a_nullable_with_a_value_gives_it_and_without_gives_none() {
    let fake = Fake::new();
    let image = fake.image(2);
    let nullable = fake.class(image, ClassSpec::new("Nullable`1").value_type().size(24).field(FieldSpec::new("hasValue", type_code::BOOLEAN, 16)).field(FieldSpec::new("value", type_code::I4, 20)));
    let with = fake.alloc(16);
    fake.memory.write_u8(with, 1);
    fake.memory.write_i32(with + 4, 99);
    let without = fake.alloc(16);
    fake.memory.write_i32(without + 4, 99);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(nullable.address).unwrap();
    let with = Struct { address: with, class: Arc::clone(&class) };
    let without = Struct { address: without, class };
    assert_eq!(mono.read_nullable(&with).unwrap().unwrap().as_i64(), Some(99));
    assert!(mono.read_nullable(&without).unwrap().is_none());
}

#[test]
fn a_nullable_with_the_has_value_spelling_of_another_profile_is_read() {
    let fake = Fake::new();
    let image = fake.image(2);
    let nullable = fake.class(image, ClassSpec::new("Nullable`1").value_type().size(24).field(FieldSpec::new("has_value", type_code::BOOLEAN, 16)).field(FieldSpec::new("value", type_code::I4, 20)));
    let at = fake.alloc(16);
    fake.memory.write_u8(at, 1);
    fake.memory.write_i32(at + 4, 5);
    let mono = Mono::new(fake.memory, OFFSETS, 0);
    let class = mono.class_at(nullable.address).unwrap();
    assert_eq!(mono.read_nullable(&Struct { address: at, class }).unwrap().unwrap().as_i64(), Some(5));
}
