//! A miniature contract exercising every type shape the extractor must handle.
//!
//! Deliberately small: the real `src/lib.rs` is the integration fixture, this
//! file is the unit-test fixture.
#![allow(clippy::all)]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec};

#[contracttype]
pub struct Inner {
    pub label: Symbol,
    pub weight: u32,
}

#[contracttype]
pub struct Outer {
    pub id: Bytes,
    pub owner: Address,
    pub children: Vec<Inner>,
    pub lookup: Option<Address>,
    pub tags: Vec<Symbol>,
}

#[contracttype]
pub enum Mode {
    Off,
    On(u32),
    Named { label: Symbol },
}

#[contract]
pub struct Mini;

#[contractimpl]
impl Mini {
    /// Store `record` and echo its identifier back.
    pub fn store(env: Env, record: Outer) -> u32 {
        env.storage().instance().set(&symbol_short!("rec"), &record);
        env.events()
            .publish((symbol_short!("audit"), symbol_short!("stored")), (env.ledger().timestamp(), record.owner.clone()));
        1
    }

    /// Read back the record stored at `index`.
    pub fn load(env: Env, index: u32) -> Option<Outer> {
        let _ = env.storage().instance().get(&symbol_short!("rec"));
        let _ = index;
        None
    }

    pub fn modes(env: Env) -> Vec<Mode> {
        let _ = env;
        Vec::new(&env)
    }

    pub fn batch(env: Env, rows: Vec<(Address, Symbol, Bytes)>) -> Vec<u32> {
        let _ = rows;
        Vec::new(&env)
    }
}
