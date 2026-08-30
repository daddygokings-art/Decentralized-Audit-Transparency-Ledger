use soroban_sdk::{
    Address, Bytes, BytesN, Env, Symbol,
};

use crate::{Event, ProposalAction};

/// Generic build contract used by test-data factories.
pub trait Factory<T> {
    fn build(&self, env: &Env) -> T;
}

/// Composable factory behavior for generating collections of realistic test objects.
pub trait FactoryComposition {
    fn compose<T, F: Factory<T>>(&self, factory: &F, env: &Env) -> T {
        factory.build(env)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GovernanceActionKind {
    Pause,
    Unpause,
    TransferOwnership,
    SetGlobalMaxLogs,
    SetMetadataSchema,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitterSpec {
    pub address: Address,
    pub name: Bytes,
    pub role: Symbol,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaSpec {
    pub event_type: Symbol,
    pub min_length: u32,
    pub version: u32,
    pub required_fields: Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventSpec {
    pub index: u32,
    pub timestamp: u64,
    pub event_type: Symbol,
    pub category: Symbol,
    pub submitter: Address,
    pub metadata: Bytes,
    pub version: u32,
    pub parent_event_id: Option<BytesN<32>>,
}

/// Generates a realistic on-chain submitter identity for contract tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct SubmitterFactory;

impl SubmitterFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Factory<Address> for SubmitterFactory {
    fn build(&self, env: &Env) -> Address {
        Address::generate(env)
    }
}

impl Factory<SubmitterSpec> for SubmitterFactory {
    fn build(&self, env: &Env) -> SubmitterSpec {
        self.produce_spec(env, 0)
    }
}

impl SubmitterFactory {
    pub fn with_index(&self, env: &Env, index: u32) -> Address {
        let mut seed = [0u8; 32];
        seed[31] = (index % 256) as u8;
        let _ = seed;
        Address::generate(env)
    }

    pub fn produce_spec(&self, env: &Env, _index: u32) -> SubmitterSpec {
        SubmitterSpec {
            address: self.build(env),
            name: Bytes::from_slice(env, b"submitter"),
            role: Symbol::new(env, "operator"),
            active: true,
        }
    }
}

/// Generates a realistic event payload that matches the contract's on-chain schema.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventFactory;

impl EventFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Factory<Event> for EventFactory {
    fn build(&self, env: &Env) -> Event {
        self.build_with(env, None, None, None, 0, 0)
    }
}

impl Factory<EventSpec> for EventFactory {
    fn build(&self, env: &Env) -> EventSpec {
        self.produce_spec(env, 0)
    }
}

impl EventFactory {
    pub fn build_with(
        &self,
        env: &Env,
        submitter: Option<Address>,
        event_type: Option<Symbol>,
        metadata: Option<Bytes>,
        index: u32,
        timestamp: u64,
    ) -> Event {
        let submitter = submitter.unwrap_or_else(|| SubmitterFactory.build(env));
        let event_type = event_type.unwrap_or_else(|| Symbol::new(env, "payment"));
        let metadata = metadata.unwrap_or_else(|| Bytes::from_slice(env, b"event-metadata"));
        let timestamp = if timestamp == 0 { env.ledger().timestamp() } else { timestamp };
        let hash = BytesN::from_array(env, &[7u8; 32]);
        let prev_hash = BytesN::from_array(env, &[0u8; 32]);
        Event {
            index,
            timestamp,
            event_type,
            category: Symbol::new(env, "compliance"),
            submitter,
            metadata,
            sub_event_type: Some(Symbol::new(env, "segment")),
            version: 1,
            event_hash: hash,
            prev_hash,
            parent_event_id: None,
        }
    }

    pub fn produce_spec(&self, env: &Env, index: u32) -> EventSpec {
        let submitter = SubmitterFactory.build(env);
        let event_type = Symbol::new(env, "payment");
        EventSpec {
            index,
            timestamp: env.ledger().timestamp(),
            event_type,
            category: Symbol::new(env, "compliance"),
            submitter,
            metadata: Bytes::from_slice(env, b"payload"),
            version: 1,
            parent_event_id: None,
        }
    }
}

/// Produces realistic metadata validation schemas for event types.
#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaFactory;

impl SchemaFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Factory<Bytes> for SchemaFactory {
    fn build(&self, env: &Env) -> Bytes {
        Bytes::from_slice(env, &[8u8, 0, 0, 0])
    }
}

impl Factory<SchemaSpec> for SchemaFactory {
    fn build(&self, env: &Env) -> SchemaSpec {
        self.produce_spec(env, Symbol::new(env, "payment"), 8)
    }
}

impl SchemaFactory {
    pub fn build_for(&self, env: &Env, _event_type: Symbol, min_length: u32) -> Bytes {
        let mut buf = [0u8; 4];
        let len = min_length.to_le_bytes();
        buf.copy_from_slice(&len);
        Bytes::from_slice(env, &buf)
    }

    pub fn produce_spec(&self, env: &Env, event_type: Symbol, min_length: u32) -> SchemaSpec {
        SchemaSpec {
            event_type,
            min_length,
            version: 1,
            required_fields: Bytes::from_slice(env, b"data"),
        }
    }
}

/// Produces governance actions that match the contract's proposal model.
#[derive(Clone, Copy, Debug, Default)]
pub struct GovernanceActionFactory;

impl GovernanceActionFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Factory<ProposalAction> for GovernanceActionFactory {
    fn build(&self, env: &Env) -> ProposalAction {
        ProposalAction::Pause
    }
}

impl GovernanceActionFactory {
    pub fn build_for(&self, env: &Env, kind: GovernanceActionKind) -> ProposalAction {
        match kind {
            GovernanceActionKind::Pause => ProposalAction::Pause,
            GovernanceActionKind::Unpause => ProposalAction::Unpause,
            GovernanceActionKind::TransferOwnership => {
                ProposalAction::TransferOwnership(Address::generate(env))
            }
            GovernanceActionKind::SetGlobalMaxLogs => ProposalAction::SetGlobalMaxLogs(1000),
            GovernanceActionKind::SetMetadataSchema => {
                ProposalAction::SetMetadataSchema(Symbol::new(env, "payment"), Bytes::from_slice(env, &[8u8, 0, 0, 0]))
            }
        }
    }
}

/// Convenience factory that composes the default event, submitter, schema and governance factories.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestDataFactory;

impl FactoryComposition for TestDataFactory {}

impl TestDataFactory {
    pub fn new() -> Self {
        Self
    }

    pub fn submitter(&self, env: &Env) -> Address {
        SubmitterFactory.build(env)
    }

    pub fn event(&self, env: &Env) -> Event {
        EventFactory.build(env)
    }

    pub fn schema(&self, env: &Env) -> Bytes {
        SchemaFactory.build(env)
    }

    pub fn governance_action(&self, env: &Env, kind: GovernanceActionKind) -> ProposalAction {
        GovernanceActionFactory.build_for(env, kind)
    }
}

pub type DefaultFactory = TestDataFactory;

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn factories_produce_realistic_contract_test_data() {
        let env = Env::default();
        let factory = TestDataFactory::new();

        let submitter = factory.submitter(&env);
        let event = factory.event(&env);
        let schema = factory.schema(&env);
        let action = factory.governance_action(&env, GovernanceActionKind::SetGlobalMaxLogs);

        assert!(submitter.to_string().len() > 0);
        assert_eq!(event.version, 1);
        assert_eq!(event.category, Symbol::new(&env, "compliance"));
        assert_eq!(schema.len(), 4);
        assert!(matches!(action, ProposalAction::SetGlobalMaxLogs(1000)));
    }
}
