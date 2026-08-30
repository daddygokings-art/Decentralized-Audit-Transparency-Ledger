//! Tests for the on-chain inverted-index module (Issue #388).
//!
//! All tests use the Soroban test environment (`Env::default()`).
//! The module is not wired into the live contract; it is exercised
//! through its public functions directly.

#[cfg(test)]
mod inverted_index_tests {
    use soroban_sdk::{bytes, testutils::Address as _, Address, Bytes, BytesN, Env, Symbol, Vec};

    use crate::inverted_index::{
        index_add_entry, index_event_metadata, index_get_count, index_query, IndexKey,
        INDEX_MAX_ENTRIES,
    };

    // ── Helper: build a deterministic 32-byte hash from a seed byte ──────────

    fn hash32(b: u8) -> [u8; 32] {
        let mut arr = [0u8; 32];
        arr[0] = b;
        arr
    }

    fn bytes_n32(env: &Env, b: u8) -> BytesN<32> {
        BytesN::<32>::from_array(env, &hash32(b))
    }

    // ── Test 1: basic add + query ─────────────────────────────────────────────

    #[test]
    fn test_basic_add_and_query() {
        let env = Env::default();
        let cat = Symbol::new(&env, "finance");
        let etype = Symbol::new(&env, "payment");
        let key = IndexKey::CategoryTypeIndex(cat.clone(), etype.clone());

        // Initially empty.
        let result = index_query(&env, key.clone());
        assert_eq!(result.len(), 0);

        // Add three indices.
        index_add_entry(&env, key.clone(), 0u32);
        index_add_entry(&env, key.clone(), 5u32);
        index_add_entry(&env, key.clone(), 99u32);

        let result = index_query(&env, key.clone());
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).unwrap(), 0u32);
        assert_eq!(result.get(1).unwrap(), 5u32);
        assert_eq!(result.get(2).unwrap(), 99u32);
    }

    // ── Test 2: zero-entry query returns empty vec ────────────────────────────

    #[test]
    fn test_zero_entry_query_returns_empty() {
        let env = Env::default();
        let sym = Symbol::new(&env, "nonexistent");
        let key = IndexKey::SubEventTypeIndex(sym);
        let result = index_query(&env, key);
        assert_eq!(result.len(), 0);
    }

    // ── Test 3: index_get_count matches stored entries ────────────────────────

    #[test]
    fn test_get_count_matches_entries() {
        let env = Env::default();
        let sym = Symbol::new(&env, "withdrawal");
        let key = IndexKey::SubEventTypeIndex(sym.clone());

        assert_eq!(index_get_count(&env, key.clone()), 0);

        for i in 0..10u32 {
            index_add_entry(&env, key.clone(), i);
        }
        assert_eq!(index_get_count(&env, key), 10);
    }

    // ── Test 4: FIFO eviction at capacity ────────────────────────────────────

    #[test]
    fn test_fifo_eviction_at_capacity() {
        let env = Env::default();
        let sym = Symbol::new(&env, "evict");
        let key = IndexKey::SubEventTypeIndex(sym.clone());

        // Fill to exactly INDEX_MAX_ENTRIES.
        for i in 0..INDEX_MAX_ENTRIES {
            index_add_entry(&env, key.clone(), i);
        }
        assert_eq!(index_get_count(&env, key.clone()), INDEX_MAX_ENTRIES);

        // Add one more — should evict the oldest (index 0).
        index_add_entry(&env, key.clone(), INDEX_MAX_ENTRIES);
        assert_eq!(index_get_count(&env, key.clone()), INDEX_MAX_ENTRIES);

        let result = index_query(&env, key.clone());
        // The first entry should now be 1, not 0.
        assert_eq!(result.get(0).unwrap(), 1u32);
        // The last entry should be INDEX_MAX_ENTRIES.
        assert_eq!(
            result.get(INDEX_MAX_ENTRIES - 1).unwrap(),
            INDEX_MAX_ENTRIES
        );
    }

    // ── Test 5: multiple distinct field indexes are independent ───────────────

    #[test]
    fn test_multiple_field_indexes_independent() {
        let env = Env::default();

        let field_a = Symbol::new(&env, "amount");
        let field_b = Symbol::new(&env, "currency");
        let hash_a = bytes_n32(&env, 1);
        let hash_b = bytes_n32(&env, 2);

        let key_a = IndexKey::MetadataFieldIndex(field_a.clone(), hash_a.clone());
        let key_b = IndexKey::MetadataFieldIndex(field_b.clone(), hash_b.clone());

        index_add_entry(&env, key_a.clone(), 10u32);
        index_add_entry(&env, key_a.clone(), 20u32);
        index_add_entry(&env, key_b.clone(), 30u32);

        let result_a = index_query(&env, key_a);
        let result_b = index_query(&env, key_b);

        assert_eq!(result_a.len(), 2);
        assert_eq!(result_b.len(), 1);
        assert_eq!(result_a.get(0).unwrap(), 10u32);
        assert_eq!(result_b.get(0).unwrap(), 30u32);
    }

    // ── Test 6: CategoryTypeIndex correctly combined ──────────────────────────

    #[test]
    fn test_category_type_combined_index() {
        let env = Env::default();

        let cat1 = Symbol::new(&env, "finance");
        let cat2 = Symbol::new(&env, "supply");
        let etype = Symbol::new(&env, "transfer");

        let key1 = IndexKey::CategoryTypeIndex(cat1.clone(), etype.clone());
        let key2 = IndexKey::CategoryTypeIndex(cat2.clone(), etype.clone());

        index_add_entry(&env, key1.clone(), 1u32);
        index_add_entry(&env, key1.clone(), 2u32);
        index_add_entry(&env, key2.clone(), 3u32);

        let r1 = index_query(&env, key1);
        let r2 = index_query(&env, key2);

        // Finance/transfer has 2, supply/transfer has 1.
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2.get(0).unwrap(), 3u32);
    }

    // ── Test 7: SubmitterTypeIndex ────────────────────────────────────────────

    #[test]
    fn test_submitter_type_index() {
        let env = Env::default();
        let submitter: Address = Address::generate(&env);
        let etype = Symbol::new(&env, "audit");

        let key = IndexKey::SubmitterTypeIndex(submitter.clone(), etype.clone());

        index_add_entry(&env, key.clone(), 7u32);
        index_add_entry(&env, key.clone(), 42u32);

        let result = index_query(&env, key);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(0).unwrap(), 7u32);
        assert_eq!(result.get(1).unwrap(), 42u32);
    }

    // ── Test 8: index_event_metadata parses key=value pairs ──────────────────

    #[test]
    fn test_index_event_metadata_parses_fields() {
        let env = Env::default();
        let submitter: Address = Address::generate(&env);
        let etype = Symbol::new(&env, "payment");
        let category = Symbol::new(&env, "finance");
        let sub_type: Option<Symbol> = Some(Symbol::new(&env, "wire"));

        // metadata: amount=1000;currency=USD
        let metadata_str = b"amount=1000;currency=USD";
        let mut metadata = Bytes::new(&env);
        for &b in metadata_str.iter() {
            let single = bytes!(&env, [b]);
            metadata.append(&single);
        }

        index_event_metadata(
            &env,
            0u32,
            &etype,
            &category,
            &submitter,
            &sub_type,
            &metadata,
        );

        // CategoryTypeIndex should have entry 0.
        let cat_key =
            IndexKey::CategoryTypeIndex(category.clone(), etype.clone());
        let cat_result = index_query(&env, cat_key);
        assert_eq!(cat_result.len(), 1);
        assert_eq!(cat_result.get(0).unwrap(), 0u32);

        // SubmitterTypeIndex should have entry 0.
        let sub_key =
            IndexKey::SubmitterTypeIndex(submitter.clone(), etype.clone());
        let sub_result = index_query(&env, sub_key);
        assert_eq!(sub_result.len(), 1);

        // SubEventTypeIndex should have entry 0.
        let setype_key = IndexKey::SubEventTypeIndex(Symbol::new(&env, "wire"));
        let setype_result = index_query(&env, setype_key);
        assert_eq!(setype_result.len(), 1);

        // MetadataFieldIndex for "amount" value "1000" should have entry 0.
        let val_bytes = {
            let s = b"1000";
            let mut b = Bytes::new(&env);
            for &byte in s.iter() {
                let single = bytes!(&env, [byte]);
                b.append(&single);
            }
            b
        };
        let val_hash: BytesN<32> = env.crypto().sha256(&val_bytes);
        let field_key =
            IndexKey::MetadataFieldIndex(Symbol::new(&env, "amount"), val_hash);
        let field_result = index_query(&env, field_key);
        assert_eq!(field_result.len(), 1);
        assert_eq!(field_result.get(0).unwrap(), 0u32);
    }

    // ── Test 9: empty metadata does not panic ─────────────────────────────────

    #[test]
    fn test_empty_metadata_no_panic() {
        let env = Env::default();
        let submitter: Address = Address::generate(&env);
        let etype = Symbol::new(&env, "noop");
        let category = Symbol::new(&env, "admin");
        let metadata = Bytes::new(&env);

        index_event_metadata(
            &env,
            0u32,
            &etype,
            &category,
            &submitter,
            &None,
            &metadata,
        );

        // CategoryTypeIndex still gets entry even with empty metadata.
        let key = IndexKey::CategoryTypeIndex(category, etype);
        let result = index_query(&env, key);
        assert_eq!(result.len(), 1);
    }

    // ── Test 10: duplicate entries allowed (append-only semantics) ────────────

    #[test]
    fn test_duplicate_entries_allowed() {
        let env = Env::default();
        let sym = Symbol::new(&env, "dup");
        let key = IndexKey::SubEventTypeIndex(sym);

        // Append the same index multiple times.
        index_add_entry(&env, key.clone(), 5u32);
        index_add_entry(&env, key.clone(), 5u32);
        index_add_entry(&env, key.clone(), 5u32);

        let result = index_query(&env, key);
        assert_eq!(result.len(), 3);
        for i in 0..3 {
            assert_eq!(result.get(i).unwrap(), 5u32);
        }
    }

    // ── Test 11: FIFO eviction removes exactly the oldest entries ─────────────

    #[test]
    fn test_fifo_eviction_removes_oldest() {
        let env = Env::default();
        let sym = Symbol::new(&env, "fifo2");
        let key = IndexKey::SubEventTypeIndex(sym);

        // Add INDEX_MAX_ENTRIES entries with values 0..INDEX_MAX_ENTRIES-1.
        for i in 0..INDEX_MAX_ENTRIES {
            index_add_entry(&env, key.clone(), i);
        }

        // Add 5 more entries — should evict the 5 oldest (0..4).
        for extra in 0..5u32 {
            index_add_entry(&env, key.clone(), INDEX_MAX_ENTRIES + extra);
        }

        assert_eq!(index_get_count(&env, key.clone()), INDEX_MAX_ENTRIES);

        let result = index_query(&env, key);
        // First remaining entry should be 5.
        assert_eq!(result.get(0).unwrap(), 5u32);
        // Last entry should be INDEX_MAX_ENTRIES + 4.
        assert_eq!(
            result.get(INDEX_MAX_ENTRIES - 1).unwrap(),
            INDEX_MAX_ENTRIES + 4
        );
    }
}
