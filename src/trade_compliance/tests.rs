#[cfg(test)]
mod tests {
    use crate::trade_compliance::*;
    use soroban_sdk::{vec, Address, Bytes, Env};

    fn create_test_env() -> Env {
        Env::default()
    }

    fn sample_address(env: &Env, _id: u32) -> Address {
        Address::generate(env)
    }

    #[test]
    fn test_initialize() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        TradeCompliance::initialize(env.clone(), owner.clone());
    }

    #[test]
    fn test_register_hs_code() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        TradeCompliance::initialize(env.clone(), owner.clone());

        let code_id = TradeCompliance::register_hs_code(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"8471.30.00"),
            Bytes::from_slice(&env, b"Portable automatic data processing machines"),
            Bytes::from_slice(&env, b"Electronics"),
            Bytes::from_slice(&env, b"UNIT"),
            500, // 5% duty
            Bytes::from_slice(&env, b"Computers"),
        );

        let classification = TradeCompliance::get_hs_code(env.clone(), code_id);
        assert_eq!(classification.base_duty_rate, 500);
    }

    #[test]
    fn test_determine_origin() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let trader = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let shipment_id = BytesN::from_array(&env, &[1u8; 32]);
        let origin_id = TradeCompliance::determine_origin(
            env.clone(),
            trader.clone(),
            shipment_id,
            Bytes::from_slice(&env, b"Computers"),
            Bytes::from_slice(&env, b"8471.30.00"),
            Bytes::from_slice(&env, b"US"),
            0, // fully originating
            100,
        );

        let origin = TradeCompliance::get_origin(env.clone(), origin_id);
        assert_eq!(origin.origin_type, 0);
    }

    #[test]
    fn test_qualify_for_fta() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let trader = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let shipment_id = BytesN::from_array(&env, &[2u8; 32]);
        let fta_id = TradeCompliance::qualify_for_fta(
            env.clone(),
            trader.clone(),
            shipment_id,
            Bytes::from_slice(&env, b"USMCA"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"MX"),
            Bytes::from_slice(&env, b"6204.62.00"),
            true,  // qualifies
            true,  // ROO satisfied
        );

        let fta = TradeCompliance::get_fta_qualification(env.clone(), fta_id);
        assert!(fta.qualifies);
        assert!(fta.roo_satisfied);
    }

    #[test]
    fn test_valuate_for_customs() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let trader = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let shipment_id = BytesN::from_array(&env, &[3u8; 32]);
        let valuation_id = TradeCompliance::valuate_for_customs(
            env.clone(),
            trader.clone(),
            shipment_id,
            10000u64, // $100.00
            Bytes::from_slice(&env, b"USD"),
            1,        // transaction value method
            500,      // +$5.00 freight
        );

        let valuation = TradeCompliance::get_valuation(env.clone(), valuation_id);
        assert_eq!(valuation.dutiable_value, 10500u64);
    }

    #[test]
    fn test_issue_trade_license() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let holder = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let license_id = TradeCompliance::issue_trade_license(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"IMP-2024-001"),
            holder.clone(),
            vec![&env, Bytes::from_slice(&env, b"Electronics")],
            vec![&env, Bytes::from_slice(&env, b"US"), Bytes::from_slice(&env, b"MX")],
            365,
        );

        let license = TradeCompliance::get_license(env.clone(), license_id);
        assert_eq!(license.status, 0); // active
    }

    #[test]
    fn test_register_customs_broker() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let broker = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let broker_id = TradeCompliance::register_customs_broker(
            env.clone(),
            owner.clone(),
            broker.clone(),
            Bytes::from_slice(&env, b"ABC Customs Brokers"),
            Bytes::from_slice(&env, b"BROKER-001"),
            vec![&env, Bytes::from_slice(&env, b"US"), Bytes::from_slice(&env, b"CA"), Bytes::from_slice(&env, b"MX")],
            365,
        );

        let broker_profile = TradeCompliance::get_broker(env.clone(), broker_id);
        assert!(broker_profile.is_active);
    }

    #[test]
    fn test_certify_aeo() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let entity = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let aeo_id = TradeCompliance::certify_aeo(
            env.clone(),
            owner.clone(),
            entity.clone(),
            Bytes::from_slice(&env, b"XYZ Importers Ltd"),
            Bytes::from_slice(&env, b"C-TPAT"),
            3, // enhanced security
            365,
        );

        let aeo = TradeCompliance::get_aeo_certification(env.clone(), aeo_id);
        assert_eq!(aeo.status, 0); // active
        assert_eq!(aeo.security_level, 3);
    }

    #[test]
    fn test_issue_certificate_of_origin() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);
        let importer = sample_address(&env, 3);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let shipment_id = BytesN::from_array(&env, &[4u8; 32]);
        let coo_id = TradeCompliance::issue_certificate_of_origin(
            env.clone(),
            exporter.clone(),
            shipment_id,
            importer.clone(),
            Bytes::from_slice(&env, b"6204.62.00"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"USMCA"),
            Bytes::from_slice(&env, b"COO-2024-001"),
        );

        let coo = TradeCompliance::get_certificate_of_origin(env.clone(), coo_id);
        assert_eq!(coo.exporter, exporter);
    }

    #[test]
    fn test_calculate_duty() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let trader = sample_address(&env, 2);

        TradeCompliance::initialize(env.clone(), owner.clone());

        let shipment_id = BytesN::from_array(&env, &[5u8; 32]);
        let duty_id = TradeCompliance::calculate_duty(
            env.clone(),
            trader.clone(),
            shipment_id,
            10000u64, // $100.00
            500,      // 5% base duty
            200,      // 2% FTA duty
            );

        let duty = TradeCompliance::get_duty_calculation(env.clone(), duty_id);
        assert_eq!(duty.calculated_duty, 500u64);  // $5.00
        assert_eq!(duty.fta_duty, 200u64);        // $2.00
        assert_eq!(duty.duty_savings, 300u64);    // $3.00 saved
    }

    #[test]
    fn test_get_trade_compliance_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        TradeCompliance::initialize(env.clone(), owner.clone());

        TradeCompliance::register_hs_code(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"8471.30.00"),
            Bytes::from_slice(&env, b"Portable computers"),
            Bytes::from_slice(&env, b"Electronics"),
            Bytes::from_slice(&env, b"UNIT"),
            500,
            Bytes::from_slice(&env, b"Computers"),
        );

        let (hs_codes, licenses, brokers, aeos, trades) =
            TradeCompliance::get_trade_compliance_stats(env.clone());

        assert_eq!(hs_codes, 1);
        assert_eq!(licenses, 0);
    }

    #[test]
    fn test_full_trade_workflow() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let exporter = sample_address(&env, 2);
        let importer = sample_address(&env, 3);
        let broker = sample_address(&env, 4);

        TradeCompliance::initialize(env.clone(), owner.clone());

        // 1. Register HS code
        let _code_id = TradeCompliance::register_hs_code(
            env.clone(),
            owner.clone(),
            Bytes::from_slice(&env, b"6204.62.00"),
            Bytes::from_slice(&env, b"Women's trousers"),
            Bytes::from_slice(&env, b"Apparel"),
            Bytes::from_slice(&env, b"PAIR"),
            1200, // 12% duty
            Bytes::from_slice(&env, b"Textiles"),
        );

        // 2. Determine origin
        let shipment_id = BytesN::from_array(&env, &[6u8; 32]);
        let _origin_id = TradeCompliance::determine_origin(
            env.clone(),
            exporter.clone(),
            shipment_id,
            Bytes::from_slice(&env, b"Women's trousers"),
            Bytes::from_slice(&env, b"6204.62.00"),
            Bytes::from_slice(&env, b"US"),
            0, // fully originating
            100,
        );

        // 3. Qualify for FTA
        let _fta_id = TradeCompliance::qualify_for_fta(
            env.clone(),
            exporter.clone(),
            shipment_id,
            Bytes::from_slice(&env, b"USMCA"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"MX"),
            Bytes::from_slice(&env, b"6204.62.00"),
            true,
            true,
        );

        // 4. Valuate for customs
        let _valuation_id = TradeCompliance::valuate_for_customs(
            env.clone(),
            exporter.clone(),
            shipment_id,
            50000u64, // $500.00
            Bytes::from_slice(&env, b"USD"),
            1,
            2000, // +$20 freight
        );

        // 5. Register broker
        let _broker_id = TradeCompliance::register_customs_broker(
            env.clone(),
            owner.clone(),
            broker.clone(),
            Bytes::from_slice(&env, b"FastClear Customs"),
            Bytes::from_slice(&env, b"BROKER-TX-001"),
            vec![&env, Bytes::from_slice(&env, b"US"), Bytes::from_slice(&env, b"MX")],
            365,
        );

        // 6. Issue Certificate of Origin
        let _coo_id = TradeCompliance::issue_certificate_of_origin(
            env.clone(),
            exporter.clone(),
            shipment_id,
            importer.clone(),
            Bytes::from_slice(&env, b"6204.62.00"),
            Bytes::from_slice(&env, b"US"),
            Bytes::from_slice(&env, b"USMCA"),
            Bytes::from_slice(&env, b"COO-TX-2024-0001"),
        );

        // 7. Calculate duty
        let _duty_id = TradeCompliance::calculate_duty(
            env.clone(),
            broker.clone(),
            shipment_id,
            52000u64, // dutiable value
            1200,     // 12% base
            300,      // 3% FTA rate
        );

        // 8. Verify workflow
        let (hs_codes, licenses, brokers, aeos, _trades) =
            TradeCompliance::get_trade_compliance_stats(env.clone());

        assert_eq!(hs_codes, 1);
        assert_eq!(brokers, 1);
    }
}
