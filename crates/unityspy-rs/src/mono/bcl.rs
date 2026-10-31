//! The .NET collections the game keeps its state in: `List<T>`, `Dictionary<K,V>`, `HashSet<T>` and
//! `Nullable<T>`. Their field offsets are looked up by name from the class (a Mono profile may
//! call a field `_entries` or `entries`), never hard-coded. Elements are read with the concrete
//! types of the instantiated classes; a field whose type is still a generic parameter is an error.

use super::Mono;
use super::field::Field;
use super::object::{Object, Struct};
use super::value::{Array, Value};
use crate::error::{Error, Result};
use crate::mem::reader::Memory;

/// A collection is cut short after this many elements, as a guard against reading garbage.
const MAX_ELEMENTS: usize = 1 << 20;

impl<M: Memory> Mono<M> {
    /// The first of `names` that `object` has as an instance field, read.
    fn read_first_field(&self, object: &Object, names: &[&str]) -> Result<Value> {
        for name in names {
            match self.read_field(object, name) {
                Err(Error::FieldNotFound { .. }) => {}
                other => return other,
            }
        }
        Err(Error::FieldNotFound { class: object.class.full_name.clone(), field: names.join("|") })
    }

    /// The first of `names` that `class` (or a parent) has as a field.
    fn find_first_field(&self, class: &super::class::Class, names: &[&str]) -> Result<Field> {
        for name in names {
            match self.find_field(class, name, None) {
                Err(Error::FieldNotFound { .. }) => {}
                other => return other,
            }
        }
        Err(Error::FieldNotFound { class: class.full_name.clone(), field: names.join("|") })
    }

    fn element_field(&self, element: &Struct, names: &[&str]) -> Result<Value> {
        let field = self.find_first_field(&element.class, names)?;
        self.read_field_slot(&field, (element.address + field.offset as u64).wrapping_sub(super::object::HEADER))
    }

    fn collection_count(&self, object: &Object, names: &[&str]) -> Result<usize> {
        let count = self.read_first_field(object, names)?.as_i64().unwrap_or(0);
        if !(0..=MAX_ELEMENTS as i64).contains(&count) {
            return Err(Error::Implausible { what: "collection count", value: count });
        }
        Ok(count as usize)
    }

    fn collection_array(&self, object: &Object, names: &[&str]) -> Result<Option<Array>> {
        match self.read_first_field(object, names)? {
            Value::Array(array) => Ok(Some(array)),
            _ => Ok(None),
        }
    }

    /// The items of a `List<T>`: the first `_size` of the `_items` array.
    pub fn read_list(&self, list: &Object) -> Result<Vec<Value>> {
        let size = self.collection_count(list, &["_size", "size"])?;
        match self.collection_array(list, &["_items", "items"])? {
            Some(items) => self.array_items(&items, size),
            None => Ok(Vec::new()),
        }
    }

    /// The `(key, value)` pairs of a `Dictionary<K,V>`, skipping free entries (negative hash code).
    pub fn read_dictionary(&self, dictionary: &Object) -> Result<Vec<(Value, Value)>> {
        let count = self.collection_count(dictionary, &["_count", "count"])?;
        let Some(entries) = self.collection_array(dictionary, &["_entries", "entries"])? else {
            return Ok(Vec::new());
        };
        let mut pairs = Vec::new();
        for entry in self.array_items(&entries, count)? {
            let Value::Struct(entry) = entry else {
                return Err(Error::Unexpected("a dictionary entry is not a struct".into()));
            };
            if self.element_field(&entry, &["hashCode", "hash_code"])?.as_i64().unwrap_or(-1) < 0 {
                continue;
            }
            pairs.push((self.element_field(&entry, &["key"])?, self.element_field(&entry, &["value"])?));
        }
        Ok(pairs)
    }

    /// The items of a `HashSet<T>`, skipping free slots (negative hash code).
    pub fn read_hash_set(&self, set: &Object) -> Result<Vec<Value>> {
        let count = self.collection_count(set, &["_count", "m_count", "count"])?;
        let Some(slots) = self.collection_array(set, &["_slots", "m_slots", "_entries", "entries"])? else {
            return Ok(Vec::new());
        };
        let mut items = Vec::new();
        for slot in self.array_items(&slots, count)? {
            let Value::Struct(slot) = slot else {
                return Err(Error::Unexpected("a hash set slot is not a struct".into()));
            };
            if self.element_field(&slot, &["hashCode", "hash_code"])?.as_i64().unwrap_or(-1) < 0 {
                continue;
            }
            items.push(self.element_field(&slot, &["value", "Value"])?);
        }
        Ok(items)
    }

    /// The value of a `Nullable<T>` stored inline, or `None` when it has no value.
    pub fn read_nullable(&self, nullable: &Struct) -> Result<Option<Value>> {
        let has_value = self.element_field(nullable, &["hasValue", "has_value"])?;
        if has_value.as_bool() != Some(true) {
            return Ok(None);
        }
        self.element_field(nullable, &["value"]).map(Some)
    }
}

#[cfg(test)]
#[path = "bcl_tests.rs"]
mod tests;
