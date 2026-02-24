#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};

// -------------------------------------------------------------------
//  Storage Keys
// -------------------------------------------------------------------

/// Each price entry is stored per asset symbol so a single oracle
/// contract can track many different price feeds simultaneously.
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Latest price (in micro-units, e.g. 1_000_000 = $1.00) for an asset.
    Price(String),
    /// UNIX timestamp (seconds) of the last price update for an asset.
    Timestamp(String),
    /// Maximum age (in seconds) before a price is considered stale.
    StaleTtl,
    /// Admin address – the only address allowed to push price updates.
    Admin,
}

// -------------------------------------------------------------------
//  Errors
// -------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    /// No price has been recorded for the requested asset yet.
    AssetNotFound = 1,
    /// The price value must be strictly greater than zero.
    InvalidPrice = 2,
    /// Only the admin may push price updates.
    Unauthorized = 3,
    /// The contract has already been initialized.
    AlreadyInitialized = 4,
    /// The staleness TTL must be greater than zero.
    InvalidTtl = 5,
}

// -------------------------------------------------------------------
//  Contract
// -------------------------------------------------------------------

#[contract]
pub struct OraclePriceFeed;

#[contractimpl]
impl OraclePriceFeed {

    // ---------------------------------------------------------------
    //  Initialization
    // ---------------------------------------------------------------

    /// Initialize the oracle with an admin address and a staleness TTL.
    ///
    /// * `admin`     – Address that is authorised to call `set_price`.
    /// * `stale_ttl` – Number of seconds after which a price is stale.
    pub fn initialize(
        env: Env,
        admin: Address,
        stale_ttl: u64,
    ) -> Result<(), OracleError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(OracleError::AlreadyInitialized);
        }
        if stale_ttl == 0 {
            return Err(OracleError::InvalidTtl);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::StaleTtl, &stale_ttl);

        env.events().publish(
            (symbol_short!("init"),),
            (admin.clone(), stale_ttl),
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    //  Write – set_price
    // ---------------------------------------------------------------

    /// Record a new price for `asset` at the current ledger timestamp.
    ///
    /// * `asset` – Uppercase ticker symbol, e.g. `"XLM"`, `"BTC"`.
    /// * `price` – Price expressed in micro-units (1 USD = 1_000_000).
    ///
    /// Only the admin may call this function.
    ///
    /// **Storage diff produced (visible in the debugger):**
    /// ```
    /// BEFORE  Price("XLM") = <old_value>   Timestamp("XLM") = <old_ts>
    /// AFTER   Price("XLM") = <new_value>   Timestamp("XLM") = <new_ts>
    /// ```
    pub fn set_price(
        env: Env,
        asset: String,
        price: i128,
    ) -> Result<(), OracleError> {
        if price <= 0 {
            return Err(OracleError::InvalidPrice);
        }

        // Admin authorisation check.
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap();
        admin.require_auth();

        // Current ledger time acts as the canonical timestamp.
        let now: u64 = env.ledger().timestamp();

        env.storage()
            .persistent()
            .set(&DataKey::Price(asset.clone()), &price);
        env.storage()
            .persistent()
            .set(&DataKey::Timestamp(asset.clone()), &now);

        env.events().publish(
            (symbol_short!("setprice"),),
            (asset.clone(), price, now),
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    //  Read – get_price
    // ---------------------------------------------------------------

    /// Return the latest recorded price for `asset` in micro-units.
    ///
    /// Returns `OracleError::AssetNotFound` if no price has ever been
    /// pushed for that asset.
    pub fn get_price(env: Env, asset: String) -> Result<i128, OracleError> {
        env.storage()
            .persistent()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::AssetNotFound)
    }

    // ---------------------------------------------------------------
    //  Read – get_timestamp
    // ---------------------------------------------------------------

    /// Return the UNIX timestamp of the most recent `set_price` call
    /// for `asset`.
    ///
    /// Returns `OracleError::AssetNotFound` if no price has ever been
    /// pushed for that asset.
    pub fn get_timestamp(env: Env, asset: String) -> Result<u64, OracleError> {
        env.storage()
            .persistent()
            .get(&DataKey::Timestamp(asset))
            .ok_or(OracleError::AssetNotFound)
    }

    // ---------------------------------------------------------------
    //  Read – is_stale
    // ---------------------------------------------------------------

    /// Return `true` when the price for `asset` is older than the
    /// configured `stale_ttl` seconds relative to the current ledger
    /// timestamp.
    ///
    /// Returns `OracleError::AssetNotFound` if no price has ever been
    /// pushed for that asset.
    pub fn is_stale(env: Env, asset: String) -> Result<bool, OracleError> {
        let last_ts: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Timestamp(asset))
            .ok_or(OracleError::AssetNotFound)?;

        let stale_ttl: u64 = env
            .storage()
            .instance()
            .get(&DataKey::StaleTtl)
            .unwrap();

        let now: u64 = env.ledger().timestamp();

        Ok(now.saturating_sub(last_ts) > stale_ttl)
    }

    // ---------------------------------------------------------------
    //  Read – get_stale_ttl
    // ---------------------------------------------------------------

    /// Return the configured staleness TTL in seconds.
    pub fn get_stale_ttl(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::StaleTtl)
            .unwrap()
    }
}

