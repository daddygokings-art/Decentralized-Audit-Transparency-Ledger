/// # Export Controls & Sanctions Compliance Module
///
/// Comprehensive export controls and sanctions compliance framework implementing
/// OFAC (Office of Foreign Assets Control), EU sanctions, UN Security Council
/// restrictions, and BIS (Bureau of Industry and Security) export control
/// regulations. Includes denied party screening, end-use checks, license
/// determination, re-export controls, controlled commodities tracking, and
/// automated screening with real-time risk flagging.
///
/// ## Regulatory Framework
/// - **OFAC** — U.S. Office of Foreign Assets Control SDN lists
/// - **BIS EAR** — Bureau of Industry and Security Export Administration Regulations
/// - **BIS CCL** — Commerce Control List (EEE, encryption, military items, etc.)
/// - **EU Sanctions** — European Union consolidated list
/// - **UN Security Council** — UN sanctions lists

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ExportControlsError {
    /// Denied party match in OFAC/EU/UN lists
    DeniedPartyDetected = 3000,
    /// End-use check failed or suspicious end-use
    EndUseCheckFailed = 3001,
    /// License required but not present
    LicenseRequired = 3002,
    /// License expired or invalid
    InvalidLicense = 3003,
    /// Re-export of controlled item prohibited
    ReExportProhibited = 3004,
    /// Destination country restricted for commodity
    RestrictedDestination = 3005,
    /// Controlled commodity not allowed
    ControlledCommodity = 3006,
    /// Sanctioned end-use detected (military, nuclear, etc.)
    SanctionedEndUse = 3007,
    /// Entity on multiple restricted lists
    MultipleListMatches = 3008,
    /// Transaction blocked by screening
    TransactionBlocked = 3009,
    /// Screening database not initialized
    ScreeningDatabaseUninitialized = 3010,
    /// Export classification unknown
    UnknownExportClass = 3011,
    /// Deemed export (transfer of controlled technology) prohibited
    DeemedExportProhibited = 3012,
    /// Country group restrictions violated
    CountryGroupRestricted = 3013,
    /// Encryption level exceeds limits
    EncryptionLevelExceeded = 3014,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// Sanctioning authority type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum SanctioningAuthority {
    /// OFAC (U.S. Office of Foreign Assets Control)
    OFAC = 1,
    /// EU (European Union sanctions)
    EU = 2,
    /// UN (United Nations Security Council)
    UN = 3,
    /// BIS (Bureau of Industry and Security)
    BIS = 4,
    /// DDTC (Directorate of Defense Trade Controls)
    DDTC = 5,
    /// CATSEARCH (Consolidated Allied Trades Search)
    CATSEARCH = 6,
}

/// Export control category/regulation type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ExportControlRegulation {
    /// EAR (Export Administration Regulations)
    EAR = 1,
    /// ITAR (International Traffic in Arms Regulations)
    ITAR = 2,
    /// ECEU (EU export control)
    ECEU = 3,
    /// Encryption controls
    Encryption = 4,
    /// Fundamental research exemption
    FundamentalResearch = 5,
}

/// Country group classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum CountryGroup {
    /// Group A - Allies (NATO, Japan, Korea, etc.)
    GroupA = 1,
    /// Group B - Advanced countries
    GroupB = 2,
    /// Group D - Other countries
    GroupD = 3,
    /// Group E - Embargo countries (Cuba, Iran, Syria, DPRK)
    GroupE = 4,
    /// Not in any group / Unknown
    Unknown = 5,
}

/// License determination type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum LicenseType {
    /// No license required (NLR)
    NoLicenseRequired = 0,
    /// License required
    LicenseRequired = 1,
    /// License exception applicable (e.g., LVS, GFE)
    LicenseException = 2,
    /// License prohibited (NLR)
    LicenseProhibited = 3,
    /// License unknown/pending classification
    Unknown = 4,
}

/// Denied party match record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeniedPartyMatch {
    /// Match ID
    pub id: BytesN<32>,
    /// Party being screened
    pub party: Address,
    /// Authority that listed the party
    pub authority: u32, // SanctioningAuthority
    /// Party name in list
    pub listed_name: Bytes,
    /// Match confidence (0-100)
    pub confidence: u32,
    /// Detection timestamp
    pub detected_at: u64,
    /// List entry hash
    pub list_entry_hash: BytesN<32>,
    /// Additional identifiers (names, addresses, etc.)
    pub identifiers: Vec<Bytes>,
}

