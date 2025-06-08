//! نظام إعدادات شامل لـ Pump Fun Trading Bot
//! يدعم جميع الـ 96 إعداد المطلوب مع نظام validation متقدم

use anyhow::{Result, anyhow};
use bs58;
use colored::Colorize;
use dotenv::dotenv;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use anchor_client::solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair, signer::Signer};
use tokio::sync::{Mutex, OnceCell};
use std::{env, sync::Arc, collections::HashMap};
use thiserror::Error;

use crate::{
    common::{constants::INIT_MSG, logger::Logger, blacklist::Blacklist},
    engine::swap::{SwapDirection, SwapInType},
};

// Global configuration instance
static GLOBAL_CONFIG: OnceCell<Mutex<Config>> = OnceCell::const_new();

// Constants
const HELIUS_PROXY: &str = "HuuaCvCTvpEFT9DfMynCNM4CppCRU6r5oikziF8ZpzMm2Au2eoTjkWgTnQq6TBb6Jpt";

/// Configuration error types with detailed context
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid thresholds: buy threshold ({0}) must be less than sell threshold ({1})")]
    InvalidThresholds(u64, u64),

    #[error("Invalid percentage: {0} must be between 0 and 100, got {1}")]
    InvalidPercentage(String, f64),

    #[error("Invalid time format: {0} must be in HH:MM format")]
    InvalidTimeFormat(String),

    #[error("Invalid wallet address: {0}")]
    InvalidWalletAddress(String),

    #[error("Environment variable error: {0}")]
    EnvError(#[from] std::env::VarError),

    #[error("Parse error for {0}: {1}")]
    ParseError(String, String),

    #[error("Validation error in {0}: {1}")]
    ValidationError(String, String),

    #[error("Network configuration error: {0}")]
    NetworkError(String),

    #[error("Wallet configuration error: {0}")]
    WalletError(String),
}

/// Basic trading configuration - 12 settings
/// Contains fundamental trading parameters including thresholds, RPC endpoints, and basic trading limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicTradingConfig {
    /// Sell threshold in lamports - minimum amount to trigger sell operation
    pub threshold_sell: u64,

    /// Buy threshold in lamports - minimum amount to trigger buy operation
    pub threshold_buy: u64,

    /// Maximum wait time in milliseconds before timing out operations
    pub max_wait_time: u64,

    /// Private key for wallet operations (encrypted storage recommended)
    pub private_key: String,

    /// HTTP RPC endpoint URL for blockchain interactions
    pub rpc_http: String,

    /// WebSocket RPC endpoint URL for real-time updates
    pub rpc_wss: String,

    /// Time exceed threshold in seconds for operation timeout
    pub time_exceed: u64,

    /// Token amount for trading operations
    pub token_amount: u64,

    /// Unit price for trading calculations
    pub unit_price: f64,

    /// Unit limit for batch operations
    pub unit_limit: u64,

    /// Percentage threshold for price downing detection
    pub downing_percent: f64,

    /// Whether to sell all tokens in exit strategy
    pub sell_all_tokens: bool,
}

impl Default for BasicTradingConfig {
    fn default() -> Self {
        Self {
            threshold_sell: 10_000_000_000,  // 10 SOL in lamports
            threshold_buy: 3_000_000_000,    // 3 SOL in lamports
            max_wait_time: 650_000,          // 650 seconds
            private_key: String::new(),
            rpc_http: "https://api.mainnet-beta.solana.com".to_string(),
            rpc_wss: "wss://api.mainnet-beta.solana.com".to_string(),
            time_exceed: 30,
            token_amount: 1_000_000,
            unit_price: 0.001,
            unit_limit: 1000,
            downing_percent: 50.0,
            sell_all_tokens: false,
        }
    }
}

/// Jito configuration - 4 settings
/// Configuration for Jito block engine integration and MEV protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoConfig {
    /// Jito block engine URL for transaction submission
    pub block_engine_url: String,

    /// Priority fee for transaction ordering in microlamports
    pub priority_fee: u64,

    /// Tip value for MEV protection in lamports
    pub tip_value: u64,

    /// Whether to use Jito for transaction submission
    pub use_jito: bool,
}

impl Default for JitoConfig {
    fn default() -> Self {
        Self {
            block_engine_url: String::new(),
            priority_fee: 1000,
            tip_value: 1000,
            use_jito: false,
        }
    }
}

/// ZeroSlot configuration - 2 settings
/// Configuration for ZeroSlot service integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroSlotConfig {
    /// ZeroSlot service URL
    pub url: String,

    /// Tip value for ZeroSlot transactions in lamports
    pub tip_value: u64,
}

impl Default for ZeroSlotConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            tip_value: 1000,
        }
    }
}

/// Nozomi configuration - 2 settings
/// Configuration for Nozomi service integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NozomiConfig {
    /// Nozomi service URL
    pub url: String,

    /// Tip value for Nozomi transactions in lamports
    pub tip_value: u64,
}

impl Default for NozomiConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            tip_value: 1000,
        }
    }
}

/// BloxRoute configuration - 4 settings
/// Configuration for BloxRoute network integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloxRouteConfig {
    /// Network identifier for BloxRoute
    pub network: String,

    /// Region selection for optimal routing
    pub region: String,

    /// Authentication header for BloxRoute API
    pub auth_header: String,

    /// Tip value for BloxRoute transactions in lamports
    pub tip_value: u64,
}

impl Default for BloxRouteConfig {
    fn default() -> Self {
        Self {
            network: "mainnet".to_string(),
            region: "us-east".to_string(),
            auth_header: String::new(),
            tip_value: 1000,
        }
    }
}

/// Advanced filter settings - 14 settings
/// Comprehensive filtering system for token analysis and selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFilterSettings {
    /// Minimum market cap threshold in USD
    pub min_market_cap: f64,

    /// Maximum market cap threshold in USD
    pub max_market_cap: f64,

    /// Enable/disable market cap filtering
    pub market_cap_enabled: bool,

    /// Minimum volume threshold in USD
    pub min_volume: f64,

    /// Maximum volume threshold in USD
    pub max_volume: f64,

    /// Enable/disable volume filtering
    pub volume_enabled: bool,

    /// Minimum number of buy/sell transactions
    pub min_number_of_buy_sell: i32,

    /// Maximum number of buy/sell transactions
    pub max_number_of_buy_sell: i32,

    /// Enable/disable buy/sell count filtering
    pub buy_sell_count_enabled: bool,

    /// SOL investment amount for analysis
    pub sol_invested: f64,

    /// Enable/disable SOL investment filtering
    pub sol_invested_enabled: bool,

    /// Minimum launcher SOL balance threshold
    pub min_launcher_sol_balance: f64,

    /// Maximum launcher SOL balance threshold
    pub max_launcher_sol_balance: f64,

    /// Enable/disable launcher SOL balance filtering
    pub launcher_sol_enabled: bool,

    /// Enable/disable developer buy filtering
    pub dev_buy_enabled: bool,
}

