# Lending Library
[![Travis](https://img.shields.io/travis/HarkonenBade/lending-library.svg)](https://travis-ci.org/HarkonenBade/lending-library)
[![Crates.io](https://img.shields.io/crates/v/lending-library.svg)](https://crates.io/crates/lending-library)
[![docs.rs](https://docs.rs/lending-library/badge.svg)](https://docs.rs/lending-library/)

A data store that lends temporary ownership of stored values.

This allows for access and/or mutation of independent keys in the store simultaneously.


## Example
```rust
use lending_library::*;

struct Processor;
struct Item(String);

impl Item {
    fn gen(dat: &str) -> Self { Item(dat.to_string()) }
}

impl Processor {
    fn link(&self, _first: &Item, _second: &Item) {}
}

enum Event {
    Foo {
        id: i64,
        dat: &'static str,
    },
    Bar {
        id: i64,
        o_id: i64,
        o_dat: &'static str,
    }
}

const EVTS: &[Event] = &[Event::Foo {id:1, dat:"a_val"},
                         Event::Foo {id:2, dat:"b_val"},
                         Event::Bar {id:1, o_id: 2, o_dat:"B_val"},
                         Event::Bar {id:1, o_id: 3, o_dat:"c_val"}];

struct Store {
    id_gen: Box<Iterator<Item = i64>>,
    id_to_dat: LendingLibrary<i64, Item>,
}

impl Store {
    fn new() -> Self {
        Store {
            id_gen: Box::new(0..),
            id_to_dat: LendingLibrary::new(),
        }
    }

    pub fn declare(&mut self, uid: i64, dat: &str) -> Loan<i64, Item> {
        if !self.id_to_dat.contains_key(&uid) {
            self.id_to_dat.insert(uid, Item::gen(dat));
        }
        self.id_to_dat.lend(&uid).unwrap()
    }
}

fn main() {
    let mut store = Store::new();
    let pro = Processor;
    for evt in EVTS {
        match *evt {
            Event::Foo { id, dat } => {
                store.declare(id, dat);
            }
            Event::Bar { id, o_id, o_dat } => {
                let i = store.declare(id, "");
                let o = store.declare(o_id, o_dat);
                pro.link(&i, &o);
            }
        }
    }
}
```