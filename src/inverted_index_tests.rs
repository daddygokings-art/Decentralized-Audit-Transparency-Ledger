//! Comprehensive tests for the on-chain inverted index (Issue #359).
//!
//! Tests cover:
//! - Keyword extraction: delimiters, lowercasing, length filters, stop words
//! - Index configuration and per-type governance flags
//! - Exact keyword match search
//! - Prefix/partial match behavior (not supported — documented)
//! - Pagination (page/page_size)
//! - Empty results for missing keywords
//! - FIFO bucket eviction at capacity
//! - reindex_events: backfill, type-disabled rebuild, batch limiting
//! - remove_event_from_index cleanup
//! - Aggregate statistics tracking
//! - Special characters and non-ASCII handling
//! - Backward-compat: LegacyIndexKey (Issue #388) API unchanged
//! - Case-insensitive matching
//! - Duplicate keyword deduplication within a single event

#[cfg(test)]
mod inverted_index_tests {
    use crate::inverted_index::{
        extract_keywords, get_event_keywords, get_index_config, get_index_stats, index_add_entry, index_event,
        index_event_metadata, index_get_count, index_query, init_index_config, is_indexing_enabled_for_type,
        keyword_entry_count, keyword_hash, reindex_events, remove_event_from_index, search_events_by_keyword,
        set_index_type_enabled, IndexConfig, LegacyIndexKey, DEFAULT_BUCKET_MAX_ENTRIES,
        DEFAULT_MAX_KEYWORDS_PER_EVENT, MAX_REINDEX_BATCH, MAX_SEARCH_PAGE_SIZE,
    };
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Bytes, BytesN, Env, Symbol, Vec};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn env() -> Env {
        Env::default()
    }

    fn meta(env: &Env, s: &[u8]) -> Bytes {
        Bytes::from_slice(env, s)
    }

    fn kw(env: &Env, s: &[u8]) -> Bytes {
        Bytes::from_slice(env, s)
    }

    fn default_config() -> IndexConfig {
        IndexConfig::default_config()
    }

    // ── 1. extract_keywords tests ─────────────────────────────────────────────

    #[test]
    fn test_extract_keywords_basic_space_split() {
        let env = env();
        let m = meta(&env, b"payment transfer wire");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        assert_eq!(hashes.len(), 3);
    }

    #[test]
    fn test_extract_keywords_semicolon_delimiter() {
        let env = env();
        let m = meta(&env, b"amount=1000;currency=USD;type=wire");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        // Tokens after splitting on = and ;: "amount","1000","currency","usd","type","wire"
        // Filtered by length >= 3: all 6 pass
        assert!(hashes.len() >= 4);
    }

    #[test]
    fn test_extract_keywords_lowercased() {
        let env = env();
        // "PAYMENT" and "payment" should produce the same hash
        let m1 = meta(&env, b"PAYMENT");
        let m2 = meta(&env, b"payment");
        let h1 = extract_keywords(&env, &m1, 3, 32, 20);
        let h2 = extract_keywords(&env, &m2, 3, 32, 20);
        assert_eq!(h1.len(), 1);
        assert_eq!(h2.len(), 1);
        assert_eq!(h1.get(0).unwrap(), h2.get(0).unwrap());
    }

    #[test]
    fn test_extract_keywords_min_length_filter() {
        let env = env();
        // "ab" is 2 bytes; with min=3 it should be filtered
        let m = meta(&env, b"ab pay transfer");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        // "ab" filtered, "pay" and "transfer" pass
        assert_eq!(hashes.len(), 2);
    }

    #[test]
    fn test_extract_keywords_max_length_filter() {
        let env = env();
        // 35-byte word exceeds max_len=32
        let m = meta(&env, b"averylongkeywordthatexceedsthirtytwocharacters payment");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        // Only "payment" passes (long token is capped at 32 bytes and discarded)
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_extract_keywords_stop_words_filtered() {
        let env = env();
        // Stop words: "the", "a", "an", "in", "of", "at", "to", "is", "it"
        let m = meta(&env, b"the payment is at the bank");
        let hashes = extract_keywords(&env, &m, 2, 32, 20);
        // "the", "is", "at", "the" are stop words; "payment", "bank" remain
        let len = hashes.len();
        assert!(len <= 2); // at most 2 non-stop words
        assert!(len >= 1);
    }

    #[test]
    fn test_extract_keywords_max_keywords_cap() {
        let env = env();
        // 10 distinct words; cap at 5
        let m = meta(&env, b"alpha bravo charlie delta echo foxtrot golf hotel india juliet");
        let hashes = extract_keywords(&env, &m, 3, 32, 5);
        assert_eq!(hashes.len(), 5);
    }

    #[test]
    fn test_extract_keywords_deduplication() {
        let env = env();
        // "payment" repeated 3 times; should appear only once
        let m = meta(&env, b"payment payment payment");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_extract_keywords_empty_metadata() {
        let env = env();
        let m = Bytes::new(&env);
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        assert_eq!(hashes.len(), 0);
    }

    #[test]
    fn test_extract_keywords_special_chars_non_printable() {
        let env = env();
        // Non-printable bytes should be skipped, not crash
        let m = Bytes::from_slice(&env, &[0x01, 0x02, b'p', b'a', b'y', 0x7f]);
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        // "pay" is 3 chars; should be extracted (min=3)
        assert_eq!(hashes.len(), 1);
    }

    #[test]
    fn test_extract_keywords_only_delimiters() {
        let env = env();
        let m = meta(&env, b"=====;;;  ,,");
        let hashes = extract_keywords(&env, &m, 3, 32, 20);
        assert_eq!(hashes.len(), 0);
    }

    // ── 2. keyword_hash consistency ───────────────────────────────────────────

    #[test]
    fn test_keyword_hash_deterministic() {
        let env = env();
        let h1 = keyword_hash(&env, b"payment");
        let h2 = keyword_hash(&env, b"payment");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_keyword_hash_different_words_differ() {
        let env = env();
        let h1 = keyword_hash(&env, b"payment");
        let h2 = keyword_hash(&env, b"transfer");
        assert_ne!(h1, h2);
    }

    // ── 3. index_event tests ──────────────────────────────────────────────────

    #[test]
    fn test_index_event_stores_keywords() {
        let env = env();
        init_index_config(&env, default_config());
        let event_type = symbol_short!("payment");
        let m = meta(&env, b"wire transfer bank");

        index_event(&env, 0, &event_type, &m);

        let kws = get_event_keywords(&env, 0);
        assert!(kws.len() > 0);
    }

    #[test]
    fn test_index_event_keyword_searchable() {
        let env = env();
        init_index_config(&env, default_config());
        let event_type = symbol_short!("payment");
        let m = meta(&env, b"wire transfer");

        index_event(&env, 42, &event_type, &m);

        let results = search_events_by_keyword(&env, &kw(&env, b"wire"), 0, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results.get(0).unwrap().event_index, 42);
    }

    #[test]
    fn test_index_event_multiple_events_same_keyword() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 0, &et, &meta(&env, b"wire transfer payment"));
        index_event(&env, 1, &et, &meta(&env, b"bank wire transfer"));
        index_event(&env, 2, &et, &meta(&env, b"wire ACH payment"));

        // "wire" appears in all 3 events
        let results = search_events_by_keyword(&env, &kw(&env, b"wire"), 0, 10);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_index_event_disabled_globally_skips_indexing() {
        let env = env();
        let mut config = default_config();
        config.indexing_enabled = false;
        init_index_config(&env, config);

        let et = symbol_short!("pay");
        index_event(&env, 0, &et, &meta(&env, b"payment wire transfer"));

        // Nothing should be indexed
        let results = search_events_by_keyword(&env, &kw(&env, b"wire"), 0, 10);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_index_event_disabled_per_type_skips() {
        let env = env();
        init_index_config(&env, default_config());

        let et = symbol_short!("audit");
        set_index_type_enabled(&env, et.clone(), false);

        index_event(&env, 0, &et, &meta(&env, b"audit compliance check"));

        let results = search_events_by_keyword(&env, &kw(&env, b"compliance"), 0, 10);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_index_event_enabled_per_type_when_global_on() {
        let env = env();
        init_index_config(&env, default_config());

        let et = symbol_short!("pay");
        set_index_type_enabled(&env, et.clone(), true);

        index_event(&env, 5, &et, &meta(&env, b"payment authorized"));

        let results = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_index_event_stats_updated() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 0, &et, &meta(&env, b"wire transfer"));
        index_event(&env, 1, &et, &meta(&env, b"bank deposit"));

        let stats = get_index_stats(&env);
        assert_eq!(stats.total_indexed_events, 2);
        assert!(stats.total_index_entries >= 2);
    }

    // ── 4. search_events_by_keyword tests ────────────────────────────────────

    #[test]
    fn test_search_exact_match() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 10, &et, &meta(&env, b"cryptocurrency payment wallet"));

        let r = search_events_by_keyword(&env, &kw(&env, b"cryptocurrency"), 0, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(0).unwrap().event_index, 10);
    }

    #[test]
    fn test_search_case_insensitive() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // Index with lowercase
        index_event(&env, 7, &et, &meta(&env, b"payment transfer"));

        // Search with uppercase
        let r = search_events_by_keyword(&env, &kw(&env, b"PAYMENT"), 0, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(0).unwrap().event_index, 7);
    }

    #[test]
    fn test_search_empty_keyword_returns_empty() {
        let env = env();
        init_index_config(&env, default_config());
        let r = search_events_by_keyword(&env, &Bytes::new(&env), 0, 10);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_search_keyword_not_indexed_returns_empty() {
        let env = env();
        init_index_config(&env, default_config());
        let r = search_events_by_keyword(&env, &kw(&env, b"nonexistentword"), 0, 10);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_search_page_size_zero_returns_empty() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");
        index_event(&env, 0, &et, &meta(&env, b"payment wire"));
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 0);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_search_page_out_of_range_returns_empty() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // Only 1 event indexed → page 1 (with page_size=10) is out of range
        index_event(&env, 0, &et, &meta(&env, b"payment wire transfer"));
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 1, 10);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_search_pagination_correct_slicing() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // Index 5 events all containing "wire"
        for i in 0..5u32 {
            index_event(&env, i, &et, &meta(&env, b"wire transfer payment"));
        }

        // Page 0, size 2 → events 0,1
        let r0 = search_events_by_keyword(&env, &kw(&env, b"wire"), 0, 2);
        assert_eq!(r0.len(), 2);
        assert_eq!(r0.get(0).unwrap().event_index, 0);
        assert_eq!(r0.get(1).unwrap().event_index, 1);

        // Page 1, size 2 → events 2,3
        let r1 = search_events_by_keyword(&env, &kw(&env, b"wire"), 1, 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r1.get(0).unwrap().event_index, 2);
        assert_eq!(r1.get(1).unwrap().event_index, 3);

        // Page 2, size 2 → event 4 only
        let r2 = search_events_by_keyword(&env, &kw(&env, b"wire"), 2, 2);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2.get(0).unwrap().event_index, 4);
    }

    #[test]
    fn test_search_page_size_capped_at_max() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // Index 200 events
        for i in 0..200u32 {
            index_event(&env, i, &et, &meta(&env, b"payment wire transfer"));
        }

        // Requesting 200 should be capped at MAX_SEARCH_PAGE_SIZE
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 200);
        assert_eq!(r.len() as u32, MAX_SEARCH_PAGE_SIZE);
    }

    #[test]
    fn test_search_keyword_entry_count() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 0, &et, &meta(&env, b"payment transfer wire"));
        index_event(&env, 1, &et, &meta(&env, b"payment bank"));

        // "payment" appears in 2 events
        let count = keyword_entry_count(&env, &kw(&env, b"payment"));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_search_partial_match_not_supported() {
        // The index only supports exact keyword matches (after tokenisation).
        // "pay" does NOT match "payment" — document this as intended behavior.
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("evt");

        index_event(&env, 0, &et, &meta(&env, b"payment transfer"));

        // "pay" is not a keyword in this metadata (only "payment" and "transfer")
        let r = search_events_by_keyword(&env, &kw(&env, b"pay"), 0, 10);
        assert_eq!(r.len(), 0);

        // Exact "payment" does match
        let r2 = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r2.len(), 1);
    }

    // ── 5. Governance flag tests ──────────────────────────────────────────────

    #[test]
    fn test_governance_per_type_flag_default_is_true() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");
        // Not explicitly set → defaults to enabled
        assert!(is_indexing_enabled_for_type(&env, &et));
    }

    #[test]
    fn test_governance_per_type_flag_can_be_disabled() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");
        set_index_type_enabled(&env, et.clone(), false);
        assert!(!is_indexing_enabled_for_type(&env, &et));
    }

    #[test]
    fn test_governance_per_type_flag_can_be_re_enabled() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        set_index_type_enabled(&env, et.clone(), false);
        assert!(!is_indexing_enabled_for_type(&env, &et));

        set_index_type_enabled(&env, et.clone(), true);
        assert!(is_indexing_enabled_for_type(&env, &et));
    }

    #[test]
    fn test_governance_global_disabled_overrides_per_type_enabled() {
        let env = env();
        let mut config = default_config();
        config.indexing_enabled = false;
        init_index_config(&env, config);

        let et = symbol_short!("pay");
        set_index_type_enabled(&env, et.clone(), true);

        // Global disabled → per-type enabled doesn't matter
        assert!(!is_indexing_enabled_for_type(&env, &et));
    }

    #[test]
    fn test_index_config_persisted_and_readable() {
        let env = env();
        let config = IndexConfig {
            indexing_enabled: true,
            min_keyword_len: 4,
            max_keyword_len: 20,
            max_keywords_per_event: 10,
            bucket_max_entries: 500,
        };
        init_index_config(&env, config.clone());

        let loaded = get_index_config(&env);
        assert_eq!(loaded.min_keyword_len, 4);
        assert_eq!(loaded.max_keyword_len, 20);
        assert_eq!(loaded.max_keywords_per_event, 10);
        assert_eq!(loaded.bucket_max_entries, 500);
    }

    // ── 6. FIFO bucket eviction ───────────────────────────────────────────────

    #[test]
    fn test_fifo_eviction_on_bucket_overflow() {
        let env = env();
        // Small bucket cap for test
        let config = IndexConfig {
            indexing_enabled: true,
            min_keyword_len: 3,
            max_keyword_len: 32,
            max_keywords_per_event: 20,
            bucket_max_entries: 5,
        };
        init_index_config(&env, config);

        let et = symbol_short!("pay");
        // Index 7 events with the same keyword → should evict oldest 2
        for i in 0..7u32 {
            let m = meta(&env, b"payment transfer wire");
            index_event(&env, i, &et, &m);
        }

        // Bucket capped at 5; oldest (0,1) should be evicted
        let count = keyword_entry_count(&env, &kw(&env, b"payment"));
        assert_eq!(count, 5);

        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r.len(), 5);
        // First remaining entry should be event 2
        assert_eq!(r.get(0).unwrap().event_index, 2);
        // Last remaining entry should be event 6
        assert_eq!(r.get(4).unwrap().event_index, 6);
    }

    // ── 7. reindex_events tests ───────────────────────────────────────────────

    #[test]
    fn test_reindex_events_builds_index() {
        let env = env();
        init_index_config(&env, default_config());

        // Simulate 3 stored events; initially not indexed
        // (no prior call to index_event)
        let events: [(Symbol, Bytes); 3] = [
            (symbol_short!("pay"), meta(&env, b"wire payment transfer")),
            (symbol_short!("pay"), meta(&env, b"bank deposit payment")),
            (symbol_short!("pay"), meta(&env, b"ACH payment batch")),
        ];

        let loader =
            |idx: u32| -> Option<(Symbol, Bytes)> { events.get(idx as usize).map(|(et, m)| (et.clone(), m.clone())) };

        let next = reindex_events(&env, 0, 3, loader);
        assert_eq!(next, 3);

        // All 3 events should now be searchable by "payment"
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn test_reindex_events_overwrites_existing_index() {
        let env = env();
        init_index_config(&env, default_config());

        let et = symbol_short!("pay");

        // First: index event 0 with "oldword"
        index_event(&env, 0, &et, &meta(&env, b"oldword data"));

        // Verify "oldword" is indexed
        let r1 = search_events_by_keyword(&env, &kw(&env, b"oldword"), 0, 10);
        assert_eq!(r1.len(), 1);

        // Reindex event 0 with new metadata (no "oldword")
        let loader = |idx: u32| -> Option<(Symbol, Bytes)> {
            if idx == 0 {
                Some((et.clone(), meta(&env, b"newword transfer")))
            } else {
                None
            }
        };
        reindex_events(&env, 0, 1, &loader);

        // "oldword" should no longer find event 0
        let r2 = search_events_by_keyword(&env, &kw(&env, b"oldword"), 0, 10);
        assert_eq!(r2.len(), 0);

        // "newword" should find event 0
        let r3 = search_events_by_keyword(&env, &kw(&env, b"newword"), 0, 10);
        assert_eq!(r3.len(), 1);
    }

    #[test]
    fn test_reindex_events_respects_batch_limit() {
        let env = env();
        init_index_config(&env, default_config());

        // Request reindex of 100 events but MAX_REINDEX_BATCH is 50
        let loader =
            |idx: u32| -> Option<(Symbol, Bytes)> { Some((symbol_short!("pay"), meta(&env, b"payment transfer"))) };

        let next = reindex_events(&env, 0, 100, loader);
        // Should stop after MAX_REINDEX_BATCH events
        assert_eq!(next, MAX_REINDEX_BATCH);
    }

    #[test]
    fn test_reindex_events_skips_disabled_types() {
        let env = env();
        init_index_config(&env, default_config());

        let disabled_type = symbol_short!("skip");
        set_index_type_enabled(&env, disabled_type.clone(), false);

        let loader = |idx: u32| -> Option<(Symbol, Bytes)> {
            if idx < 3 {
                Some((disabled_type.clone(), meta(&env, b"payment transfer")))
            } else {
                None
            }
        };

        reindex_events(&env, 0, 3, loader);

        // Disabled type → no index entries
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn test_reindex_stats_updated() {
        let env = env();
        init_index_config(&env, default_config());

        let loader = |idx: u32| -> Option<(Symbol, Bytes)> {
            if idx < 5 {
                Some((symbol_short!("pay"), meta(&env, b"wire payment")))
            } else {
                None
            }
        };

        reindex_events(&env, 0, 5, loader);

        let stats = get_index_stats(&env);
        assert_eq!(stats.last_reindex_event, 5);
    }

    // ── 8. remove_event_from_index tests ─────────────────────────────────────

    #[test]
    fn test_remove_event_from_index() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 0, &et, &meta(&env, b"wire payment transfer"));
        index_event(&env, 1, &et, &meta(&env, b"wire payment transfer"));

        // Event 0 searchable
        let r1 = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r1.len(), 2);

        remove_event_from_index(&env, 0);

        // Event 0 no longer in results; event 1 still there
        let r2 = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2.get(0).unwrap().event_index, 1);
    }

    #[test]
    fn test_remove_nonindexed_event_is_no_op() {
        let env = env();
        init_index_config(&env, default_config());
        // Should not panic
        remove_event_from_index(&env, 999);
    }

    // ── 9. Aggregate statistics tests ─────────────────────────────────────────

    #[test]
    fn test_stats_initial_zero() {
        let env = env();
        let stats = get_index_stats(&env);
        assert_eq!(stats.total_indexed_events, 0);
        assert_eq!(stats.total_index_entries, 0);
        assert_eq!(stats.total_unique_keywords, 0);
    }

    #[test]
    fn test_stats_entries_incremented_per_index_event() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // "payment" → 1 unique keyword, 1 entry
        index_event(&env, 0, &et, &meta(&env, b"payment"));

        let stats = get_index_stats(&env);
        assert_eq!(stats.total_indexed_events, 1);
        assert!(stats.total_index_entries >= 1);
        assert!(stats.total_unique_keywords >= 1);
    }

    // ── 10. Special characters and edge cases ─────────────────────────────────

    #[test]
    fn test_metadata_with_only_short_tokens_no_results() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // All tokens shorter than min_len=3
        index_event(&env, 0, &et, &meta(&env, b"ab cd ef"));

        // No keywords should be indexed
        let stats = get_index_stats(&env);
        assert_eq!(stats.total_indexed_events, 0); // index_event returns early
    }

    #[test]
    fn test_metadata_with_numbers_as_keywords() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        index_event(&env, 0, &et, &meta(&env, b"txid 12345 confirmed"));

        // "12345" and "txid" and "confirmed" should all be indexed
        let r = search_events_by_keyword(&env, &kw(&env, b"12345"), 0, 10);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn test_multiple_event_types_independent_indexes() {
        let env = env();
        init_index_config(&env, default_config());

        let et1 = symbol_short!("pay");
        let et2 = symbol_short!("audit");

        index_event(&env, 0, &et1, &meta(&env, b"payment confirmed wire"));
        index_event(&env, 1, &et2, &meta(&env, b"compliance audit payment"));

        // "payment" appears in both events (different types)
        let r = search_events_by_keyword(&env, &kw(&env, b"payment"), 0, 10);
        assert_eq!(r.len(), 2);
    }

    // ── 11. Backward-compat: Legacy Issue #388 API ────────────────────────────

    #[test]
    fn test_legacy_index_add_and_query() {
        let env = env();
        let cat = Symbol::new(&env, "finance");
        let etype = Symbol::new(&env, "payment");
        let key = LegacyIndexKey::CategoryTypeIndex(cat.clone(), etype.clone());

        let result = index_query(&env, key.clone());
        assert_eq!(result.len(), 0);

        index_add_entry(&env, key.clone(), 0u32);
        index_add_entry(&env, key.clone(), 5u32);
        index_add_entry(&env, key.clone(), 99u32);

        let result = index_query(&env, key.clone());
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).unwrap(), 0u32);
        assert_eq!(result.get(1).unwrap(), 5u32);
        assert_eq!(result.get(2).unwrap(), 99u32);
    }

    #[test]
    fn test_legacy_get_count() {
        let env = env();
        let sym = Symbol::new(&env, "withdrawal");
        let key = LegacyIndexKey::SubEventTypeIndex(sym.clone());

        assert_eq!(index_get_count(&env, key.clone()), 0);
        for i in 0..10u32 {
            index_add_entry(&env, key.clone(), i);
        }
        assert_eq!(index_get_count(&env, key), 10);
    }

    #[test]
    fn test_legacy_fifo_eviction() {
        let env = env();
        let sym = Symbol::new(&env, "evict");
        let key = LegacyIndexKey::SubEventTypeIndex(sym.clone());

        for i in 0..DEFAULT_BUCKET_MAX_ENTRIES {
            index_add_entry(&env, key.clone(), i);
        }
        assert_eq!(index_get_count(&env, key.clone()), DEFAULT_BUCKET_MAX_ENTRIES);

        index_add_entry(&env, key.clone(), DEFAULT_BUCKET_MAX_ENTRIES);
        assert_eq!(index_get_count(&env, key.clone()), DEFAULT_BUCKET_MAX_ENTRIES);

        let result = index_query(&env, key.clone());
        assert_eq!(result.get(0).unwrap(), 1u32);
        assert_eq!(
            result.get(DEFAULT_BUCKET_MAX_ENTRIES - 1).unwrap(),
            DEFAULT_BUCKET_MAX_ENTRIES
        );
    }

    #[test]
    fn test_legacy_index_event_metadata_parses_fields() {
        let env = env();
        let submitter: Address = Address::generate(&env);
        let etype = Symbol::new(&env, "payment");
        let category = Symbol::new(&env, "finance");
        let sub_type: Option<Symbol> = Some(Symbol::new(&env, "wire"));

        let metadata = Bytes::from_slice(&env, b"amount=1000;currency=USD");

        index_event_metadata(&env, 0u32, &etype, &category, &submitter, &sub_type, &metadata);

        let cat_key = LegacyIndexKey::CategoryTypeIndex(category.clone(), etype.clone());
        let cat_result = index_query(&env, cat_key);
        assert_eq!(cat_result.len(), 1);

        let sub_key = LegacyIndexKey::SubmitterTypeIndex(submitter.clone(), etype.clone());
        let sub_result = index_query(&env, sub_key);
        assert_eq!(sub_result.len(), 1);

        let setype_key = LegacyIndexKey::SubEventTypeIndex(Symbol::new(&env, "wire"));
        let setype_result = index_query(&env, setype_key);
        assert_eq!(setype_result.len(), 1);
    }

    #[test]
    fn test_legacy_empty_metadata_no_panic() {
        let env = env();
        let submitter: Address = Address::generate(&env);
        let etype = Symbol::new(&env, "noop");
        let category = Symbol::new(&env, "admin");
        let metadata = Bytes::new(&env);

        index_event_metadata(&env, 0u32, &etype, &category, &submitter, &None, &metadata);

        let key = LegacyIndexKey::CategoryTypeIndex(category, etype);
        let result = index_query(&env, key);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_legacy_multiple_field_indexes_independent() {
        let env = env();
        let field_a = Symbol::new(&env, "amount");
        let field_b = Symbol::new(&env, "currency");

        let val_a = Bytes::from_slice(&env, b"1000");
        let val_b = Bytes::from_slice(&env, b"USD");
        let hash_a: BytesN<32> = env.crypto().sha256(&val_a);
        let hash_b: BytesN<32> = env.crypto().sha256(&val_b);

        let key_a = LegacyIndexKey::MetadataFieldIndex(field_a.clone(), hash_a.clone());
        let key_b = LegacyIndexKey::MetadataFieldIndex(field_b.clone(), hash_b.clone());

        index_add_entry(&env, key_a.clone(), 10u32);
        index_add_entry(&env, key_a.clone(), 20u32);
        index_add_entry(&env, key_b.clone(), 30u32);

        let result_a = index_query(&env, key_a);
        let result_b = index_query(&env, key_b);

        assert_eq!(result_a.len(), 2);
        assert_eq!(result_b.len(), 1);
    }

    // ── 12. Storage cost benchmark documentation ──────────────────────────────

    /// Documents the index storage cost with default settings.
    ///
    /// With defaults: max 20 keywords × 4 bytes = 80 bytes per event.
    /// 1 000 events → ~80 KB bucket data + ~20 KB EventKeywords entries.
    /// Total overhead: ~100 KB per 1 000 events (acceptable for on-chain state).
    ///
    /// Without indexing: 0 additional bytes.
    #[test]
    fn test_storage_cost_with_indexing_baseline() {
        let env = env();
        init_index_config(&env, default_config());
        let et = symbol_short!("pay");

        // Index 10 events with up to 20 keywords each
        for i in 0..10u32 {
            index_event(
                &env,
                i,
                &et,
                &meta(
                    &env,
                    b"payment transfer wire bank deposit confirmation receipt invoice approved authorized",
                ),
            );
        }

        let stats = get_index_stats(&env);
        // Each event contributes keywords; verify total entries is tracked
        assert!(stats.total_index_entries > 0);
        assert_eq!(stats.total_indexed_events, 10);

        // Each keyword bucket append: 4 bytes
        // Storage cost per event: min(20, extracted_keywords) × 4 bytes = max 80 bytes
        // 10 events: max 800 bytes of bucket data
        // This is well within Soroban's instance storage limits
        let max_expected_entries: u64 = 10 * DEFAULT_MAX_KEYWORDS_PER_EVENT as u64;
        assert!(stats.total_index_entries <= max_expected_entries);
    }
}
