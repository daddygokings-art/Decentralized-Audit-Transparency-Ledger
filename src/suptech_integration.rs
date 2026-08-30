#![no_std]

use crate::suptech_types::{RegulatoryFramework, SupervisoryReport};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Regulatory integration endpoint.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegulatoryEndpoint {
    /// Endpoint ID
    pub endpoint_id: BytesN<32>,
    /// Regulatory framework
    pub framework: u8, // RegulatoryFramework as u8
    /// Endpoint URL/address
    pub endpoint_address: Bytes,
    /// API protocol version
    pub protocol_version: u32,
    /// Last sync timestamp
    pub last_sync: u64,
    /// Is active
    pub is_active: bool,
    /// Endpoint status (connected, disconnected, error)
    pub status: u8, // EndpointStatus as u8
    /// Sync frequency (seconds)
    pub sync_frequency: u64,
}

/// Endpoint status.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EndpointStatus {
    /// Connected and operational
    Connected = 0,
    /// Disconnected but recoverable
    Disconnected = 1,
    /// Error state
    Error = 2,
    /// Maintenance/offline
    Maintenance = 3,
}

impl EndpointStatus {
    pub fn is_operational(&self) -> bool {
        matches!(self, EndpointStatus::Connected)
    }
}

/// Data transmission record to regulator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransmissionRecord {
    /// Transmission ID
    pub transmission_id: BytesN<32>,
    /// Source (institution)
    pub source: Address,
    /// Destination (regulator)
    pub destination: Address,
    /// Data type (report, feed, alert)
    pub data_type: Bytes,
    /// Data hash
    pub data_hash: BytesN<32>,
    /// Transmission timestamp
    pub transmitted_at: u64,
    /// Acknowledgment timestamp
    pub acknowledged_at: Option<u64>,
    /// Transmission status
    pub status: u8, // TransmissionStatus as u8
}

/// Transmission status.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TransmissionStatus {
    /// Pending transmission
    Pending = 0,
    /// Transmitted
    Transmitted = 1,
    /// Acknowledged by regulator
    Acknowledged = 2,
    /// Failed transmission
    Failed = 3,
    /// Retransmission scheduled
    RetransmissionScheduled = 4,
}

impl TransmissionStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransmissionStatus::Acknowledged | TransmissionStatus::Failed
        )
    }
}

/// Regulatory integration manager.
pub struct IntegrationManager;

impl IntegrationManager {
    /// Register regulatory endpoint (BIS, FSB, central bank)
    pub fn register_endpoint(
        env: &Env,
        framework: RegulatoryFramework,
        endpoint_address: Bytes,
        protocol_version: u32,
    ) -> Result<RegulatoryEndpoint, &'static str> {
        if endpoint_address.is_empty() {
            return Err("Endpoint address cannot be empty");
        }

        let endpoint_id = Self::compute_endpoint_id(env, framework);