/// Export license record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportLicense {
    /// License ID
    pub id: BytesN<32>,
    /// Exporter address
    pub exporter: Address,
    /// License type (issued by BIS/DDTC)
    pub license_type: Bytes, // e.g., "DEPT-COMMERCE-12345"
    /// Issued date
    pub issued_date: u64,
    /// Expiration date
    pub expiration_date: u64,
    /// Controlled items authorized
    pub items_authorized: Vec<Bytes>,
    /// Destination countries
    pub destination_countries: Vec<Bytes>,
    /// End-use statement
    pub end_use_statement: Bytes,
    /// Authorized end-user
    pub authorized_end_user: Address,
    /// License value/quantity limit
    pub quantity_limit: u64,
    /// License status (0=active, 1=suspended, 2=revoked, 3=expired)
    pub status: u32,
    /// License hash
    pub license_hash: BytesN<32>,
}

/// Controlled commodity record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledCommodity {
    /// Commodity ID
    pub id: BytesN<32>,
    /// Commodity name/description
    pub name: Bytes,
    /// ECCN (Export Control Classification Number) or HS code
    pub eccn: Bytes,
    /// Control status (encryption, military, etc.)
    pub control_type: Bytes,
    /// Regulated by (BIS, DDTC, etc.)
    pub regulated_by: u32,
    /// Restricted countries (vector of country codes)
    pub restricted_countries: Vec<Bytes>,
    /// License requirement
    pub license_requirement: u32, // LicenseType
    /// Technical data classification
    pub technical_data_restricted: bool,
    /// Encryption level (bits)
    pub encryption_level: u32,
    /// Is deemed export (transfer of tech to foreign national)
    pub is_deemed_export: bool,
    /// Last updated
    pub updated_at: u64,
    /// Commodity hash
    pub commodity_hash: BytesN<32>,
}

/// End-use check record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EndUseCheck {
    /// Check ID
    pub id: BytesN<32>,
    /// Commodity/item being checked
    pub commodity: Bytes,
    /// Declared end-use
    pub declared_end_use: Bytes,
    /// End-user address
    pub end_user: Address,
    /// Final destination country
    pub destination_country: Bytes,
    /// Check timestamp
    pub checked_at: u64,
    /// Check result (0=cleared, 1=failed, 2=pending, 3=escalated)
    pub result: u32,
    /// Risk flags identified
    pub risk_flags: Vec<Bytes>,
    /// Suspicious pattern indicators
    pub suspicious_patterns: Vec<Bytes>,
    /// Check details hash
    pub check_hash: BytesN<32>,
}

/// Re-export record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReExportRecord {
    /// Re-export ID
    pub id: BytesN<32>,
    /// Original exporter
    pub original_exporter: Address,
    /// Re-exporter
    pub re_exporter: Address,
    /// Commodity details
    pub commodity: Bytes,
    /// Original destination (first buyer)
    pub original_destination: Bytes,
    /// New destination
    pub new_destination: Bytes,
    /// Original license used
    pub original_license: BytesN<32>,
    /// Re-export authorization required
    pub authorization_required: bool,
    /// Re-export approved
    pub approved: bool,
    /// Approval timestamp
    pub approved_at: u64,
    /// Re-export record hash
    pub record_hash: BytesN<32>,
}

/// Denied party list entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeniedPartyListEntry {
    /// Entry ID
    pub id: BytesN<32>,
    /// Entity name
    pub entity_name: Bytes,
    /// Alternative names
    pub alt_names: Vec<Bytes>,
    /// Address
    pub address: Bytes,
    /// Country
    pub country: Bytes,
    /// Authority listing
    pub authority: u32, // SanctioningAuthority
    /// Reason for listing
    pub reason: Bytes,
    /// Effective date
    pub effective_date: u64,
    /// Entry hash for verification
    pub entry_hash: BytesN<32>,
}

/// Screening result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreeningResult {
    /// Screening ID
    pub id: BytesN<32>,
    /// Party screened
    pub party: Address,
    /// Commodity involved
    pub commodity: Bytes,
    /// Destination
    pub destination: Bytes,
    /// Screening timestamp
    pub screened_at: u64,
    /// Overall result (0=cleared, 1=blocked, 2=escalated)
    pub result: u32,
    /// Matches found
    pub matches_found: u32,
    /// Risk score (0-100)
    pub risk_score: u32,
    /// License needed
    pub license_needed: bool,
    /// End-use check required
    pub end_use_check_required: bool,
    /// Screening details hash
    pub screening_hash: BytesN<32>,
}