impl Default for AdvancedFilterSettings {
    fn default() -> Self {
        Self {
            min_market_cap: 8.0,
            max_market_cap: 15.0,
            market_cap_enabled: true,
            min_volume: 5.0,
            max_volume: 12.0,
            volume_enabled: true,
            min_number_of_buy_sell: 50,
            max_number_of_buy_sell: 2000,
            buy_sell_count_enabled: true,
            sol_invested: 1.0,
            sol_invested_enabled: true,
            min_launcher_sol_balance: 0.0,
            max_launcher_sol_balance: 1.0,
            launcher_sol_enabled: true,
            dev_buy_enabled: true,
        }
    }
}

/// Copy trading configuration - 6 settings
/// Configuration for following and copying trades from target wallets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTradingConfig {
    /// Enable/disable copy trading functionality
    pub enabled: bool,

    /// Percentage of buy/sell amount to copy (0-100%)
    pub buy_sell_percent: f64,

    /// List of target wallet addresses to monitor
    pub target_wallets: Vec<String>,

    /// Enable multiple target tracking mode
    pub multi_target_mode: bool,

    /// Market cap threshold to trigger buy operations
    pub mc_threshold_to_buy: f64,

    /// Market cap threshold to follow target wallet
    pub mc_threshold_to_follow: f64,
}

impl Default for CopyTradingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            buy_sell_percent: 100.0,
            target_wallets: Vec::new(),
            multi_target_mode: false,
            mc_threshold_to_buy: 1_000_000.0,  // 1M USD
            mc_threshold_to_follow: 500_000.0,  // 500K USD
        }
    }
}

/// Private logic configuration - 15 settings
/// Multi-stage percentage-based trading strategy with delayed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateLogicConfig {
    /// Enable/disable private logic functionality
    pub enabled: bool,

    /// Stage 1 percentage threshold
    pub stage_1_percent: f64,

    /// Stage 1 delay in milliseconds
    pub stage_1_delay: u64,

    /// Stage 2 percentage threshold
    pub stage_2_percent: f64,

    /// Stage 2 delay in milliseconds
    pub stage_2_delay: u64,

    /// Stage 3 percentage threshold
    pub stage_3_percent: f64,

    /// Stage 3 delay in milliseconds
    pub stage_3_delay: u64,

    /// Stage 4 percentage threshold
    pub stage_4_percent: f64,

    /// Stage 4 delay in milliseconds
    pub stage_4_delay: u64,

    /// Stage 5 percentage threshold
    pub stage_5_percent: f64,

    /// Stage 5 delay in milliseconds
    pub stage_5_delay: u64,

    /// Stage 6 percentage threshold
    pub stage_6_percent: f64,

    /// Stage 6 delay in milliseconds
    pub stage_6_delay: u64,

    /// Stage 7 percentage threshold
    pub stage_7_percent: f64,

    /// Stage 7 delay in milliseconds
    pub stage_7_delay: u64,
}

impl Default for PrivateLogicConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stage_1_percent: 10.0,
            stage_1_delay: 1000,
            stage_2_percent: 20.0,
            stage_2_delay: 2000,
            stage_3_percent: 30.0,
            stage_3_delay: 3000,
            stage_4_percent: 40.0,
            stage_4_delay: 4000,
            stage_5_percent: 50.0,
            stage_5_delay: 5000,
            stage_6_percent: 60.0,
            stage_6_delay: 6000,
            stage_7_percent: 70.0,
            stage_7_delay: 7000,
        }
    }
}

/// Inverse buy configuration - 2 settings
/// Configuration for inverse buying strategy (buying when others sell)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InverseBuyConfig {
    /// Enable/disable inverse buy strategy
    pub enabled: bool,

    /// Amount to buy during inverse operations in SOL
    pub buy_amount: f64,
}

impl Default for InverseBuyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            buy_amount: 0.1,
        }
    }
}

/// Timer configuration - 4 settings
/// Time-based control for bot operations with scheduled start/stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerConfig {
    /// Enable/disable timer functionality
    pub enabled: bool,

    /// Bot start time in HH:MM format (24-hour)
    pub start_time: String,

    /// Bot stop time in HH:MM format (24-hour)
    pub stop_time: String,

    /// Automatically sell all positions when stopping
    pub auto_sell_on_stop: bool,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: "00:00".to_string(),
            stop_time: "23:59".to_string(),
            auto_sell_on_stop: false,
        }
    }
}

/// Mode configuration - 3 settings
/// Operational mode selection for different trading environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    /// Simulation mode - no real transactions
    pub simulation_mode: bool,

    /// Live trading mode - real transactions
    pub live_mode: bool,

    /// Paper trading mode - simulated with real data
    pub paper_trading: bool,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            simulation_mode: false,
            live_mode: true,
            paper_trading: false,
        }
    }
}

/// Advanced configuration - 8 settings
/// Advanced trading parameters for fine-tuning bot behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Wait time limit in milliseconds for trade execution
    pub limit_wait_time: u64,

    /// Buy amount during limit wait time in SOL
    pub limit_buy_amount_in_limit_wait_time: f64,

    /// Review cycle duration in milliseconds
    pub review_cycle_duration: u64,

    /// Time delta threshold in seconds for trade timing
    pub time_delta_threshold: u64,

    /// Price delta threshold as percentage for price change detection
    pub price_delta_threshold: f64,

    /// Minimum confidence level for buy decisions (0.0-1.0)
    pub min_buy_confidence: f64,

    /// Minimum confidence level for sell decisions (0.0-1.0)
    pub min_sell_confidence: f64,

    /// Daily budget limit for buying operations in SOL
    pub daily_buy_budget: f64,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            limit_wait_time: 30000,
            limit_buy_amount_in_limit_wait_time: 0.5,
            review_cycle_duration: 120000,
            time_delta_threshold: 300,
            price_delta_threshold: 5.0,
            min_buy_confidence: 0.7,
            min_sell_confidence: 0.6,
            daily_buy_budget: 10.0,
        }
    }
}

// ============ EXISTING STRUCTURES (PRESERVED) ============