        Ok(RegulatoryEndpoint {
            endpoint_id,
            framework: framework as u8,
            endpoint_address,
            protocol_version,
            last_sync: 0,
            is_active: true,
            status: EndpointStatus::Connected as u8,
            sync_frequency: 3600, // Default: 1 hour
        })
    }

    /// Compute endpoint ID
    pub fn compute_endpoint_id(env: &Env, framework: RegulatoryFramework) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(
            env,
            framework.as_symbol().to_string().as_bytes(),
        ));
        input.append(&Bytes::from_slice(env, b"ENDPOINT"));

        env.crypto().sha256(&input)
    }

    /// Create transmission record
    pub fn create_transmission(
        env: &Env,
        source: Address,
        destination: Address,
        data_type: Bytes,
        data_hash: BytesN<32>,
    ) -> Result<TransmissionRecord, &'static str> {
        if data_type.is_empty() {
            return Err("Data type cannot be empty");
        }

        let transmission_id = Self::compute_transmission_id(env, &source, &destination);

        Ok(TransmissionRecord {
            transmission_id,
            source,
            destination,
            data_type,
            data_hash,
            transmitted_at: env.ledger().timestamp(),
            acknowledged_at: None,
            status: TransmissionStatus::Transmitted as u8,
        })
    }

    /// Compute transmission ID
    pub fn compute_transmission_id(
        env: &Env,
        source: &Address,
        destination: &Address,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, source.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, destination.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Acknowledge transmission (regulator confirms receipt)
    pub fn acknowledge_transmission(
        env: &Env,
        transmission: &mut TransmissionRecord,
    ) -> Result<(), &'static str> {
        if transmission.status != TransmissionStatus::Transmitted as u8 {
            return Err("Transmission is not in transmitted state");
        }

        transmission.acknowledged_at = Some(env.ledger().timestamp());
        transmission.status = TransmissionStatus::Acknowledged as u8;

        Ok(())
    }

    /// Mark transmission as failed
    pub fn fail_transmission(
        env: &Env,
        transmission: &mut TransmissionRecord,
    ) -> Result<(), &'static str> {
        transmission.status = TransmissionStatus::Failed as u8;
        Ok(())
    }

    /// Schedule retransmission
    pub fn schedule_retransmission(
        transmission: &mut TransmissionRecord,
    ) -> Result<(), &'static str> {
        if transmission.status != TransmissionStatus::Failed as u8 {
            return Err("Only failed transmissions can be retried");
        }

        transmission.status = TransmissionStatus::RetransmissionScheduled as u8;
        Ok(())
    }

    /// Check if transmission is acknowledged
    pub fn is_transmission_acknowledged(transmission: &TransmissionRecord) -> bool {
        transmission.status == TransmissionStatus::Acknowledged as u8
    }

    /// Get transmission age (seconds)
    pub fn get_transmission_age(transmission: &TransmissionRecord, current_time: u64) -> u64 {
        current_time.saturating_sub(transmission.transmitted_at)
    }

    /// Get BIS Basel Committee rules for framework
    pub fn get_bis_rules() -> Vec<Bytes> {
        // Return high-level BIS compliance rules
        let env = soroban_sdk::Env::default();
        let mut rules = Vec::new(&env);

        rules.push_back(Bytes::from_slice(&env, b"Basel III capital adequacy"));
        rules.push_back(Bytes::from_slice(&env, b"Liquidity coverage ratio"));
        rules.push_back(Bytes::from_slice(&env, b"Leverage ratio"));
        rules.push_back(Bytes::from_slice(&env, b"Countercyclical buffer"));

        rules
    }

    /// Get FSB standards for framework
    pub fn get_fsb_standards() -> Vec<Bytes> {
        let env = soroban_sdk::Env::default();
        let mut standards = Vec::new(&env);

        standards.push_back(Bytes::from_slice(&env, b"OTC derivatives"));
        standards.push_back(Bytes::from_slice(&env, b"Shadow banking monitoring"));
        standards.push_back(Bytes::from_slice(&env, b"Cyber resilience"));
        standards.push_back(Bytes::from_slice(&env, b"Resolution planning"));

        standards
    }

    /// Get national regulator reporting requirements
    pub fn get_national_requirements(framework: RegulatoryFramework) -> Vec<Bytes> {
        let env = soroban_sdk::Env::default();
        let mut reqs = Vec::new(&env);

        match framework {
            RegulatoryFramework::ECB => {
                reqs.push_back(Bytes::from_slice(&env, b"SREP reporting"));
                reqs.push_back(Bytes::from_slice(&env, b"COREP framework"));
                reqs.push_back(Bytes::from_slice(&env, b"Fit and proper assessment"));
            }
            RegulatoryFramework::FED => {
                reqs.push_back(Bytes::from_slice(&env, b"Dodd-Frank reporting"));
                reqs.push_back(Bytes::from_slice(&env, b"Stress testing requirements"));
                reqs.push_back(Bytes::from_slice(&env, b"Concentration limits"));
            }
            RegulatoryFramework::PBOC => {
                reqs.push_back(Bytes::from_slice(&env, b"Macroprudential framework"));
                reqs.push_back(Bytes::from_slice(&env, b"Systemic risk monitoring"));
                reqs.push_back(Bytes::from_slice(&env, b"Capital conservation"));
            }
            RegulatoryFramework::BoE => {
                reqs.push_back(Bytes::from_slice(&env, b"PRA regulation"));
                reqs.push_back(Bytes::from_slice(&env, b"Senior Manager Regime"));
                reqs.push_back(Bytes::from_slice(&env, b"CASS requirements"));
            }
            _ => {
                reqs.push_back(Bytes::from_slice(&env, b"Standard reporting"));
            }
        }

        reqs
    }

    /// Check endpoint health
    pub fn is_endpoint_healthy(endpoint: &RegulatoryEndpoint, current_time: u64) -> bool {
        if !endpoint.is_active {
            return false;
        }

        if endpoint.status != EndpointStatus::Connected as u8 {
            return false;
        }

        let time_since_sync = current_time.saturating_sub(endpoint.last_sync);
        time_since_sync < endpoint.sync_frequency * 2 // Allow 2x sync frequency as threshold
    }

    /// Sync endpoint status
    pub fn sync_endpoint(
        env: &Env,
        endpoint: &mut RegulatoryEndpoint,
    ) -> Result<(), &'static str> {
        endpoint.last_sync = env.ledger().timestamp();
        endpoint.status = EndpointStatus::Connected as u8;

        Ok(())
    }

    /// Update endpoint status to error
    pub fn endpoint_error(endpoint: &mut RegulatoryEndpoint) {
        endpoint.status = EndpointStatus::Error as u8;
    }

    /// Check if transmission is overdue for acknowledgment
    pub fn is_acknowledgment_overdue(
        transmission: &TransmissionRecord,
        current_time: u64,
        timeout_seconds: u64,
    ) -> bool {
        if transmission.acknowledged_at.is_some() {
            return false;
        }

        let age = current_time.saturating_sub(transmission.transmitted_at);
        age > timeout_seconds
    }
}

