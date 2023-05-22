use std::hash::Hash;

use linked_hash_map::LinkedHashMap;

pub trait IntoGroupLinkedHashMap<K, V> {
    fn into_group_linked_map(self) -> LinkedHashMap<K, Vec<V>>;
}

impl<K, V, T> IntoGroupLinkedHashMap<K, V> for T
where
    T: Iterator<Item = (K, V)>,
    K: Hash,
    K: Eq,
    V: Clone,
{
    fn into_group_linked_map(self) -> LinkedHashMap<K, Vec<V>> {
        let mut map: LinkedHashMap<K, Vec<V>> = LinkedHashMap::new();
        for (key, value) in self {
            map.entry(key)
                .and_modify(|v| v.push(value.clone()))
                .or_insert(vec![value]);
        }
        map
    }
}