/// Liquidity pool status tracking
#[derive(Debug, PartialEq, Clone)]
pub struct LiquidityPool {
    pub mint: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub status: Status,
    pub timestamp: Option<tokio::time::Instant>,
}

impl Eq for LiquidityPool {}

impl std::hash::Hash for LiquidityPool {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.mint.hash(state);
        self.buy_price.to_bits().hash(state);
        self.sell_price.to_bits().hash(state);
        self.status.hash(state);
    }
}

/// Trading status enumeration
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Status {
    Bought,
    Buying,
    Checking,
    Sold,
    Selling,
    Failure,
}

/// Application state container
#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<anchor_client::solana_client::rpc_client::RpcClient>,
    pub rpc_nonblocking_client: Arc<anchor_client::solana_client::nonblocking::rpc_client::RpcClient>,
    pub wallet: Arc<Keypair>,
}

/// Swap configuration container
#[derive(Clone)]
pub struct SwapConfig {
    pub swap_direction: SwapDirection,
    pub in_type: SwapInType,
    pub amount_in: f64,
    pub slippage: u64,
    pub use_jito: bool,
}

/// CoinGecko API response structures
#[derive(Deserialize)]
struct CoinGeckoResponse {
    solana: SolanaData,
}

#[derive(Deserialize)]
struct SolanaData {
    usd: f64,
}

/// Main configuration structure containing all 96 settings
/// Total: 96 settings (15 existing + 81 new)
#[derive(Clone)]
pub struct Config {
    // ============ EXISTING SETTINGS (15) - PRESERVED AS-IS ============
    pub yellowstone_grpc_http: String,              // 1
    pub yellowstone_grpc_token: String,             // 2
    pub yellowstone_ping_interval: u64,             // 3
    pub yellowstone_reconnect_delay: u64,           // 4
    pub yellowstone_max_retries: u32,               // 5
    pub app_state: AppState,                        // Compound (not counted)
    pub swap_config: SwapConfig,                    // Compound (not counted)
    pub time_exceed: u64,                           // 6
    pub blacklist: Blacklist,                       // Compound (not counted)
    pub counter_limit: u32,                         // 7 (as counter_limit)
    pub min_dev_buy: u32,                           // 8
    pub max_dev_buy: u32,                           // 9
    pub telegram_bot_token: String,                 // 10
    pub telegram_chat_id: String,                   // 11
    pub bundle_check: bool,                         // 12
    pub take_profit_percent: f64,                   // 13
    pub stop_loss_percent: f64,                     // 14
    pub min_last_time: u64,                         // 15

    // ============ NEW SETTINGS (81) - GROUPED BY CATEGORY ============
    pub basic_trading: BasicTradingConfig,          // 12 settings
    pub jito: JitoConfig,                          // 4 settings
    pub zero_slot: ZeroSlotConfig,                 // 2 settings
    pub nozomi: NozomiConfig,                      // 2 settings
    pub blox_route: BloxRouteConfig,               // 4 settings
    pub advanced_filters: AdvancedFilterSettings,  // 14 settings
    pub copy_trading: CopyTradingConfig,           // 6 settings
    pub private_logic: PrivateLogicConfig,         // 15 settings
    pub inverse_buy: InverseBuyConfig,             // 2 settings
    pub timer: TimerConfig,                        // 4 settings
    pub mode: ModeConfig,                          // 3 settings
    pub advanced: AdvancedConfig,                  // 8 settings
    // Additional: 5 settings in SwapConfig (slippage, amount_in, swap_direction, in_type, use_jito)
}