/// Integration statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationStatistics {
    /// Total endpoints
    pub total_endpoints: u32,
    /// Active endpoints
    pub active_endpoints: u32,
    /// Total transmissions
    pub total_transmissions: u32,
    /// Acknowledged transmissions
    pub acknowledged_transmissions: u32,
    /// Failed transmissions
    pub failed_transmissions: u32,
    /// Average acknowledgment time (seconds)
    pub avg_ack_time: u64,
}

impl IntegrationStatistics {
    pub fn new() -> Self {
        IntegrationStatistics {
            total_endpoints: 0,
            active_endpoints: 0,
            total_transmissions: 0,
            acknowledged_transmissions: 0,
            failed_transmissions: 0,
            avg_ack_time: 0,
        }
    }

    pub fn transmission_success_rate(&self) -> u32 {
        if self.total_transmissions == 0 {
            return 100;
        }

        ((self.acknowledged_transmissions as u64 * 100) / self.total_transmissions as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_registration() {
        let env = soroban_sdk::Env::default();
        let address = Bytes::from_slice(&env, b"https://bis.example.com");

        let endpoint =
            IntegrationManager::register_endpoint(&env, RegulatoryFramework::BIS, address, 1)
                .unwrap();

        assert!(endpoint.is_active);
        assert_eq!(endpoint.status, EndpointStatus::Connected as u8);
    }

    #[test]
    fn test_transmission_acknowledgment() {
        let env = soroban_sdk::Env::default();
        let source = soroban_sdk::Address::generate(&env);
        let dest = soroban_sdk::Address::generate(&env);
        let data_type = Bytes::from_slice(&env, b"report");

        let mut transmission = IntegrationManager::create_transmission(
            &env,
            source,
            dest,
            data_type,
            BytesN::zero(),
        )
        .unwrap();

        assert!(IntegrationManager::acknowledge_transmission(&env, &mut transmission).is_ok());
        assert!(IntegrationManager::is_transmission_acknowledged(&transmission));
    }

    #[test]
    fn test_endpoint_health() {
        let env = soroban_sdk::Env::default();
        let address = Bytes::from_slice(&env, b"https://example.com");

        let endpoint =
            IntegrationManager::register_endpoint(&env, RegulatoryFramework::FSB, address, 1)
                .unwrap();

        assert!(IntegrationManager::is_endpoint_healthy(&endpoint, env.ledger().timestamp()));
    }

    #[test]
    fn test_bis_rules() {
        let rules = IntegrationManager::get_bis_rules();
        assert!(rules.len() > 0);
    }
}