// -------------------------------------------------------------------
//  Unit tests
// -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    /// Helper: create a fresh test environment with all auth mocked.
    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(OraclePriceFeed, ());
        (env, admin, contract_id)
    }

    #[test]
    fn test_initialize_and_set_get_price() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "XLM");
        client.set_price(&asset, &1_100_000i128);

        let price = client.get_price(&asset);
        assert_eq!(price, 1_100_000i128);
    }

    #[test]
    fn test_price_not_stale_immediately_after_update() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "BTC");
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.set_price(&asset, &40_000_000_000i128);

        // Still within TTL – should not be stale.
        assert!(!client.is_stale(&asset));
    }

    #[test]
    fn test_price_becomes_stale_after_ttl() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "ETH");
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.set_price(&asset, &2_000_000_000i128);

        // Advance ledger by more than the 300-second TTL.
        env.ledger().with_mut(|l| l.timestamp = 1_500);
        assert!(client.is_stale(&asset));
    }

    #[test]
    fn test_update_price_refreshes_staleness() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "XLM");
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.set_price(&asset, &1_000_000i128);

        // Go past staleness threshold.
        env.ledger().with_mut(|l| l.timestamp = 2_000);
        assert!(client.is_stale(&asset));

        // Push a fresh price – staleness should reset.
        client.set_price(&asset, &1_050_000i128);
        assert!(!client.is_stale(&asset));

        let price = client.get_price(&asset);
        assert_eq!(price, 1_050_000i128);
    }

    #[test]
    fn test_get_timestamp() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "XLM");
        env.ledger().with_mut(|l| l.timestamp = 5_000);
        client.set_price(&asset, &1_000_000i128);

        assert_eq!(client.get_timestamp(&asset), 5_000u64);
    }

    #[test]
    fn test_asset_not_found_panics_via_try() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "UNKNOWN");
        // Contract returns Err(AssetNotFound); try_ yields Err(Ok(AssetNotFound))
        let result = client.try_get_price(&asset);
        assert_eq!(result, Err(Ok(OracleError::AssetNotFound)));
    }

    #[test]
    fn test_double_initialize_fails() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);
        let result = client.try_initialize(&admin, &300u64);
        // Contract returns Err(AlreadyInitialized); try_initialize yields Err(Ok(AlreadyInitialized))
        assert_eq!(result, Err(Ok(OracleError::AlreadyInitialized)));
    }

    #[test]
    fn test_invalid_price_rejected() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &300u64);

        let asset = String::from_str(&env, "XLM");

        let result_zero = client.try_set_price(&asset, &0i128);
        // Contract returns Err(InvalidPrice)
        assert_eq!(result_zero, Err(Ok(OracleError::InvalidPrice)));

        let result_neg = client.try_set_price(&asset, &-1i128);
        assert_eq!(result_neg, Err(Ok(OracleError::InvalidPrice)));
    }

    #[test]
    fn test_get_stale_ttl() {
        let (env, admin, contract_id) = setup();
        let client = OraclePriceFeedClient::new(&env, &contract_id);

        client.initialize(&admin, &600u64);
        assert_eq!(client.get_stale_ttl(), 600u64);
    }
}