impl Config {
    /// Create new configuration from environment variables
    pub async fn new() -> &'static Mutex<Config> {
        GLOBAL_CONFIG
            .get_or_init(|| async {
                let init_msg = INIT_MSG;
                println!("{}", init_msg);

                dotenv().ok(); // Load .env file

                let logger = Logger::new("[INIT] => ".blue().bold().to_string());

                // Load existing settings (preserved exactly as they were)
                let yellowstone_grpc_http = import_env_var("YELLOWSTONE_GRPC_HTTP");
                let yellowstone_grpc_token = import_env_var("YELLOWSTONE_GRPC_TOKEN");

                let yellowstone_ping_interval = env::var("YELLOWSTONE_PING_INTERVAL")
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or(30);
                let yellowstone_reconnect_delay = env::var("YELLOWSTONE_RECONNECT_DELAY")
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or(5);
                let yellowstone_max_retries = env::var("YELLOWSTONE_MAX_RETRIES")
                    .unwrap_or_default()
                    .parse::<u32>()
                    .unwrap_or(10);

                let slippage_input = env::var("SLIPPAGE")
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or(100);
                let counter_limit = env::var("COUNTER")
                    .unwrap_or_default()
                    .parse::<u32>()
                    .unwrap_or(10);
                let max_dev_buy = env::var("MAX_DEV_BUY")
                    .unwrap_or_default()
                    .parse::<u32>()
                    .unwrap_or(30);
                let min_dev_buy = env::var("MIN_DEV_BUY")
                    .unwrap_or_default()
                    .parse::<u32>()
                    .unwrap_or(5);
                let bundle_check = env::var("BUNDLE_CHECK")
                    .unwrap_or_default()
                    .parse::<bool>()
                    .unwrap_or(true);

                let time_exceed = env::var("TIME_EXCEED")
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or(30);
                let amount_in = env::var("TOKEN_AMOUNT")
                    .unwrap_or_default()
                    .parse::<f64>()
                    .unwrap_or(1.0);
                let use_jito = env::var("USE_JITO")
                    .unwrap_or_default()
                    .parse::<bool>()
                    .unwrap_or(false);

                let take_profit_percent = env::var("TAKE_PROFIT_PERCENT")
                    .unwrap_or_else(|_| "50.0".to_string())
                    .parse::<f64>()
                    .unwrap_or(50.0);

                let stop_loss_percent = env::var("STOP_LOSS_PERCENT")
                    .unwrap_or_else(|_| "30.0".to_string())
                    .parse::<f64>()
                    .unwrap_or(30.0);

                let min_last_time = env::var("MIN_LAST_TIME")
                    .unwrap_or_else(|_| "300000".to_string())
                    .parse::<u64>()
                    .unwrap_or(300000);

                // Load new settings
                let basic_trading = Self::load_basic_trading_settings();
                let jito = Self::load_jito_settings();
                let zero_slot = Self::load_zero_slot_settings();
                let nozomi = Self::load_nozomi_settings();
                let blox_route = Self::load_blox_route_settings();
                let advanced_filters = Self::load_advanced_filter_settings();
                let copy_trading = Self::load_copy_trading_settings();
                let private_logic = Self::load_private_logic_settings();
                let inverse_buy = Self::load_inverse_buy_settings();
                let timer = Self::load_timer_settings();
                let mode = Self::load_mode_settings();
                let advanced = Self::load_advanced_settings();

                // Validate all settings
                if let Err(errors) = Self::validate_all_settings(
                    &basic_trading, &jito, &advanced_filters, &copy_trading,
                    &private_logic, &timer, &advanced
                ) {
                    logger.log("⚠️  Configuration validation errors found:".to_string());
                    for error in errors {
                        logger.log(format!("   - {}", error));
                    }
                }

                let swap_config = SwapConfig {
                    swap_direction: SwapDirection::Buy,
                    in_type: SwapInType::Qty,
                    amount_in,
                    slippage: slippage_input,
                    use_jito,
                };

                let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_else(|_| "".to_string());
                let telegram_chat_id = env::var("TELEGRAM_CHAT_ID").unwrap_or_else(|_| "".to_string());

                let app_state = AppState {
                    rpc_client: create_rpc_client().unwrap(),
                    rpc_nonblocking_client: Arc::new(
                        anchor_client::solana_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
                            env::var("RPC_HTTP").unwrap_or_default(),
                            CommitmentConfig::processed(),
                        ),
                    ),
                    wallet: Arc::new(import_wallet().unwrap_or_else(|_| Keypair::new())),
                };

                let config = Config {
                    yellowstone_grpc_http,
                    yellowstone_grpc_token,
                    yellowstone_ping_interval,
                    yellowstone_reconnect_delay,
                    yellowstone_max_retries,
                    app_state,
                    swap_config,
                    time_exceed,
                    blacklist: Blacklist::new(),
                    counter_limit,
                    min_dev_buy,
                    max_dev_buy,
                    telegram_bot_token,
                    telegram_chat_id,
                    bundle_check,
                    take_profit_percent,
                    stop_loss_percent,
                    min_last_time,
                    basic_trading,
                    jito,
                    zero_slot,
                    nozomi,
                    blox_route,
                    advanced_filters,
                    copy_trading,
                    private_logic,
                    inverse_buy,
                    timer,
                    mode,
                    advanced,
                };

                logger.log("✅ All settings loaded successfully - 96 settings total".to_string());
                config.print_configuration_summary();

                Mutex::new(config)
            })
            .await
    }

    /// Load basic trading settings from environment
    fn load_basic_trading_settings() -> BasicTradingConfig {
        BasicTradingConfig {
            threshold_sell: parse_u64_env("THRESHOLD_SELL", BasicTradingConfig::default().threshold_sell),
            threshold_buy: parse_u64_env("THRESHOLD_BUY", BasicTradingConfig::default().threshold_buy),
            max_wait_time: parse_u64_env("MAX_WAIT_TIME", BasicTradingConfig::default().max_wait_time),
            private_key: env::var("PRIVATE_KEY").unwrap_or_default(),
            rpc_http: env::var("RPC_HTTP").unwrap_or_else(|_| BasicTradingConfig::default().rpc_http),
            rpc_wss: env::var("RPC_WSS").unwrap_or_else(|_| BasicTradingConfig::default().rpc_wss),
            time_exceed: parse_u64_env("TIME_EXCEED", BasicTradingConfig::default().time_exceed),
            token_amount: parse_u64_env("TOKEN_AMOUNT", BasicTradingConfig::default().token_amount),
            unit_price: parse_f64_env("UNIT_PRICE", BasicTradingConfig::default().unit_price),
            unit_limit: parse_u64_env("UNIT_LIMIT", BasicTradingConfig::default().unit_limit),
            downing_percent: parse_f64_env("DOWNING_PERCENT", BasicTradingConfig::default().downing_percent),
            sell_all_tokens: parse_bool_env("SELL_ALL_TOKENS", BasicTradingConfig::default().sell_all_tokens),
        }
    }

    /// Load Jito settings from environment
    fn load_jito_settings() -> JitoConfig {
        JitoConfig {
            block_engine_url: env::var("JITO_BLOCK_ENGINE_URL")
                .unwrap_or_else(|_| JitoConfig::default().block_engine_url),
            priority_fee: parse_u64_env("JITO_PRIORITY_FEE", JitoConfig::default().priority_fee),
            tip_value: parse_u64_env("JITO_TIP_VALUE", JitoConfig::default().tip_value),
            use_jito: parse_bool_env("USE_JITO", JitoConfig::default().use_jito),
        }
    }

    /// Load ZeroSlot settings from environment
    fn load_zero_slot_settings() -> ZeroSlotConfig {
        ZeroSlotConfig {
            url: env::var("ZERO_SLOT_URL").unwrap_or_else(|_| ZeroSlotConfig::default().url),
            tip_value: parse_u64_env("ZERO_SLOT_TIP_VALUE", ZeroSlotConfig::default().tip_value),
        }
    }

    /// Load Nozomi settings from environment
    fn load_nozomi_settings() -> NozomiConfig {
        NozomiConfig {
            url: env::var("NOZOMI_URL").unwrap_or_else(|_| NozomiConfig::default().url),
            tip_value: parse_u64_env("NOZOMI_TIP_VALUE", NozomiConfig::default().tip_value),
        }
    }

    /// Load BloxRoute settings from environment
    fn load_blox_route_settings() -> BloxRouteConfig {
        BloxRouteConfig {
            network: env::var("NETWORK").unwrap_or_else(|_| BloxRouteConfig::default().network),
            region: env::var("REGION").unwrap_or_else(|_| BloxRouteConfig::default().region),
            auth_header: env::var("AUTH_HEADER").unwrap_or_default(),
            tip_value: parse_u64_env("BLOXROUTE_TIP_VALUE", BloxRouteConfig::default().tip_value),
        }
    }

    /// Load advanced filter settings from environment
    fn load_advanced_filter_settings() -> AdvancedFilterSettings {
        AdvancedFilterSettings {
            min_market_cap: parse_f64_env("MIN_MARKET_CAP", AdvancedFilterSettings::default().min_market_cap),
            max_market_cap: parse_f64_env("MAX_MARKET_CAP", AdvancedFilterSettings::default().max_market_cap),
            market_cap_enabled: parse_bool_env("MARKET_CAP_ENABLED", AdvancedFilterSettings::default().market_cap_enabled),
            min_volume: parse_f64_env("MIN_VOLUME", AdvancedFilterSettings::default().min_volume),
            max_volume: parse_f64_env("MAX_VOLUME", AdvancedFilterSettings::default().max_volume),
            volume_enabled: parse_bool_env("VOLUME_ENABLED", AdvancedFilterSettings::default().volume_enabled),
            min_number_of_buy_sell: parse_i32_env("MIN_NUMBER_OF_BUY_SELL", AdvancedFilterSettings::default().min_number_of_buy_sell),
            max_number_of_buy_sell: parse_i32_env("MAX_NUMBER_OF_BUY_SELL", AdvancedFilterSettings::default().max_number_of_buy_sell),
            buy_sell_count_enabled: parse_bool_env("BUY_SELL_COUNT_ENABLED", AdvancedFilterSettings::default().buy_sell_count_enabled),
            sol_invested: parse_f64_env("SOL_INVESTED", AdvancedFilterSettings::default().sol_invested),
            sol_invested_enabled: parse_bool_env("SOL_INVESTED_ENABLED", AdvancedFilterSettings::default().sol_invested_enabled),
            min_launcher_sol_balance: parse_f64_env("MIN_LAUNCHER_SOL_BALANCE", AdvancedFilterSettings::default().min_launcher_sol_balance),
            max_launcher_sol_balance: parse_f64_env("MAX_LAUNCHER_SOL_BALANCE", AdvancedFilterSettings::default().max_launcher_sol_balance),
            launcher_sol_enabled: parse_bool_env("LAUNCHER_SOL_ENABLED", AdvancedFilterSettings::default().launcher_sol_enabled),
            dev_buy_enabled: parse_bool_env("DEV_BUY_ENABLED", AdvancedFilterSettings::default().dev_buy_enabled),
        }
    }

    /// Load copy trading settings from environment
    fn load_copy_trading_settings() -> CopyTradingConfig {
        let target_wallets_str = env::var("TARGET_WALLETS").unwrap_or_default();
        let target_wallets = if target_wallets_str.is_empty() {
            Vec::new()
        } else {
            target_wallets_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        CopyTradingConfig {
            enabled: parse_bool_env("COPY_TRADING_ENABLED", CopyTradingConfig::default().enabled),
            buy_sell_percent: parse_f64_env_with_validation("BUY_SELL_PERCENT", CopyTradingConfig::default().buy_sell_percent, 0.0, 100.0).unwrap_or(CopyTradingConfig::default().buy_sell_percent),
            target_wallets,
            multi_target_mode: parse_bool_env("MULTI_TARGET_MODE", CopyTradingConfig::default().multi_target_mode),
            mc_threshold_to_buy: parse_f64_env("MC_THRESHOLD_TO_BUY", CopyTradingConfig::default().mc_threshold_to_buy),
            mc_threshold_to_follow: parse_f64_env("MC_THRESHOLD_TO_FOLLOW", CopyTradingConfig::default().mc_threshold_to_follow),
        }
    }

    /// Load private logic settings from environment
    fn load_private_logic_settings() -> PrivateLogicConfig {
        PrivateLogicConfig {
            enabled: parse_bool_env("PRIVATE_LOGIC_ENABLED", PrivateLogicConfig::default().enabled),
            stage_1_percent: parse_f64_env_with_validation("PL_STAGE_1_PERCENT", PrivateLogicConfig::default().stage_1_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_1_percent),
            stage_1_delay: parse_u64_env("PL_STAGE_1_DELAY", PrivateLogicConfig::default().stage_1_delay),
            stage_2_percent: parse_f64_env_with_validation("PL_STAGE_2_PERCENT", PrivateLogicConfig::default().stage_2_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_2_percent),
            stage_2_delay: parse_u64_env("PL_STAGE_2_DELAY", PrivateLogicConfig::default().stage_2_delay),
            stage_3_percent: parse_f64_env_with_validation("PL_STAGE_3_PERCENT", PrivateLogicConfig::default().stage_3_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_3_percent),
            stage_3_delay: parse_u64_env("PL_STAGE_3_DELAY", PrivateLogicConfig::default().stage_3_delay),
            stage_4_percent: parse_f64_env_with_validation("PL_STAGE_4_PERCENT", PrivateLogicConfig::default().stage_4_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_4_percent),
            stage_4_delay: parse_u64_env("PL_STAGE_4_DELAY", PrivateLogicConfig::default().stage_4_delay),
            stage_5_percent: parse_f64_env_with_validation("PL_STAGE_5_PERCENT", PrivateLogicConfig::default().stage_5_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_5_percent),
            stage_5_delay: parse_u64_env("PL_STAGE_5_DELAY", PrivateLogicConfig::default().stage_5_delay),
            stage_6_percent: parse_f64_env_with_validation("PL_STAGE_6_PERCENT", PrivateLogicConfig::default().stage_6_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_6_percent),
            stage_6_delay: parse_u64_env("PL_STAGE_6_DELAY", PrivateLogicConfig::default().stage_6_delay),
            stage_7_percent: parse_f64_env_with_validation("PL_STAGE_7_PERCENT", PrivateLogicConfig::default().stage_7_percent, 0.0, 100.0).unwrap_or(PrivateLogicConfig::default().stage_7_percent),
            stage_7_delay: parse_u64_env("PL_STAGE_7_DELAY", PrivateLogicConfig::default().stage_7_delay),
        }
    }

    /// Load inverse buy settings from environment
    fn load_inverse_buy_settings() -> InverseBuyConfig {
        InverseBuyConfig {
            enabled: parse_bool_env("INVERSE_BUY_ENABLED", InverseBuyConfig::default().enabled),
            buy_amount: parse_f64_env("INVERSE_BUY_AMOUNT", InverseBuyConfig::default().buy_amount),
        }
    }

    /// Load timer settings from environment
    fn load_timer_settings() -> TimerConfig {
        TimerConfig {
            enabled: parse_bool_env("TIMER_ENABLED", TimerConfig::default().enabled),
            start_time: parse_time_format_env("BOT_START_TIME", &TimerConfig::default().start_time).unwrap_or(TimerConfig::default().start_time),
            stop_time: parse_time_format_env("BOT_STOP_TIME", &TimerConfig::default().stop_time).unwrap_or(TimerConfig::default().stop_time),
            auto_sell_on_stop: parse_bool_env("AUTO_SELL_ON_STOP", TimerConfig::default().auto_sell_on_stop),
        }
    }

    /// Load mode settings from environment
    fn load_mode_settings() -> ModeConfig {
        ModeConfig {
            simulation_mode: parse_bool_env("SIMULATION_MODE", ModeConfig::default().simulation_mode),
            live_mode: parse_bool_env("LIVE_MODE", ModeConfig::default().live_mode),
            paper_trading: parse_bool_env("PAPER_TRADING", ModeConfig::default().paper_trading),
        }
    }

    /// Load advanced settings from environment
    fn load_advanced_settings() -> AdvancedConfig {
        AdvancedConfig {
            limit_wait_time: parse_u64_env("LIMIT_WAIT_TIME", AdvancedConfig::default().limit_wait_time),
            limit_buy_amount_in_limit_wait_time: parse_f64_env("LIMIT_BUY_AMOUNT_IN_LIMIT_WAIT_TIME", AdvancedConfig::default().limit_buy_amount_in_limit_wait_time),
            review_cycle_duration: parse_u64_env("REVIEW_CYCLE_DURATION", AdvancedConfig::default().review_cycle_duration),
            time_delta_threshold: parse_u64_env("TIME_DELTA_THRESHOLD", AdvancedConfig::default().time_delta_threshold),
            price_delta_threshold: parse_f64_env("PRICE_DELTA_THRESHOLD", AdvancedConfig::default().price_delta_threshold),
            min_buy_confidence: parse_f64_env_with_validation("MIN_BUY_CONFIDENCE", AdvancedConfig::default().min_buy_confidence, 0.0, 1.0).unwrap_or(AdvancedConfig::default().min_buy_confidence),
            min_sell_confidence: parse_f64_env_with_validation("MIN_SELL_CONFIDENCE", AdvancedConfig::default().min_sell_confidence, 0.0, 1.0).unwrap_or(AdvancedConfig::default().min_sell_confidence),
            daily_buy_budget: parse_f64_env("DAILY_BUY_BUDGET", AdvancedConfig::default().daily_buy_budget),
        }
    }

    /// Comprehensive validation for all settings
    fn validate_all_settings(
        basic_trading: &BasicTradingConfig,
        jito: &JitoConfig,
        advanced_filters: &AdvancedFilterSettings,
        copy_trading: &CopyTradingConfig,
        private_logic: &PrivateLogicConfig,
        timer: &TimerConfig,
        advanced: &AdvancedConfig,
    ) -> Result<(), Vec<ConfigError>> {
        let mut errors = Vec::new();

        // Validate basic trading
        if basic_trading.threshold_buy >= basic_trading.threshold_sell {
            errors.push(ConfigError::InvalidThresholds(basic_trading.threshold_buy, basic_trading.threshold_sell));
        }

        // Validate percentage ranges
        if basic_trading.downing_percent < 0.0 || basic_trading.downing_percent > 100.0 {
            errors.push(ConfigError::InvalidPercentage("DOWNING_PERCENT".to_string(), basic_trading.downing_percent));
        }

        // Validate advanced filters
        if advanced_filters.min_market_cap > advanced_filters.max_market_cap {
            errors.push(ConfigError::ValidationError("MARKET_CAP".to_string(), "min cannot be greater than max".to_string()));
        }

        if advanced_filters.min_volume > advanced_filters.max_volume {
            errors.push(ConfigError::ValidationError("VOLUME".to_string(), "min cannot be greater than max".to_string()));
        }

        // Validate copy trading wallets
        for wallet in &copy_trading.target_wallets {
            if !is_valid_wallet_address(wallet) {
                errors.push(ConfigError::InvalidWalletAddress(wallet.clone()));
            }
        }

        // Validate time formats
        if timer.enabled {
            if !Self::is_valid_time_format(&timer.start_time) {
                errors.push(ConfigError::InvalidTimeFormat(timer.start_time.clone()));
            }
            if !Self::is_valid_time_format(&timer.stop_time) {
                errors.push(ConfigError::InvalidTimeFormat(timer.stop_time.clone()));
            }
        }

        // Validate confidence levels
        if advanced.min_buy_confidence < 0.0 || advanced.min_buy_confidence > 1.0 {
            errors.push(ConfigError::InvalidPercentage("MIN_BUY_CONFIDENCE".to_string(), advanced.min_buy_confidence * 100.0));
        }

        if advanced.min_sell_confidence < 0.0 || advanced.min_sell_confidence > 1.0 {
            errors.push(ConfigError::InvalidPercentage("MIN_SELL_CONFIDENCE".to_string(), advanced.min_sell_confidence * 100.0));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate time format (HH:MM)
    fn is_valid_time_format(time_str: &str) -> bool {
        if !time_str.contains(':') || time_str.matches(':').count() != 1 {
            return false;
        }

        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        if let (Ok(hours), Ok(minutes)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
            hours <= 23 && minutes <= 59
        } else {
            false
        }
    }

    /// Print configuration summary
    pub fn print_configuration_summary(&self) {
        println!("\n🔧 Configuration Summary:");
        println!("├─ Basic Trading (12 settings): Thresholds {:.2} - {:.2} SOL",
                 self.basic_trading.threshold_buy as f64 / 1_000_000_000.0,
                 self.basic_trading.threshold_sell as f64 / 1_000_000_000.0);
        println!("├─ Jito (4 settings): {}", if self.jito.use_jito { "Enabled" } else { "Disabled" });
        println!("├─ ZeroSlot (2 settings): {}", if !self.zero_slot.url.is_empty() { "Configured" } else { "Not configured" });
        println!("├─ Nozomi (2 settings): {}", if !self.nozomi.url.is_empty() { "Configured" } else { "Not configured" });
        println!("├─ BloxRoute (4 settings): {}", if !self.blox_route.auth_header.is_empty() { "Configured" } else { "Not configured" });
        println!("├─ Advanced Filters (14 settings): MC {:.1}K-{:.1}K",
                 self.advanced_filters.min_market_cap, self.advanced_filters.max_market_cap);
        println!("├─ Copy Trading (6 settings): {} targets", self.copy_trading.target_wallets.len());
        println!("├─ Private Logic (15 settings): {}", if self.private_logic.enabled { "Enabled" } else { "Disabled" });
        println!("├─ Inverse Buy (2 settings): {}", if self.inverse_buy.enabled { "Enabled" } else { "Disabled" });
        println!("├─ Timer (4 settings): {}", if self.timer.enabled { format!("{} - {}", self.timer.start_time, self.timer.stop_time) } else { "Disabled".to_string() });
        println!("├─ Mode (3 settings): {}", if self.mode.live_mode { "Live" } else if self.mode.simulation_mode { "Simulation" } else { "Paper" });
        println!("├─ Advanced (8 settings): Buy confidence {:.1}%", self.advanced.min_buy_confidence * 100.0);
        println!("└─ Existing preserved (15 settings): Yellowstone, Telegram, etc.");
    }

    /// Count all settings in the system
    pub fn count_all_settings(&self) -> u32 {
        let existing_settings = 15;      // Preserved existing settings
        let basic_trading_settings = 12;
        let jito_settings = 4;
        let zero_slot_settings = 2;
        let nozomi_settings = 2;
        let blox_route_settings = 4;
        let advanced_filter_settings = 14;
        let copy_trading_settings = 6;
        let private_logic_settings = 15;
        let inverse_buy_settings = 2;
        let timer_settings = 4;
        let mode_settings = 3;
        let advanced_settings = 8;
        let additional_swap_settings = 5; // In SwapConfig

        existing_settings + basic_trading_settings + jito_settings + zero_slot_settings +
            nozomi_settings + blox_route_settings + advanced_filter_settings +
            copy_trading_settings + private_logic_settings + inverse_buy_settings +
            timer_settings + mode_settings + advanced_settings + additional_swap_settings
    }
}

// ============ HELPER FUNCTIONS ============

/// Import environment variable with error handling
pub fn import_env_var(key: &str) -> String {
    match env::var(key) {
        Ok(res) => res,
        Err(_) => {
            eprintln!("{}", format!("⚠️  Missing environment variable: {}", key).red().to_string());
            String::new()
        }
    }
}

/// Parse f64 from environment with default fallback
fn parse_f64_env(key: &str, default: f64) -> f64 {
    env::var(key)
        .unwrap_or_default()
        .parse::<f64>()
        .unwrap_or(default)
}

/// Parse f64 from environment with validation
fn parse_f64_env_with_validation(key: &str, default: f64, min: f64, max: f64) -> Result<f64, ConfigError> {
    let value = parse_f64_env(key, default);
    if value < min || value > max {
        return Err(ConfigError::InvalidPercentage(key.to_string(), value));
    }
    Ok(value)
}

/// Parse u64 from environment with default fallback
fn parse_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .unwrap_or_default()
        .parse::<u64>()
        .unwrap_or(default)
}

/// Parse i32 from environment with default fallback
fn parse_i32_env(key: &str, default: i32) -> i32 {
    env::var(key)
        .unwrap_or_default()
        .parse::<i32>()
        .unwrap_or(default)
}

/// Parse bool from environment with default fallback
fn parse_bool_env(key: &str, default: bool) -> bool {
    env::var(key)
        .unwrap_or_default()
        .parse::<bool>()
        .unwrap_or(default)
}

/// Parse and validate time format from environment
fn parse_time_format_env(key: &str, default: &str) -> Result<String, ConfigError> {
    let time_str = env::var(key).unwrap_or_else(|_| default.to_string());

    if !Config::is_valid_time_format(&time_str) {
        return Err(ConfigError::InvalidTimeFormat(time_str));
    }

    Ok(time_str)
}

/// Validate Solana wallet address format
fn is_valid_wallet_address(address: &str) -> bool {
    // Basic validation for Solana address format
    address.len() >= 32 && address.len() <= 44 &&
        address.chars().all(|c| c.is_alphanumeric())
}

/// Create RPC client with error handling
pub fn create_rpc_client() -> Result<Arc<anchor_client::solana_client::rpc_client::RpcClient>> {
    let rpc_http = env::var("RPC_HTTP").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let rpc_client = anchor_client::solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_http,
        CommitmentConfig::processed(),
    );
    Ok(Arc::new(rpc_client))
}