// ── Data Keys ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ExportControlsKey {
    /// Contract owner/administrator
    Owner,
    /// Denied party list entries by ID
    DeniedPartyEntry(BytesN<32>),
    /// Denied party matches by party address
    DeniedPartyMatchesByParty(Address),
    /// Denied party entry lookup by name hash
    DeniedPartyByName(BytesN<32>),
    /// Export licenses by ID
    ExportLicense(BytesN<32>),
    /// Licenses by exporter
    LicensesByExporter(Address),
    /// Controlled commodities by ID
    ControlledCommodity(BytesN<32>),
    /// Commodity by ECCN
    CommodityByECCN(Bytes),
    /// End-use checks by ID
    EndUseCheck(BytesN<32>),
    /// End-use checks by commodity
    EndUseChecksByCommodity(Bytes),
    /// Re-export records by ID
    ReExportRecord(BytesN<32>),
    /// Re-export records by re-exporter
    ReExportsByExporter(Address),
    /// Screening results by ID
    ScreeningResult(BytesN<32>),
    /// Screening results by party
    ScreeningsByParty(Address),
    /// Country restrictions by country code
    CountryRestrictions(Bytes),
    /// Country group classification
    CountryGroup(Bytes),
    /// Total denied party entries
    DeniedPartyCount,
    /// Total licenses
    LicenseCount,
    /// Total commodities
    CommodityCount,
    /// Total end-use checks
    EndUseCheckCount,
    /// Total re-exports
    ReExportCount,
    /// Total screenings
    ScreeningCount,
    /// Total blocked transactions
    BlockedTransactionCount,
    /// Last OFAC list update
    OFACLastUpdate,
    /// Last EU list update
    EULastUpdate,
    /// Last UN list update
    UNLastUpdate,
    /// High-risk destinations registry
    HighRiskDestination(Bytes),
    /// Deemed export notifications (transfer of tech)
    DeemedExportAlert(Address),
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct ExportControls;

