#[cfg(test)]
mod tests {
    use crate::privacy_training::*;
    use soroban_sdk::{vec, Address, Bytes, BytesN, Env};

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
        PrivacyTraining::initialize(env.clone(), owner.clone());
    }

    #[test]
    fn test_create_module() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::GDPRBasics as u32,
            Bytes::from_slice(&env, b"GDPR Fundamentals"),
            Bytes::from_slice(&env, b"Introduction to GDPR principles and obligations"),
            BytesN::from_array(&env, &[1u8; 32]),
            60,
            80,
            vec![&env, StaffRole::AllStaff as u32],
            true,
            12,
        );

        let module = PrivacyTraining::get_module(env.clone(), module_id);
        assert!(module.is_mandatory);
        assert_eq!(module.duration_minutes, 60);
        assert_eq!(module.refresher_months, 12);
    }

    #[test]
    fn test_assign_and_complete_training() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let employee = sample_address(&env, 2);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::DataHandling as u32,
            Bytes::from_slice(&env, b"Secure Data Handling"),
            Bytes::from_slice(&env, b"How to handle personal data securely"),
            BytesN::from_array(&env, &[2u8; 32]),
            45,
            70,
            vec![&env, StaffRole::AllStaff as u32],
            true,
            12,
        );

        let assignment_id = PrivacyTraining::assign_training(
            env.clone(),
            owner.clone(),
            module_id,
            employee.clone(),
            StaffRole::AllStaff as u32,
            1740000000,
        );

        let assignment = PrivacyTraining::get_assignment(env.clone(), assignment_id);
        assert_eq!(assignment.status, CompletionStatus::NotStarted as u32);

        PrivacyTraining::start_training(env.clone(), owner.clone(), assignment_id);

        let completion = PrivacyTraining::complete_training(
            env.clone(),
            owner,
            assignment_id,
            95,
            BytesN::from_array(&env, &[3u8; 32]),
        );

        assert_eq!(completion.score, 95);
        assert_eq!(completion.status, CompletionStatus::Completed as u32);
    }

    #[test]
    fn test_schedule_refresher() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let employee = sample_address(&env, 2);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::BreachResponse as u32,
            Bytes::from_slice(&env, b"Breach Response Procedures"),
            Bytes::from_slice(&env, b"How to respond to data breaches"),
            BytesN::from_array(&env, &[4u8; 32]),
            30,
            85,
            vec![&env, StaffRole::AllStaff as u32, StaffRole::IT as u32],
            true,
            6,
        );

        let assignment_id = PrivacyTraining::assign_training(
            env.clone(),
            owner.clone(),
            module_id,
            employee.clone(),
            StaffRole::IT as u32,
            1740000000,
        );

        PrivacyTraining::start_training(env.clone(), owner.clone(), assignment_id);
        PrivacyTraining::complete_training(
            env.clone(),
            owner.clone(),
            assignment_id,
            90,
            BytesN::from_array(&env, &[5u8; 32]),
        );

        let refreshed = PrivacyTraining::schedule_refresher(
            env.clone(),
            owner,
            assignment_id,
            1765069200,
        );

        assert_eq!(refreshed.status, CompletionStatus::NotStarted as u32);
        assert_eq!(refreshed.next_refresher_due, 1765069200);
    }

    #[test]
    fn test_role_specific_module() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let dpo_module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::PrivacyByDesign as u32,
            Bytes::from_slice(&env, b"Privacy by Design for DPOs"),
            Bytes::from_slice(&env, b"Advanced PbD principles for DPOs"),
            BytesN::from_array(&env, &[6u8; 32]),
            120,
            90,
            vec![&env, StaffRole::DPO as u32],
            true,
            12,
        );

        let dpo_module = PrivacyTraining::get_module(env.clone(), dpo_module_id);
        assert!(dpo_module.roles_required.contains(&StaffRole::DPO as u32));
        assert_eq!(dpo_module.roles_required.len(), 1);
    }

    #[test]
    fn test_get_training_stats() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let employee = sample_address(&env, 2);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::DSRHandling as u32,
            Bytes::from_slice(&env, b"Data Subject Requests"),
            Bytes::from_slice(&env, b"Handling DSRs efficiently"),
            BytesN::from_array(&env, &[7u8; 32]),
            40,
            75,
            vec![&env, StaffRole::AllStaff as u32],
            true,
            12,
        );

        PrivacyTraining::assign_training(
            env.clone(),
            owner,
            module_id,
            employee,
            StaffRole::AllStaff as u32,
            1740000000,
        );

        let stats = PrivacyTraining::get_training_stats(env.clone());
        assert_eq!(stats.total_modules, 1);
        assert_eq!(stats.total_assignments, 1);
        assert_eq!(stats.completion_rate, 0);
    }

    #[test]
    fn test_completion_record() {
        let env = create_test_env();
        let owner = sample_address(&env, 1);
        let employee = sample_address(&env, 2);

        PrivacyTraining::initialize(env.clone(), owner.clone());

        let module_id = PrivacyTraining::create_module(
            env.clone(),
            owner.clone(),
            ModuleType::InternationalTransfers as u32,
            Bytes::from_slice(&env, b"International Data Transfers"),
            Bytes::from_slice(&env, b"SCCs and adequacy decisions"),
            BytesN::from_array(&env, &[8u8; 32]),
            50,
            80,
            vec![&env, StaffRole::Legal as u32, StaffRole::DPO as u32],
            true,
            12,
        );

        let assignment_id = PrivacyTraining::assign_training(
            env.clone(),
            owner.clone(),
            module_id,
            employee.clone(),
            StaffRole::Legal as u32,
            1740000000,
        );

        PrivacyTraining::start_training(env.clone(), owner.clone(), assignment_id);
        let completion = PrivacyTraining::complete_training(
            env.clone(),
            owner,
            assignment_id,
            88,
            BytesN::from_array(&env, &[9u8; 32]),
        );

        assert_eq!(completion.module_title, Bytes::from_slice(&env, b"International Data Transfers"));
        assert_eq!(completion.employee, employee);
        assert!(completion.certificate_expires_at > 0);
    }
}