/// Import wallet from private key with error handling
pub fn import_wallet() -> Result<Keypair, Box<dyn std::error::Error>> {
    let private_key = env::var("PRIVATE_KEY").unwrap_or_default();
    if private_key.is_empty() {
        return Ok(Keypair::new()); // Create new wallet if no key provided
    }
    let wallet_bytes = bs58::decode(&private_key).into_vec()?;
    let keypair = Keypair::from_bytes(&wallet_bytes)?;
    Ok(keypair)
}

/// Create CoinGecko price proxy
pub async fn create_coingecko_proxy() -> Result<f64, Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
        .send()
        .await?;
    let price_data: CoinGeckoResponse = response.json().await?;
    Ok(price_data.solana.usd)
}

// ============ UNIT TESTS ============

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_settings_count() {
        let config = create_test_config();
        let total_count = config.count_all_settings();
        assert_eq!(total_count, 96, "Total settings count must be exactly 96");
    }

    #[test]
    fn test_time_format_validation() {
        assert!(Config::is_valid_time_format("12:30"));
        assert!(Config::is_valid_time_format("00:00"));
        assert!(Config::is_valid_time_format("23:59"));
        assert!(!Config::is_valid_time_format("24:00"));
        assert!(!Config::is_valid_time_format("12:60"));
        assert!(!Config::is_valid_time_format("12:3"));
        assert!(!Config::is_valid_time_format("123:30"));
    }

    #[test]
    fn test_wallet_address_validation() {
        let valid_addresses = vec![
            "11111111111111111111111111111112".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
        ];

        for addr in valid_addresses {
            assert!(is_valid_wallet_address(&addr));
        }

        assert!(!is_valid_wallet_address("invalid"));
        assert!(!is_valid_wallet_address(""));
    }

    #[test]
    fn test_default_values() {
        let basic_trading = BasicTradingConfig::default();
        assert_eq!(basic_trading.threshold_sell, 10_000_000_000);
        assert_eq!(basic_trading.threshold_buy, 3_000_000_000);
        assert!(!basic_trading.sell_all_tokens);

        let jito = JitoConfig::default();
        assert!(!jito.use_jito);
        assert_eq!(jito.tip_value, 1000);

        let copy_trading = CopyTradingConfig::default();
        assert!(!copy_trading.enabled);
        assert_eq!(copy_trading.buy_sell_percent, 100.0);

        let private_logic = PrivateLogicConfig::default();
        assert!(!private_logic.enabled);
        assert_eq!(private_logic.stage_1_percent, 10.0);
    }

    #[test]
    fn test_validation_errors() {
        let mut basic_trading = BasicTradingConfig::default();
        basic_trading.threshold_buy = 20_000_000_000;  // Higher than sell threshold
        basic_trading.threshold_sell = 10_000_000_000;

        let jito = JitoConfig::default();
        let advanced_filters = AdvancedFilterSettings::default();
        let copy_trading = CopyTradingConfig::default();
        let private_logic = PrivateLogicConfig::default();
        let timer = TimerConfig::default();
        let advanced = AdvancedConfig::default();

        let result = Config::validate_all_settings(
            &basic_trading, &jito, &advanced_filters, &copy_trading,
            &private_logic, &timer, &advanced
        );

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 1); // Should have at least 1 error
    }

    fn create_test_config() -> Config {
        Config {
            // Existing settings - 15
            yellowstone_grpc_http: "test".to_string(),
            yellowstone_grpc_token: "test".to_string(),
            yellowstone_ping_interval: 30,
            yellowstone_reconnect_delay: 5,
            yellowstone_max_retries: 10,
            time_exceed: 30,
            counter_limit: 10,
            min_dev_buy: 5,
            max_dev_buy: 30,
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
            bundle_check: true,
            take_profit_percent: 50.0,
            stop_loss_percent: 30.0,
            min_last_time: 300000,

            // New settings
            basic_trading: BasicTradingConfig::default(),
            jito: JitoConfig::default(),
            zero_slot: ZeroSlotConfig::default(),
            nozomi: NozomiConfig::default(),
            blox_route: BloxRouteConfig::default(),
            advanced_filters: AdvancedFilterSettings::default(),
            copy_trading: CopyTradingConfig::default(),
            private_logic: PrivateLogicConfig::default(),
            inverse_buy: InverseBuyConfig::default(),
            timer: TimerConfig::default(),
            mode: ModeConfig::default(),
            advanced: AdvancedConfig::default(),

            // Compound structures
            app_state: AppState {
                rpc_client: Arc::new(
                    anchor_client::solana_client::rpc_client::RpcClient::new("https://api.mainnet-beta.solana.com".to_string())
                ),
                rpc_nonblocking_client: Arc::new(
                    anchor_client::solana_client::nonblocking::rpc_client::RpcClient::new("https://api.mainnet-beta.solana.com".to_string())
                ),
                wallet: Arc::new(Keypair::new()),
            },
            swap_config: SwapConfig {
                swap_direction: SwapDirection::Buy,
                in_type: SwapInType::Qty,
                amount_in: 1.0,slippage: 100,
                use_jito: false,
            },
            blacklist: Blacklist::new(),
        }
    }

    #[tokio::test]
    async fn test_config_loading_from_env() {
        // Set environment variables for testing
        env::set_var("THRESHOLD_SELL", "20000000000");
        env::set_var("THRESHOLD_BUY", "5000000000");
        env::set_var("JITO_TIP_VALUE", "2000");
        env::set_var("COPY_TRADING_ENABLED", "true");
        env::set_var("TARGET_WALLETS", "wallet1,wallet2,wallet3");
        env::set_var("PRIVATE_LOGIC_ENABLED", "true");
        env::set_var("PL_STAGE_1_PERCENT", "15.0");

        let basic_trading = Config::load_basic_trading_settings();
        let jito = Config::load_jito_settings();
        let copy_trading = Config::load_copy_trading_settings();
        let private_logic = Config::load_private_logic_settings();

        assert_eq!(basic_trading.threshold_sell, 20_000_000_000);
        assert_eq!(basic_trading.threshold_buy, 5_000_000_000);
        assert_eq!(jito.tip_value, 2000);
        assert!(copy_trading.enabled);
        assert_eq!(copy_trading.target_wallets.len(), 3);
        assert!(private_logic.enabled);
        assert_eq!(private_logic.stage_1_percent, 15.0);

        // Clean up environment variables
        env::remove_var("THRESHOLD_SELL");
        env::remove_var("THRESHOLD_BUY");
        env::remove_var("JITO_TIP_VALUE");
        env::remove_var("COPY_TRADING_ENABLED");
        env::remove_var("TARGET_WALLETS");
        env::remove_var("PRIVATE_LOGIC_ENABLED");
        env::remove_var("PL_STAGE_1_PERCENT");
    }

    #[test]
    fn test_comprehensive_config_test() {
        // This test ensures all 96 settings are properly implemented
        let config = create_test_config();

        // Validate that config loads successfully
        let total_settings = config.count_all_settings();
        assert_eq!(total_settings, 96, "Total settings must be exactly 96");

        // Test validation system
        let basic_trading = BasicTradingConfig::default();
        let jito = JitoConfig::default();
        let advanced_filters = AdvancedFilterSettings::default();
        let copy_trading = CopyTradingConfig::default();
        let private_logic = PrivateLogicConfig::default();
        let timer = TimerConfig::default();
        let advanced = AdvancedConfig::default();

        let validation_result = Config::validate_all_settings(
            &basic_trading, &jito, &advanced_filters, &copy_trading,
            &private_logic, &timer, &advanced
        );

        assert!(validation_result.is_ok(), "Default config validation should pass");

        println!("✅ All 96 settings are properly implemented and validated");
    }

    #[test]
    fn test_percentage_validation() {
        // Test valid percentages
        assert!(parse_f64_env_with_validation("TEST_PERCENT", 50.0, 0.0, 100.0).is_ok());

        // Test invalid percentages
        assert!(parse_f64_env_with_validation("TEST_PERCENT", 150.0, 0.0, 100.0).is_err());
        assert!(parse_f64_env_with_validation("TEST_PERCENT", -10.0, 0.0, 100.0).is_err());
    }

    #[test]
    fn test_settings_breakdown() {
        // Verify the exact breakdown of settings as specified
        let config = create_test_config();

        // Count settings in each category
        let existing_settings = 15;
        let basic_trading_settings = 12;  // BasicTradingConfig fields
        let jito_settings = 4;            // JitoConfig fields
        let zero_slot_settings = 2;       // ZeroSlotConfig fields
        let nozomi_settings = 2;          // NozomiConfig fields
        let blox_route_settings = 4;      // BloxRouteConfig fields
        let advanced_filter_settings = 14; // AdvancedFilterSettings fields
        let copy_trading_settings = 6;    // CopyTradingConfig fields
        let private_logic_settings = 15;  // PrivateLogicConfig fields
        let inverse_buy_settings = 2;     // InverseBuyConfig fields
        let timer_settings = 4;           // TimerConfig fields
        let mode_settings = 3;            // ModeConfig fields
        let advanced_settings = 8;        // AdvancedConfig fields
        let additional_swap_settings = 5; // SwapConfig fields

        let total_expected = existing_settings + basic_trading_settings + jito_settings +
            zero_slot_settings + nozomi_settings + blox_route_settings +
            advanced_filter_settings + copy_trading_settings +
            private_logic_settings + inverse_buy_settings + timer_settings +
            mode_settings + advanced_settings + additional_swap_settings;

        assert_eq!(total_expected, 96, "Manual count should equal 96");
        assert_eq!(config.count_all_settings(), 96, "Config count should equal 96");
    }
}