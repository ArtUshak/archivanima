use std::fmt::Display;

use itertools::Itertools;
use multimap::MultiMap;

#[derive(Clone, Debug)]
pub struct UrlQuery {
    multimap: MultiMap<String, String>,
}

#[allow(dead_code)]
impl UrlQuery {
    pub fn new() -> UrlQuery {
        UrlQuery {
            multimap: MultiMap::new(),
        }
    }

    pub fn add(&mut self, key: String, value: String) {
        self.multimap.insert(key, value);
    }

    pub fn remove(&mut self, key: &String) {
        self.multimap.remove(key);
    }

    pub fn contains_key(&mut self, key: &String) {
        self.multimap.contains_key(key);
    }
}

impl FromIterator<(String, String)> for UrlQuery {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        UrlQuery {
            multimap: MultiMap::from_iter(iter),
        }
    }
}

impl Display for UrlQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &self
                .multimap
                .flat_iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        urlencoding::encode(key),
                        urlencoding::encode(value)
                    )
                })
                .join("&"),
        )
    }
}