#[contractimpl]
impl ExportControls {
    /// Initialize export controls module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&ExportControlsKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&ExportControlsKey::DeniedPartyCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::LicenseCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::CommodityCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::EndUseCheckCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::ReExportCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::ScreeningCount, &0u32);
        env.storage()
            .instance()
            .set(&ExportControlsKey::BlockedTransactionCount, &0u32);
    }

    // ── Denied Party Management ──────────────────────────────────────────

    /// Add denied party entry (OFAC/EU/UN list)
    pub fn add_denied_party(
        env: Env,
        caller: Address,
        entity_name: Bytes,
        alt_names: Vec<Bytes>,
        address: Bytes,
        country: Bytes,
        authority: u32,
        reason: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let entry_id = Self::compute_entry_id(&env, &entity_name, &address);
        let now = env.ledger().timestamp();

        let entry = DeniedPartyListEntry {
            id: entry_id.clone(),
            entity_name: entity_name.clone(),
            alt_names: alt_names.clone(),
            address,
            country,
            authority,
            reason,
            effective_date: now,
            entry_hash: env.crypto().sha256(&entity_name).into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::DeniedPartyEntry(entry_id.clone()), &entry);

        let name_hash = env.crypto().sha256(&entity_name).into();
        env.storage()
            .instance()
            .set(&ExportControlsKey::DeniedPartyByName(name_hash), &entry_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::DeniedPartyCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::DeniedPartyCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "denied_party_added"),),
            (entry_id.clone(), entity_name, authority),
        );

        entry_id
    }

    /// Screen party against denied party lists
    pub fn screen_denied_party(
        env: Env,
        caller: Address,
        party: Address,
        party_name: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();

        let match_id = Self::compute_match_id(&env, &party, &party_name);
        let now = env.ledger().timestamp();

        // Check against denied party names
        let name_hash = env.crypto().sha256(&party_name).into();
        let mut found_match = false;
        let mut authority = 1u32;

        if let Some(entry_id) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&ExportControlsKey::DeniedPartyByName(name_hash))
        {
            if let Some(entry) = env
                .storage()
                .instance()
                .get::<_, DeniedPartyListEntry>(&ExportControlsKey::DeniedPartyEntry(entry_id))
            {
                found_match = true;
                authority = entry.authority;
            }
        }

        if found_match {
            let denied_match = DeniedPartyMatch {
                id: match_id.clone(),
                party: party.clone(),
                authority,
                listed_name: party_name.clone(),
                confidence: 95u32, // High confidence exact match
                detected_at: now,
                list_entry_hash: env.crypto().sha256(&party_name).into(),
                identifiers: vec![&env, party_name.clone()],
            };

            env.storage()
                .instance()
                .set(&ExportControlsKey::DeniedPartyMatchesByParty(party.clone()), &match_id);

            let count: u32 = env
                .storage()
                .instance()
                .get(&ExportControlsKey::BlockedTransactionCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&ExportControlsKey::BlockedTransactionCount, &(count + 1));

            env.events().publish(
                (Symbol::new(&env, "denied_party_match"),),
                (match_id.clone(), party, authority),
            );

            panic_with_error!(&env, ExportControlsError::DeniedPartyDetected);
        }

        match_id
    }

    /// Get denied party match
    pub fn get_denied_party_match(env: Env, party: Address) -> DeniedPartyMatch {
        if let Some(match_id) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&ExportControlsKey::DeniedPartyMatchesByParty(party.clone()))
        {
            // Return match info (would be stored)
            panic_with_error!(&env, ExportControlsError::DeniedPartyDetected);
        }
        panic_with_error!(&env, ExportControlsError::ScreeningDatabaseUninitialized)
    }

    // ── Export License Management ────────────────────────────────────────

    /// Issue export license
    pub fn issue_export_license(
        env: Env,
        caller: Address,
        exporter: Address,
        license_type: Bytes,
        items_authorized: Vec<Bytes>,
        destination_countries: Vec<Bytes>,
        end_use_statement: Bytes,
        authorized_end_user: Address,
        quantity_limit: u64,
        validity_days: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let license_id = Self::compute_license_id(&env, &license_type, &exporter);
        let now = env.ledger().timestamp();

        let license = ExportLicense {
            id: license_id.clone(),
            exporter: exporter.clone(),
            license_type,
            issued_date: now,
            expiration_date: now + (validity_days as u64 * 86400),
            items_authorized,
            destination_countries,
            end_use_statement,
            authorized_end_user,
            quantity_limit,
            status: 0, // active
            license_hash: env
                .crypto()
                .sha256(&Self::pack_license_data(&env, &exporter, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::ExportLicense(license_id.clone()), &license);

        env.storage().instance().set(
            &ExportControlsKey::LicensesByExporter(exporter.clone()),
            &license_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::LicenseCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::LicenseCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "export_license_issued"),),
            (license_id.clone(), exporter, quantity_limit),
        );

        license_id
    }

    /// Get export license
    pub fn get_export_license(env: Env, license_id: BytesN<32>) -> ExportLicense {
        env.storage()
            .instance()
            .get(&ExportControlsKey::ExportLicense(license_id))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::LicenseRequired))
    }

    /// Verify license validity
    pub fn verify_license(
        env: Env,
        license_id: BytesN<32>,
        commodity: Bytes,
        destination: Bytes,
    ) -> bool {
        let license = Self::get_export_license(env.clone(), license_id);

        // Check expiration
        if license.expiration_date < env.ledger().timestamp() {
            panic_with_error!(&env, ExportControlsError::InvalidLicense);
        }

        // Check status
        if license.status != 0 {
            panic_with_error!(&env, ExportControlsError::InvalidLicense);
        }

        // Check destination
        let mut destination_allowed = false;
        for i in 0..license.destination_countries.len() {
            if license.destination_countries.get(i).unwrap() == &destination {
                destination_allowed = true;
                break;
            }
        }

        if !destination_allowed {
            panic_with_error!(&env, ExportControlsError::RestrictedDestination);
        }

        true
    }

    // ── Controlled Commodity Management ──────────────────────────────────

    /// Register controlled commodity
    pub fn register_commodity(
        env: Env,
        caller: Address,
        name: Bytes,
        eccn: Bytes,
        control_type: Bytes,
        regulated_by: u32,
        restricted_countries: Vec<Bytes>,
        license_requirement: u32,
        technical_data_restricted: bool,
        encryption_level: u32,
        is_deemed_export: bool,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let commodity_id = Self::compute_commodity_id(&env, &eccn);
        let now = env.ledger().timestamp();

        let commodity = ControlledCommodity {
            id: commodity_id.clone(),
            name,
            eccn: eccn.clone(),
            control_type,
            regulated_by,
            restricted_countries,
            license_requirement,
            technical_data_restricted,
            encryption_level,
            is_deemed_export,
            updated_at: now,
            commodity_hash: env.crypto().sha256(&eccn).into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::ControlledCommodity(commodity_id.clone()), &commodity);

        env.storage()
            .instance()
            .set(&ExportControlsKey::CommodityByECCN(eccn.clone()), &commodity_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::CommodityCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::CommodityCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "commodity_registered"),),
            (commodity_id.clone(), eccn, license_requirement),
        );

        commodity_id
    }

    /// Get controlled commodity
    pub fn get_commodity(env: Env, commodity_id: BytesN<32>) -> ControlledCommodity {
        env.storage()
            .instance()
            .get(&ExportControlsKey::ControlledCommodity(commodity_id))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::UnknownExportClass))
    }

    // ── End-Use Checks ──────────────────────────────────────────────────

    /// Perform end-use check
    pub fn check_end_use(
        env: Env,
        caller: Address,
        commodity: Bytes,
        declared_end_use: Bytes,
        end_user: Address,
        destination_country: Bytes,
    ) -> BytesN<32> {
        caller.require_auth();

        let check_id = Self::compute_check_id(&env, &commodity, &end_user);
        let now = env.ledger().timestamp();

        let mut result = 0u32; // cleared
        let mut risk_flags: Vec<Bytes> = Vec::new(&env);
        let mut suspicious_patterns: Vec<Bytes> = Vec::new(&env);

        // Flag suspicious end-uses
        if Self::is_suspicious_end_use(&env, &declared_end_use) {
            result = 1; // failed
            risk_flags.push_back(Bytes::from_slice(&env, b"SUSPICIOUS_END_USE"));
            suspicious_patterns.push_back(declared_end_use.clone());
        }

        // Check military/dual-use end-uses
        if Self::is_military_end_use(&env, &declared_end_use) {
            result = 3; // escalated
            risk_flags.push_back(Bytes::from_slice(&env, b"MILITARY_END_USE"));
        }

        let end_use_check = EndUseCheck {
            id: check_id.clone(),
            commodity,
            declared_end_use,
            end_user,
            destination_country,
            checked_at: now,
            result,
            risk_flags,
            suspicious_patterns,
            check_hash: env
                .crypto()
                .sha256(&Self::pack_check_data(&env, &end_user, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::EndUseCheck(check_id.clone()), &end_use_check);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::EndUseCheckCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::EndUseCheckCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "end_use_check_performed"),),
            (check_id.clone(), end_user, result),
        );

        if result == 1 {
            panic_with_error!(&env, ExportControlsError::EndUseCheckFailed);
        }

        check_id
    }

    /// Get end-use check
    pub fn get_end_use_check(env: Env, check_id: BytesN<32>) -> EndUseCheck {
        env.storage()
            .instance()
            .get(&ExportControlsKey::EndUseCheck(check_id))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::EndUseCheckFailed))
    }

    // ── Re-Export Controls ───────────────────────────────────────────────

    /// Record re-export transaction
    pub fn record_re_export(
        env: Env,
        caller: Address,
        re_exporter: Address,
        original_exporter: Address,
        commodity: Bytes,
        original_destination: Bytes,
        new_destination: Bytes,
        original_license: BytesN<32>,
    ) -> BytesN<32> {
        caller.require_auth();

        // Verify original license
        let _ = Self::verify_license(
            env.clone(),
            original_license.clone(),
            commodity.clone(),
            original_destination.clone(),
        );

        // Check if re-export authorization needed
        let authorization_needed = Self::is_re_export_restricted(&env, &new_destination);

        let re_export_id = Self::compute_re_export_id(&env, &re_exporter, &commodity);
        let now = env.ledger().timestamp();

        let re_export = ReExportRecord {
            id: re_export_id.clone(),
            original_exporter,
            re_exporter: re_exporter.clone(),
            commodity,
            original_destination,
            new_destination,
            original_license,
            authorization_required: authorization_needed,
            approved: !authorization_needed,
            approved_at: if !authorization_needed { now } else { 0 },
            record_hash: env
                .crypto()
                .sha256(&Self::pack_re_export_data(&env, &re_exporter, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::ReExportRecord(re_export_id.clone()), &re_export);

        env.storage().instance().set(
            &ExportControlsKey::ReExportsByExporter(re_exporter.clone()),
            &re_export_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::ReExportCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::ReExportCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "re_export_recorded"),),
            (re_export_id.clone(), re_exporter, authorization_needed),
        );

        if authorization_needed {
            panic_with_error!(&env, ExportControlsError::ReExportProhibited);
        }

        re_export_id
    }

    /// Approve re-export
    pub fn approve_re_export(env: Env, caller: Address, re_export_id: BytesN<32>) {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut re_export: ReExportRecord = env
            .storage()
            .instance()
            .get(&ExportControlsKey::ReExportRecord(re_export_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::ReExportProhibited));

        re_export.approved = true;
        re_export.approved_at = env.ledger().timestamp();

        env.storage()
            .instance()
            .set(&ExportControlsKey::ReExportRecord(re_export_id.clone()), &re_export);

        env.events().publish(
            (Symbol::new(&env, "re_export_approved"),),
            (re_export_id,),
        );
    }

    /// Get re-export record
    pub fn get_re_export(env: Env, re_export_id: BytesN<32>) -> ReExportRecord {
        env.storage()
            .instance()
            .get(&ExportControlsKey::ReExportRecord(re_export_id))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::ReExportProhibited))
    }

    // ── Automated Screening ──────────────────────────────────────────────

    /// Perform comprehensive export screening
    pub fn screen_export(
        env: Env,
        caller: Address,
        exporter: Address,
        commodity: Bytes,
        destination: Bytes,
        end_use: Bytes,
        end_user: Address,
    ) -> BytesN<32> {
        caller.require_auth();

        let screening_id = Self::compute_screening_id(&env, &exporter, &commodity, &destination);
        let now = env.ledger().timestamp();

        let mut result = 0u32; // cleared
        let mut matches_found = 0u32;
        let mut risk_score = 0u32;
        let mut license_needed = false;
        let mut end_use_check_required = false;

        // 1. Screen denied parties
        let _match_result = Self::screen_denied_party(
            env.clone(),
            exporter.clone(),
            exporter.clone(),
            Bytes::from_slice(&env, b"exporter"),
        );

        // 2. Check destination restrictions
        if Self::is_high_risk_destination(&env, &destination) {
            result = 2; // escalated
            risk_score += 30;
            let blocked_count: u32 = env
                .storage()
                .instance()
                .get(&ExportControlsKey::BlockedTransactionCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&ExportControlsKey::BlockedTransactionCount, &(blocked_count + 1));
        }

        // 3. Check commodity controls
        if let Some(commodity_id) = env
            .storage()
            .instance()
            .get::<_, BytesN<32>>(&ExportControlsKey::CommodityByECCN(commodity.clone()))
        {
            if let Some(_comm) = env
                .storage()
                .instance()
                .get::<_, ControlledCommodity>(&ExportControlsKey::ControlledCommodity(commodity_id))
            {
                license_needed = true;
                end_use_check_required = true;
                risk_score += 20;
            }
        }

        // 4. Check end-use
        if Self::is_military_end_use(&env, &end_use) {
            result = 1; // blocked
            risk_score += 40;
            matches_found += 1;
        }

        let screening = ScreeningResult {
            id: screening_id.clone(),
            party: exporter,
            commodity,
            destination,
            screened_at: now,
            result,
            matches_found,
            risk_score,
            license_needed,
            end_use_check_required,
            screening_hash: env
                .crypto()
                .sha256(&Self::pack_screening_data(&env, &exporter, &destination, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&ExportControlsKey::ScreeningResult(screening_id.clone()), &screening);

        env.storage().instance().set(
            &ExportControlsKey::ScreeningsByParty(exporter.clone()),
            &screening_id,
        );

        let count: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::ScreeningCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ExportControlsKey::ScreeningCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "export_screening_completed"),),
            (screening_id.clone(), exporter, result),
        );

        if result == 1 {
            panic_with_error!(&env, ExportControlsError::TransactionBlocked);
        }

        screening_id
    }

    /// Get screening result
    pub fn get_screening_result(env: Env, screening_id: BytesN<32>) -> ScreeningResult {
        env.storage()
            .instance()
            .get(&ExportControlsKey::ScreeningResult(screening_id))
            .unwrap_or_else(|| panic_with_error!(&env, ExportControlsError::TransactionBlocked))
    }

    // ── Country Classification ───────────────────────────────────────────

    /// Set country group classification
    pub fn set_country_group(
        env: Env,
        caller: Address,
        country_code: Bytes,
        group: u32,
    ) {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        env.storage()
            .instance()
            .set(&ExportControlsKey::CountryGroup(country_code.clone()), &group);

        env.events().publish(
            (Symbol::new(&env, "country_group_set"),),
            (country_code, group),
        );
    }

    /// Get country group
    pub fn get_country_group(env: Env, country_code: Bytes) -> u32 {
        env.storage()
            .instance()
            .get(&ExportControlsKey::CountryGroup(country_code))
            .unwrap_or(5u32) // Unknown
    }

    // ── Statistics ───────────────────────────────────────────────────────

    /// Get export controls statistics
    pub fn get_export_controls_stats(env: Env) -> (u32, u32, u32, u32, u32) {
        let denied_parties: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::DeniedPartyCount)
            .unwrap_or(0);
        let screenings: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::ScreeningCount)
            .unwrap_or(0);
        let blocked: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::BlockedTransactionCount)
            .unwrap_or(0);
        let licenses: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::LicenseCount)
            .unwrap_or(0);
        let commodities: u32 = env
            .storage()
            .instance()
            .get(&ExportControlsKey::CommodityCount)
            .unwrap_or(0);

        (denied_parties, screenings, blocked, licenses, commodities)
    }

    // ── Private Helper Functions ─────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&ExportControlsKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, ExportControlsError::TransactionBlocked);
        }
    }

    fn is_high_risk_destination(env: &Env, country: &Bytes) -> bool {
        env.storage()
            .instance()
            .has(&ExportControlsKey::HighRiskDestination(country.clone()))
    }

    fn is_re_export_restricted(env: &Env, destination: &Bytes) -> bool {
        // Check if destination requires re-export authorization
        Self::is_high_risk_destination(env, destination)
    }

    fn is_suspicious_end_use(env: &Env, end_use: &Bytes) -> bool {
        // Check for suspicious keywords
        let end_use_str = core::str::from_utf8(end_use.as_ref()).unwrap_or("");
        end_use_str.contains("weapons")
            || end_use_str.contains("military")
            || end_use_str.contains("nuclear")
            || end_use_str.contains("missile")
    }

    fn is_military_end_use(env: &Env, end_use: &Bytes) -> bool {
        Self::is_suspicious_end_use(env, end_use)
    }

    fn compute_entry_id(env: &Env, name: &Bytes, address: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(name);
        preimage.append(address);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_match_id(env: &Env, party: &Address, name: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&party.to_string().to_bytes());
        preimage.append(name);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_license_id(env: &Env, license_type: &Bytes, exporter: &Address) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(license_type);
        preimage.append(&exporter.to_string().to_bytes());
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_commodity_id(env: &Env, eccn: &Bytes) -> BytesN<32> {
        env.crypto().sha256(eccn).into()
    }

    fn compute_check_id(env: &Env, commodity: &Bytes, end_user: &Address) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(commodity);
        preimage.append(&end_user.to_string().to_bytes());
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_re_export_id(env: &Env, re_exporter: &Address, commodity: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&re_exporter.to_string().to_bytes());
        preimage.append(commodity);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_screening_id(
        env: &Env,
        exporter: &Address,
        commodity: &Bytes,
        destination: &Bytes,
    ) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&exporter.to_string().to_bytes());
        preimage.append(commodity);
        preimage.append(destination);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn pack_license_data(env: &Env, exporter: &Address, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&exporter.to_string().to_bytes());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_check_data(env: &Env, end_user: &Address, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&end_user.to_string().to_bytes());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_re_export_data(env: &Env, re_exporter: &Address, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&re_exporter.to_string().to_bytes());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_screening_data(
        env: &Env,
        exporter: &Address,
        destination: &Bytes,
        timestamp: u64,
    ) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&exporter.to_string().to_bytes());
        data.append(destination);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn u64_to_bytes(env: &Env, v: u64) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
                ((v >> 32) & 0xff) as u8,
                ((v >> 40) & 0xff) as u8,
                ((v >> 48) & 0xff) as u8,
                ((v >> 56) & 0xff) as u8,
            ]
        )
    }
}

#[cfg(test)]
mod tests;
