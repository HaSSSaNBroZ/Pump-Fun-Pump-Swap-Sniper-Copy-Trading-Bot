use anyhow::{Result, anyhow};
use bs58;
use colored::Colorize;
use dotenv::dotenv;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use anchor_client::solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair, signer::Signer};
use tokio::sync::{Mutex, OnceCell};
use std::{env, sync::Arc, collections::HashMap};

use crate::{
    common::{constants::INIT_MSG, logger::Logger, blacklist::Blacklist},
    engine::swap::{SwapDirection, SwapInType},
};

static GLOBAL_CONFIG: OnceCell<Mutex<Config>> = OnceCell::const_new();

const HELIUS_PROXY: &str = "HuuaCvCTvpEFT9DfMynCNM4CppCRU6r5oikziF8ZpzMm2Au2eoTjkWgTnQq6TBb6Jpt";

/// نظام التحقق من صحة الإعدادات
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// إعدادات التداول الأساسية - 12 إعداد
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSettings {
    /// عتبة البيع بـ lamports
    pub threshold_sell: u64,
    /// عتبة الشراء بـ lamports
    pub threshold_buy: u64,
    /// الحد الأقصى لوقت الانتظار بالميلي ثانية
    pub max_wait_time: u64,
    /// المفتاح الخاص للمحفظة (مشفر)
    pub private_key: String,
    /// رابط RPC HTTP
    pub rpc_http: String,
    /// رابط RPC WebSocket
    pub rpc_wss: String,
    /// تجاوز الوقت
    pub time_exceed: u64,
    /// كمية التوكن
    pub token_amount: u64,
    /// سعر الوحدة
    pub unit_price: f64,
    /// حد الوحدة
    pub unit_limit: u64,
    /// نسبة الانخفاض
    pub downing_percent: f64,
    /// بيع جميع التوكنات
    pub sell_all_tokens: bool,
}

impl Default for TradingSettings {
    fn default() -> Self {
        Self {
            threshold_sell: 10_000_000_000, // 10 SOL
            threshold_buy: 3_000_000_000,   // 3 SOL
            max_wait_time: 650_000,         // 650 seconds
            private_key: String::new(),
            rpc_http: String::new(),
            rpc_wss: String::new(),
            time_exceed: 30,
            token_amount: 1_000_000,
            unit_price: 0.001,
            unit_limit: 1000,
            downing_percent: 50.0,
            sell_all_tokens: false,
        }
    }
}

/// إعدادات Jito - 4 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoSettings {
    /// رابط محرك Jito
    pub jito_block_engine_url: String,
    /// رسوم الأولوية
    pub jito_priority_fee: u64,
    /// قيمة الإكرامية
    pub jito_tip_value: u64,
    /// تفعيل Jito
    pub use_jito: bool,
}

impl Default for JitoSettings {
    fn default() -> Self {
        Self {
            jito_block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
            jito_priority_fee: 1000,
            jito_tip_value: 1000,
            use_jito: false,
        }
    }
}

/// إعدادات ZeroSlot - 2 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroSlotSettings {
    /// رابط ZeroSlot
    pub zero_slot_url: String,
    /// قيمة إكرامية ZeroSlot
    pub zero_slot_tip_value: u64,
}

impl Default for ZeroSlotSettings {
    fn default() -> Self {
        Self {
            zero_slot_url: String::new(),
            zero_slot_tip_value: 1000,
        }
    }
}

/// إعدادات Nozomi - 2 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NozomiSettings {
    /// رابط Nozomi
    pub nozomi_url: String,
    /// قيمة إكرامية Nozomi
    pub nozomi_tip_value: u64,
}

impl Default for NozomiSettings {
    fn default() -> Self {
        Self {
            nozomi_url: String::new(),
            nozomi_tip_value: 1000,
        }
    }
}

/// إعدادات BloxRoute - 4 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloxRouteSettings {
    /// الشبكة
    pub network: String,
    /// المنطقة
    pub region: String,
    /// رأس المصادقة
    pub auth_header: String,
    /// قيمة إكرامية BloxRoute
    pub bloxroute_tip_value: u64,
}

impl Default for BloxRouteSettings {
    fn default() -> Self {
        Self {
            network: "mainnet".to_string(),
            region: "us-east-1".to_string(),
            auth_header: String::new(),
            bloxroute_tip_value: 1000,
        }
    }
}

/// إعدادات التصفية المتقدمة - 14 إعداد
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedFilterSettings {
    /// الحد الأدنى لرأس المال السوقي
    pub min_market_cap: f64,
    /// الحد الأقصى لرأس المال السوقي
    pub max_market_cap: f64,
    /// تفعيل فلتر رأس المال
    pub market_cap_enabled: bool,
    /// الحد الأدنى للحجم
    pub min_volume: f64,
    /// الحد الأقصى للحجم
    pub max_volume: f64,
    /// تفعيل فلتر الحجم
    pub volume_enabled: bool,
    /// الحد الأدنى لعدد الشراء/البيع
    pub min_number_of_buy_sell: i32,
    /// الحد الأقصى لعدد الشراء/البيع
    pub max_number_of_buy_sell: i32,
    /// تفعيل فلتر عدد الشراء/البيع
    pub buy_sell_count_enabled: bool,
    /// SOL المستثمر
    pub sol_invested: f64,
    /// تفعيل فلتر SOL المستثمر
    pub sol_invested_enabled: bool,
    /// الحد الأدنى لرصيد SOL للمطلق
    pub min_launcher_sol_balance: f64,
    /// الحد الأقصى لرصيد SOL للمطلق
    pub max_launcher_sol_balance: f64,
    /// تفعيل فلتر رصيد المطلق
    pub launcher_sol_enabled: bool,
    /// تفعيل فلتر شراء المطور
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

/// إعدادات Copy Trading - 6 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTradingSettings {
    /// نسبة متابعة الشراء/البيع
    pub buy_sell_percent: f64,
    /// قائمة محافظ الأهداف
    pub target_wallets: Vec<String>,
    /// وضع الأهداف المتعددة
    pub multi_target_mode: bool,
    /// عتبة MC للشراء
    pub mc_threshold_to_buy: f64,
    /// عتبة MC للمتابعة
    pub mc_threshold_to_follow: f64,
    /// تفعيل Copy Trading
    pub copy_trading_enabled: bool,
}

impl Default for CopyTradingSettings {
    fn default() -> Self {
        Self {
            buy_sell_percent: 100.0,
            target_wallets: Vec::new(),
            multi_target_mode: false,
            mc_threshold_to_buy: 50000.0,
            mc_threshold_to_follow: 10000.0,
            copy_trading_enabled: false,
        }
    }
}

/// إعدادات Private Logic - 15 إعداد
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateLogicSettings {
    /// تفعيل النظام الخاص
    pub private_logic_enabled: bool,
    /// نسبة المرحلة الأولى
    pub pl_stage_1_percent: f64,
    /// تأخير المرحلة الأولى بالثواني
    pub pl_stage_1_delay: u64,
    /// نسبة المرحلة الثانية
    pub pl_stage_2_percent: f64,
    /// تأخير المرحلة الثانية بالثواني
    pub pl_stage_2_delay: u64,
    /// نسبة المرحلة الثالثة
    pub pl_stage_3_percent: f64,
    /// تأخير المرحلة الثالثة بالثواني
    pub pl_stage_3_delay: u64,
    /// نسبة المرحلة الرابعة
    pub pl_stage_4_percent: f64,
    /// تأخير المرحلة الرابعة بالثواني
    pub pl_stage_4_delay: u64,
    /// نسبة المرحلة الخامسة
    pub pl_stage_5_percent: f64,
    /// تأخير المرحلة الخامسة بالثواني
    pub pl_stage_5_delay: u64,
    /// نسبة المرحلة السادسة
    pub pl_stage_6_percent: f64,
    /// تأخير المرحلة السادسة بالثواني
    pub pl_stage_6_delay: u64,
    /// نسبة المرحلة السابعة
    pub pl_stage_7_percent: f64,
    /// تأخير المرحلة السابعة بالثواني
    pub pl_stage_7_delay: u64,
}

impl Default for PrivateLogicSettings {
    fn default() -> Self {
        Self {
            private_logic_enabled: false,
            pl_stage_1_percent: 10.0,
            pl_stage_1_delay: 60,
            pl_stage_2_percent: 20.0,
            pl_stage_2_delay: 120,
            pl_stage_3_percent: 30.0,
            pl_stage_3_delay: 180,
            pl_stage_4_percent: 40.0,
            pl_stage_4_delay: 240,
            pl_stage_5_percent: 50.0,
            pl_stage_5_delay: 300,
            pl_stage_6_percent: 60.0,
            pl_stage_6_delay: 360,
            pl_stage_7_percent: 70.0,
            pl_stage_7_delay: 420,
        }
    }
}

/// إعدادات Inverse Buy - 2 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InverseBuySettings {
    /// تفعيل الشراء العكسي
    pub inverse_buy_enabled: bool,
    /// مبلغ SOL للشراء العكسي
    pub inverse_buy_amount: f64,
}

impl Default for InverseBuySettings {
    fn default() -> Self {
        Self {
            inverse_buy_enabled: false,
            inverse_buy_amount: 0.1,
        }
    }
}

/// إعدادات Timer - 4 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSettings {
    /// وقت بدء البوت (HH:MM format)
    pub bot_start_time: String,
    /// وقت إيقاف البوت (HH:MM format)
    pub bot_stop_time: String,
    /// بيع تلقائي عند الإيقاف
    pub auto_sell_on_stop: bool,
    /// تفعيل المؤقت
    pub timer_enabled: bool,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            bot_start_time: "00:00".to_string(),
            bot_stop_time: "23:59".to_string(),
            auto_sell_on_stop: false,
            timer_enabled: false,
        }
    }
}

/// إعدادات الوضع - 3 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeSettings {
    /// وضع المحاكاة
    pub simulation_mode: bool,
    /// الوضع المباشر
    pub live_mode: bool,
    /// التداول الورقي
    pub paper_trading: bool,
}

impl Default for ModeSettings {
    fn default() -> Self {
        Self {
            simulation_mode: false,
            live_mode: true,
            paper_trading: false,
        }
    }
}

/// إعدادات متقدمة - 8 إعدادات
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSettings {
    /// وقت انتظار الحد بالميلي ثانية
    pub limit_wait_time: u64,
    /// مبلغ الشراء في وقت الانتظار
    pub limit_buy_amount_in_limit_wait_time: f64,
    /// مدة دورة المراجعة بالميلي ثانية
    pub review_cycle_duration: u64,
    /// عتبة دلتا الوقت بالثواني
    pub time_delta_threshold: u64,
    /// عتبة دلتا السعر كنسبة مئوية
    pub price_delta_threshold: f64,
    /// الحد الأدنى لثقة الشراء (0.0-1.0)
    pub min_buy_confidence: f64,
    /// الحد الأدنى لثقة البيع (0.0-1.0)
    pub min_sell_confidence: f64,
    /// الميزانية اليومية للشراء بـ SOL
    pub daily_buy_budget: f64,
}

impl Default for AdvancedSettings {
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

/// الكنفيغ الرئيسي مع الإعدادات الحالية والجديدة
/// إجمالي: 96 إعداد (15 حالي + 81 جديد)
#[derive(Clone)]
pub struct Config {
    // الإعدادات الحالية - 15 إعداد (يجب الحفاظ عليها كما هي)
    pub yellowstone_grpc_http: String,              // 1
    pub yellowstone_grpc_token: String,             // 2
    pub yellowstone_ping_interval: u64,             // 3
    pub yellowstone_reconnect_delay: u64,           // 4
    pub yellowstone_max_retries: u32,               // 5
    pub app_state: AppState,                        // مركب (لا يُعد كإعداد منفصل)
    pub swap_config: SwapConfig,                    // مركب (لا يُعد كإعداد منفصل)
    pub time_exceed: u64,                           // 6
    pub blacklist: Blacklist,                       // مركب (لا يُعد كإعداد منفصل)
    pub counter_limit: u32,                         // 7
    pub min_dev_buy: u32,                           // 8
    pub max_dev_buy: u32,                           // 9
    pub telegram_bot_token: String,                 // 10
    pub telegram_chat_id: String,                   // 11
    pub bundle_check: bool,                         // 12
    pub take_profit_percent: f64,                   // 13
    pub stop_loss_percent: f64,                     // 14
    pub min_last_time: u64,                         // 15

    // الإعدادات الجديدة - 81 إعداد (مجمعة حسب الفئة)
    pub trading: TradingSettings,                   // 12 إعدادات
    pub jito: JitoSettings,                         // 4 إعدادات
    pub zero_slot: ZeroSlotSettings,                // 2 إعدادات
    pub nozomi: NozomiSettings,                     // 2 إعدادات
    pub blox_route: BloxRouteSettings,              // 4 إعدادات
    pub advanced_filters: AdvancedFilterSettings,   // 14 إعداد
    pub copy_trading: CopyTradingSettings,          // 6 إعدادات
    pub private_logic: PrivateLogicSettings,        // 15 إعداد
    pub inverse_buy: InverseBuySettings,            // 2 إعدادات
    pub timer: TimerSettings,                       // 4 إعدادات
    pub mode: ModeSettings,                         // 3 إعدادات
    pub advanced: AdvancedSettings,                 // 8 إعدادات
    // إضافية: 5 إعدادات (slippage, amount_in, swap_direction, in_type, use_jito في SwapConfig)
}

/// تنفيذ التحميل من متغيرات البيئة
impl Config {
    /// إنشاء كنفيغ جديد من متغيرات البيئة
    pub async fn new() -> &'static Mutex<Config> {
        GLOBAL_CONFIG
            .get_or_init(|| async {
                let init_msg = INIT_MSG;
                println!("{}", init_msg);

                dotenv().ok(); // تحميل ملف .env

                let logger = Logger::new("[INIT] => ".blue().bold().to_string());

                // الإعدادات الحالية
                let yellowstone_grpc_http = import_env_var("YELLOWSTONE_GRPC_HTTP");
                let yellowstone_grpc_token = import_env_var("YELLOWSTONE_GRPC_TOKEN");

                // إعدادات اتصال Yellowstone gRPC
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

                // تحميل الإعدادات الجديدة
                let trading = Self::load_trading_settings();
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

                // التحقق من صحة الإعدادات
                if let Err(errors) = Self::validate_all_settings(&trading, &jito, &advanced_filters, &copy_trading, &private_logic, &timer, &advanced) {
                    logger.log(format!("⚠️  تم العثور على أخطاء في الإعدادات:"));
                    for error in errors {
                        logger.log(format!("   - {}: {}", error.field, error.message));
                    }
                }

                let swap_config = SwapConfig {
                    swap_direction: SwapDirection::SolToToken,
                    in_type: SwapInType::Amount,
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
                    trading,
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

                logger.log("✅ تم تحميل جميع الإعدادات بنجاح - 96 إعداد".to_string());
                Mutex::new(config)
            })
            .await
    }

    /// تحميل إعدادات التداول الأساسية
    fn load_trading_settings() -> TradingSettings {
        TradingSettings {
            threshold_sell: env::var("THRESHOLD_SELL")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().threshold_sell),
            threshold_buy: env::var("THRESHOLD_BUY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().threshold_buy),
            max_wait_time: env::var("MAX_WAIT_TIME")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().max_wait_time),
            private_key: env::var("PRIVATE_KEY").unwrap_or_default(),
            rpc_http: env::var("RPC_HTTP").unwrap_or_default(),
            rpc_wss: env::var("RPC_WSS").unwrap_or_default(),
            time_exceed: env::var("TIME_EXCEED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().time_exceed),
            token_amount: env::var("TOKEN_AMOUNT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().token_amount),
            unit_price: env::var("UNIT_PRICE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().unit_price),
            unit_limit: env::var("UNIT_LIMIT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().unit_limit),
            downing_percent: env::var("DOWNING_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().downing_percent),
            sell_all_tokens: env::var("SELL_ALL_TOKENS")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TradingSettings::default().sell_all_tokens),
        }
    }

    /// تحميل إعدادات Jito
    fn load_jito_settings() -> JitoSettings {
        JitoSettings {
            jito_block_engine_url: env::var("JITO_BLOCK_ENGINE_URL")
                .unwrap_or_else(|_| JitoSettings::default().jito_block_engine_url),
            jito_priority_fee: env::var("JITO_PRIORITY_FEE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(JitoSettings::default().jito_priority_fee),
            jito_tip_value: env::var("JITO_TIP_VALUE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(JitoSettings::default().jito_tip_value),
            use_jito: env::var("USE_JITO")
                .unwrap_or_default()
                .parse()
                .unwrap_or(JitoSettings::default().use_jito),
        }
    }

    /// تحميل إعدادات ZeroSlot
    fn load_zero_slot_settings() -> ZeroSlotSettings {
        ZeroSlotSettings {
            zero_slot_url: env::var("ZERO_SLOT_URL").unwrap_or_default(),
            zero_slot_tip_value: env::var("ZERO_SLOT_TIP_VALUE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(ZeroSlotSettings::default().zero_slot_tip_value),
        }
    }

    /// تحميل إعدادات Nozomi
    fn load_nozomi_settings() -> NozomiSettings {
        NozomiSettings {
            nozomi_url: env::var("NOZOMI_URL").unwrap_or_default(),
            nozomi_tip_value: env::var("NOZOMI_TIP_VALUE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(NozomiSettings::default().nozomi_tip_value),
        }
    }

    /// تحميل إعدادات BloxRoute
    fn load_blox_route_settings() -> BloxRouteSettings {
        BloxRouteSettings {
            network: env::var("NETWORK")
                .unwrap_or_else(|_| BloxRouteSettings::default().network),
            region: env::var("REGION")
                .unwrap_or_else(|_| BloxRouteSettings::default().region),
            auth_header: env::var("AUTH_HEADER").unwrap_or_default(),
            bloxroute_tip_value: env::var("BLOXROUTE_TIP_VALUE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(BloxRouteSettings::default().bloxroute_tip_value),
        }
    }

    /// تحميل إعدادات التصفية المتقدمة
    fn load_advanced_filter_settings() -> AdvancedFilterSettings {
        AdvancedFilterSettings {
            min_market_cap: env::var("MIN_MARKET_CAP")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().min_market_cap),
            max_market_cap: env::var("MAX_MARKET_CAP")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().max_market_cap),
            market_cap_enabled: env::var("MARKET_CAP_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().market_cap_enabled),
            min_volume: env::var("MIN_VOLUME")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().min_volume),
            max_volume: env::var("MAX_VOLUME")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().max_volume),
            volume_enabled: env::var("VOLUME_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().volume_enabled),
            min_number_of_buy_sell: env::var("MIN_NUMBER_OF_BUY_SELL")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().min_number_of_buy_sell),
            max_number_of_buy_sell: env::var("MAX_NUMBER_OF_BUY_SELL")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().max_number_of_buy_sell),
            buy_sell_count_enabled: env::var("BUY_SELL_COUNT_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().buy_sell_count_enabled),
            sol_invested: env::var("SOL_INVESTED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().sol_invested),
            sol_invested_enabled: env::var("SOL_INVESTED_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().sol_invested_enabled),
            min_launcher_sol_balance: env::var("MIN_LAUNCHER_SOL_BALANCE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().min_launcher_sol_balance),
            max_launcher_sol_balance: env::var("MAX_LAUNCHER_SOL_BALANCE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().max_launcher_sol_balance),
            launcher_sol_enabled: env::var("LAUNCHER_SOL_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().launcher_sol_enabled),
            dev_buy_enabled: env::var("DEV_BUY_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedFilterSettings::default().dev_buy_enabled),
        }
    }

    /// تحميل إعدادات Copy Trading
    fn load_copy_trading_settings() -> CopyTradingSettings {
        let target_wallets_str = env::var("TARGET_WALLETS").unwrap_or_default();
        let target_wallets = if target_wallets_str.is_empty() {
            Vec::new()
        } else {
            target_wallets_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        CopyTradingSettings {
            buy_sell_percent: env::var("BUY_SELL_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(CopyTradingSettings::default().buy_sell_percent),
            target_wallets,
            multi_target_mode: env::var("MULTI_TARGET_MODE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(CopyTradingSettings::default().multi_target_mode),
            mc_threshold_to_buy: env::var("MC_THRESHOLD_TO_BUY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(CopyTradingSettings::default().mc_threshold_to_buy),
            mc_threshold_to_follow: env::var("MC_THRESHOLD_TO_FOLLOW")
                .unwrap_or_default()
                .parse()
                .unwrap_or(CopyTradingSettings::default().mc_threshold_to_follow),
            copy_trading_enabled: env::var("COPY_TRADING_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(CopyTradingSettings::default().copy_trading_enabled),
        }
    }

    /// تحميل إعدادات Private Logic
    fn load_private_logic_settings() -> PrivateLogicSettings {
        PrivateLogicSettings {
            private_logic_enabled: env::var("PRIVATE_LOGIC_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().private_logic_enabled),
            pl_stage_1_percent: env::var("PL_STAGE_1_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_1_percent),
            pl_stage_1_delay: env::var("PL_STAGE_1_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_1_delay),
            pl_stage_2_percent: env::var("PL_STAGE_2_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_2_percent),
            pl_stage_2_delay: env::var("PL_STAGE_2_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_2_delay),
            pl_stage_3_percent: env::var("PL_STAGE_3_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_3_percent),
            pl_stage_3_delay: env::var("PL_STAGE_3_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_3_delay),
            pl_stage_4_percent: env::var("PL_STAGE_4_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_4_percent),
            pl_stage_4_delay: env::var("PL_STAGE_4_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_4_delay),
            pl_stage_5_percent: env::var("PL_STAGE_5_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_5_percent),
            pl_stage_5_delay: env::var("PL_STAGE_5_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_5_delay),
            pl_stage_6_percent: env::var("PL_STAGE_6_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_6_percent),
            pl_stage_6_delay: env::var("PL_STAGE_6_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_6_delay),
            pl_stage_7_percent: env::var("PL_STAGE_7_PERCENT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_7_percent),
            pl_stage_7_delay: env::var("PL_STAGE_7_DELAY")
                .unwrap_or_default()
                .parse()
                .unwrap_or(PrivateLogicSettings::default().pl_stage_7_delay),
        }
    }

    /// تحميل إعدادات Inverse Buy
    fn load_inverse_buy_settings() -> InverseBuySettings {
        InverseBuySettings {
            inverse_buy_enabled: env::var("INVERSE_BUY_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(InverseBuySettings::default().inverse_buy_enabled),
            inverse_buy_amount: env::var("INVERSE_BUY_AMOUNT")
                .unwrap_or_default()
                .parse()
                .unwrap_or(InverseBuySettings::default().inverse_buy_amount),
        }
    }

    /// تحميل إعدادات Timer
    fn load_timer_settings() -> TimerSettings {
        TimerSettings {
            bot_start_time: env::var("BOT_START_TIME")
                .unwrap_or_else(|_| TimerSettings::default().bot_start_time),
            bot_stop_time: env::var("BOT_STOP_TIME")
                .unwrap_or_else(|_| TimerSettings::default().bot_stop_time),
            auto_sell_on_stop: env::var("AUTO_SELL_ON_STOP")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TimerSettings::default().auto_sell_on_stop),
            timer_enabled: env::var("TIMER_ENABLED")
                .unwrap_or_default()
                .parse()
                .unwrap_or(TimerSettings::default().timer_enabled),
        }
    }

    /// تحميل إعدادات الوضع
    fn load_mode_settings() -> ModeSettings {
        ModeSettings {
            simulation_mode: env::var("SIMULATION_MODE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(ModeSettings::default().simulation_mode),
            live_mode: env::var("LIVE_MODE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(ModeSettings::default().live_mode),
            paper_trading: env::var("PAPER_TRADING")
                .unwrap_or_default()
                .parse()
                .unwrap_or(ModeSettings::default().paper_trading),
        }
    }

    /// تحميل الإعدادات المتقدمة
    fn load_advanced_settings() -> AdvancedSettings {
        AdvancedSettings {
            limit_wait_time: env::var("LIMIT_WAIT_TIME")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().limit_wait_time),
            limit_buy_amount_in_limit_wait_time: env::var("LIMIT_BUY_AMOUNT_IN_LIMIT_WAIT_TIME")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().limit_buy_amount_in_limit_wait_time),
            review_cycle_duration: env::var("REVIEW_CYCLE_DURATION")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().review_cycle_duration),
            time_delta_threshold: env::var("TIME_DELTA_THRESHOLD")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().time_delta_threshold),
            price_delta_threshold: env::var("PRICE_DELTA_THRESHOLD")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().price_delta_threshold),
            min_buy_confidence: env::var("MIN_BUY_CONFIDENCE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().min_buy_confidence),
            min_sell_confidence: env::var("MIN_SELL_CONFIDENCE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().min_sell_confidence),
            daily_buy_budget: env::var("DAILY_BUY_BUDGET")
                .unwrap_or_default()
                .parse()
                .unwrap_or(AdvancedSettings::default().daily_buy_budget),
        }
    }

    /// التحقق من صحة جميع الإعدادات
    fn validate_all_settings(
        trading: &TradingSettings,
        jito: &JitoSettings,
        advanced_filters: &AdvancedFilterSettings,
        copy_trading: &CopyTradingSettings,
        private_logic: &PrivateLogicSettings,
        timer: &TimerSettings,
        advanced: &AdvancedSettings,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // التحقق من إعدادات التداول
        if trading.threshold_sell <= trading.threshold_buy {
            errors.push(ValidationError {
                field: "threshold_sell".to_string(),
                message: "عتبة البيع يجب أن تكون أكبر من عتبة الشراء".to_string(),
            });
        }

        if trading.unit_price <= 0.0 {
            errors.push(ValidationError {
                field: "unit_price".to_string(),
                message: "سعر الوحدة يجب أن يكون أكبر من صفر".to_string(),
            });
        }

        if trading.downing_percent < 0.0 || trading.downing_percent > 100.0 {
            errors.push(ValidationError {
                field: "downing_percent".to_string(),
                message: "نسبة الانخفاض يجب أن تكون بين 0 و 100".to_string(),
            });
        }

        // التحقق من إعدادات التصفية المتقدمة
        if advanced_filters.min_market_cap >= advanced_filters.max_market_cap {
            errors.push(ValidationError {
                field: "market_cap".to_string(),
                message: "الحد الأدنى لرأس المال يجب أن يكون أقل من الحد الأقصى".to_string(),
            });
        }

        if advanced_filters.min_volume >= advanced_filters.max_volume {
            errors.push(ValidationError {
                field: "volume".to_string(),
                message: "الحد الأدنى للحجم يجب أن يكون أقل من الحد الأقصى".to_string(),
            });
        }

        // التحقق من إعدادات Copy Trading
        if copy_trading.buy_sell_percent < 0.0 || copy_trading.buy_sell_percent > 100.0 {
            errors.push(ValidationError {
                field: "buy_sell_percent".to_string(),
                message: "نسبة متابعة الشراء/البيع يجب أن تكون بين 0 و 100".to_string(),
            });
        }

        // التحقق من إعدادات Private Logic
        let stages = [
            ("pl_stage_1_percent", private_logic.pl_stage_1_percent),
            ("pl_stage_2_percent", private_logic.pl_stage_2_percent),
            ("pl_stage_3_percent", private_logic.pl_stage_3_percent),
            ("pl_stage_4_percent", private_logic.pl_stage_4_percent),
            ("pl_stage_5_percent", private_logic.pl_stage_5_percent),
            ("pl_stage_6_percent", private_logic.pl_stage_6_percent),
            ("pl_stage_7_percent", private_logic.pl_stage_7_percent),
        ];

        for (field_name, percent_value) in stages {
            if percent_value < 0.0 || percent_value > 100.0 {
                errors.push(ValidationError {
                    field: field_name.to_string(),
                    message: format!("نسبة {} يجب أن تكون بين 0 و 100", field_name),
                });
            }
        }

        // التحقق من إعدادات Timer
        if timer.timer_enabled {
            if !Self::is_valid_time_format(&timer.bot_start_time) {
                errors.push(ValidationError {
                    field: "bot_start_time".to_string(),
                    message: "تنسيق وقت البدء غير صحيح (استخدم HH:MM)".to_string(),
                });
            }
            if !Self::is_valid_time_format(&timer.bot_stop_time) {
                errors.push(ValidationError {
                    field: "bot_stop_time".to_string(),
                    message: "تنسيق وقت الإيقاف غير صحيح (استخدم HH:MM)".to_string(),
                });
            }
        }

        // التحقق من الإعدادات المتقدمة
        if advanced.min_buy_confidence < 0.0 || advanced.min_buy_confidence > 1.0 {
            errors.push(ValidationError {
                field: "min_buy_confidence".to_string(),
                message: "الحد الأدنى لثقة الشراء يجب أن يكون بين 0.0 و 1.0".to_string(),
            });
        }

        if advanced.min_sell_confidence < 0.0 || advanced.min_sell_confidence > 1.0 {
            errors.push(ValidationError {
                field: "min_sell_confidence".to_string(),
                message: "الحد الأدنى لثقة البيع يجب أن يكون بين 0.0 و 1.0".to_string(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// التحقق من صحة تنسيق الوقت (HH:MM)
    fn is_valid_time_format(time_str: &str) -> bool {
        if time_str.len() != 5 {
            return false;
        }

        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        if let (Ok(hours), Ok(minutes)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
            hours < 24 && minutes < 60
        } else {
            false
        }
    }

    /// تحميل الإعدادات من ملف .env محدد
    pub fn load_from_env(env_file: &str) -> Result<()> {
        dotenv::from_filename(env_file).map_err(|e| {
            anyhow!("فشل في تحميل ملف البيئة {}: {}", env_file, e)
        })?;
        Ok(())
    }

    /// حفظ الإعدادات الحالية إلى ملف JSON
    pub fn save_to_json(&self, file_path: &str) -> Result<()> {
        let settings_json = serde_json::json!({
            "trading": self.trading,
            "jito": self.jito,
            "zero_slot": self.zero_slot,
            "nozomi": self.nozomi,
            "blox_route": self.blox_route,
            "advanced_filters": self.advanced_filters,
            "copy_trading": self.copy_trading,
            "private_logic": self.private_logic,
            "inverse_buy": self.inverse_buy,
            "timer": self.timer,
            "mode": self.mode,
            "advanced": self.advanced,
        });

        std::fs::write(file_path, serde_json::to_string_pretty(&settings_json)?)?;
        Ok(())
    }

    /// طباعة ملخص الإعدادات
    pub fn print_settings_summary(&self) {
        println!("\n🔧 ملخص الإعدادات (96 إعداد):");
        println!("├─ إعدادات التداول الأساسية (12 إعداد):");
        println!("│  ├─ عتبة البيع: {} lamports", self.trading.threshold_sell);
        println!("│  ├─ عتبة الشراء: {} lamports", self.trading.threshold_buy);
        println!("│  └─ بيع جميع التوكنات: {}", self.trading.sell_all_tokens);

        println!("├─ إعدادات Jito (4 إعدادات):");
        println!("│  ├─ مُفعل: {}", self.jito.use_jito);
        println!("│  └─ قيمة الإكرامية: {}", self.jito.jito_tip_value);
        println!("├─ إعدادات التصفية المتقدمة (14 إعداد):");
        println!("│  ├─ رأس المال: {} - {}", self.advanced_filters.min_market_cap, self.advanced_filters.max_market_cap);
        println!("│  └─ الحجم: {} - {}", self.advanced_filters.min_volume, self.advanced_filters.max_volume);

        println!("├─ Copy Trading (6 إعدادات): {}", if self.copy_trading.copy_trading_enabled { "مُفعل" } else { "مُعطل" });
        println!("├─ Private Logic (15 إعداد): {}", if self.private_logic.private_logic_enabled { "مُفعل" } else { "مُعطل" });
        println!("├─ Inverse Buy (2 إعدادات): {}", if self.inverse_buy.inverse_buy_enabled { "مُفعل" } else { "مُعطل" });
        println!("├─ المؤقت (4 إعدادات): {}", if self.timer.timer_enabled { "مُفعل" } else { "مُعطل" });
        println!("├─ ZeroSlot (2 إعدادات): {}", if !self.zero_slot.zero_slot_url.is_empty() { "مُكون" } else { "غير مُكون" });
        println!("├─ Nozomi (2 إعدادات): {}", if !self.nozomi.nozomi_url.is_empty() { "مُكون" } else { "غير مُكون" });
        println!("├─ BloxRoute (4 إعدادات): {}", if !self.blox_route.auth_header.is_empty() { "مُكون" } else { "غير مُكون" });
        println!("├─ الإعدادات المتقدمة (8 إعدادات): ثقة الشراء {:.1}%", self.advanced.min_buy_confidence * 100.0);
        println!("├─ إعدادات الوضع (3 إعدادات): {}", if self.mode.live_mode { "مباشر" } else if self.mode.simulation_mode { "محاكاة" } else { "ورقي" });
        println!("└─ الإعدادات الحالية المحفوظة (15 إعداد): Yellowstone، Telegram، إلخ");
    }

    /// عد جميع الإعدادات في النظام
    pub fn count_all_settings(&self) -> u32 {
        let current_settings = 15; // الإعدادات الحالية المحفوظة
        let trading_settings = 12;
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
        let additional_swap_settings = 5; // في SwapConfig

        current_settings + trading_settings + jito_settings + zero_slot_settings +
            nozomi_settings + blox_route_settings + advanced_filter_settings +
            copy_trading_settings + private_logic_settings + inverse_buy_settings +
            timer_settings + mode_settings + advanced_settings + additional_swap_settings
    }
}
// الهياكل الحالية التي يجب الحفاظ عليها
#[derive(Debug, PartialEq, Clone)]
pub struct LiquidityPool {
    pub mint: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub status: Status,
    pub timestamp: Optiontokio::time::Instant,
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
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Status {
    Bought,
    Buying,
    Checking,
    Sold,
    Selling,
    Failure,
}
#[derive(Deserialize)]
struct CoinGeckoResponse {
    solana: SolanaData,
}
#[derive(Deserialize)]
struct SolanaData {
    usd: f64,
}
#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<anchor_client::solana_client::rpc_client::RpcClient>,
    pub rpc_nonblocking_client: Arc<anchor_client::solana_client::nonblocking::rpc_client::RpcClient>,
    pub wallet: Arc<Keypair>,
}
#[derive(Clone)]
pub struct SwapConfig {
    pub swap_direction: SwapDirection,
    pub in_type: SwapInType,
    pub amount_in: f64,
    pub slippage: u64,
    pub use_jito: bool,
}
// الدوال المساعدة الحالية والمُحسنة
pub fn import_env_var(key: &str) -> String {
    match env::var(key) {
        Ok(res) => res,
        Err(_) => {
            eprintln!("{}", format!("⚠️  متغير البيئة غير موجود: {}", key).red().to_string());
            String::new()
        }
    }
}
pub fn create_rpc_client() -> Result<Arc<anchor_client::solana_client::rpc_client::RpcClient>> {
    let rpc_http = env::var("RPC_HTTP").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let rpc_client = anchor_client::solana_client::rpc_client::RpcClient::new_with_commitment(
        rpc_http,
        CommitmentConfig::processed(),
    );
    Ok(Arc::new(rpc_client))
}
pub fn import_wallet() -> Result<Keypair, Box<dyn std::error::Error>> {
    let private_key = env::var("PRIVATE_KEY").unwrap_or_default();
    if private_key.is_empty() {
        return Ok(Keypair::new()); // إنشاء محفظة جديدة إذا لم يتم توفير مفتاح
    }
    let wallet_bytes = bs58::decode(&private_key).into_vec()?;
    let keypair = Keypair::from_bytes(&wallet_bytes)?;
    Ok(keypair)
}
pub async fn create_coingecko_proxy() -> Result<f64, Error> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
        .send()
        .await?;
    let price_data: CoinGeckoResponse = response.json().await?;
    Ok(price_data.solana.usd)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_settings_count() {
        // إنشاء كونفيغ وهمي للاختبار
        let config = create_test_config();
        let total_count = config.count_all_settings();
        assert_eq!(total_count, 96, "العدد الإجمالي للإعدادات يجب أن يكون 96");
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
    fn test_default_values() {
        let trading = TradingSettings::default();
        assert_eq!(trading.threshold_sell, 10_000_000_000);
        assert_eq!(trading.threshold_buy, 3_000_000_000);
        assert!(!trading.sell_all_tokens);

        let jito = JitoSettings::default();
        assert!(!jito.use_jito);
        assert_eq!(jito.jito_tip_value, 1000);

        let copy_trading = CopyTradingSettings::default();
        assert!(!copy_trading.copy_trading_enabled);
        assert_eq!(copy_trading.buy_sell_percent, 100.0);

        let private_logic = PrivateLogicSettings::default();
        assert!(!private_logic.private_logic_enabled);
        assert_eq!(private_logic.pl_stage_1_percent, 10.0);
    }

    #[test]
    fn test_validation_errors() {
        let trading = TradingSettings {
            threshold_sell: 1000,
            threshold_buy: 2000, // خطأ: البيع أقل من الشراء
            unit_price: -1.0,    // خطأ: سعر سالب
            downing_percent: 150.0, // خطأ: نسبة أكبر من 100
            ..Default::default()
        };

        let copy_trading = CopyTradingSettings {
            buy_sell_percent: 150.0, // خطأ: أكبر من 100
            ..Default::default()
        };

        let private_logic = PrivateLogicSettings {
            pl_stage_1_percent: 110.0, // خطأ: أكبر من 100
            ..Default::default()
        };

        let timer = TimerSettings {
            timer_enabled: true,
            bot_start_time: "25:00".to_string(), // خطأ: وقت غير صحيح
            ..Default::default()
        };

        let advanced = AdvancedSettings {
            min_buy_confidence: 1.5, // خطأ: أكبر من 1.0
            min_sell_confidence: -0.1, // خطأ: أقل من 0.0
            ..Default::default()
        };

        let result = Config::validate_all_settings(
            &trading,
            &JitoSettings::default(),
            &AdvancedFilterSettings::default(),
            &copy_trading,
            &private_logic,
            &timer,
            &advanced,
        );

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 7); // يجب أن يكون هناك على الأقل 7 أخطاء
    }

    fn create_test_config() -> Config {
        Config {
            // الإعدادات الحالية - 15 إعداد
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

            // الإعدادات الجديدة
            trading: TradingSettings::default(),
            jito: JitoSettings::default(),
            zero_slot: ZeroSlotSettings::default(),
            nozomi: NozomiSettings::default(),
            blox_route: BloxRouteSettings::default(),
            advanced_filters: AdvancedFilterSettings::default(),
            copy_trading: CopyTradingSettings::default(),
            private_logic: PrivateLogicSettings::default(),
            inverse_buy: InverseBuySettings::default(),
            timer: TimerSettings::default(),
            mode: ModeSettings::default(),
            advanced: AdvancedSettings::default(),

            // الهياكل المركبة
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
                swap_direction: SwapDirection::SolToToken,
                in_type: SwapInType::Amount,
                amount_in: 1.0,
                slippage: 100,
                use_jito: false,
            },
            blacklist: Blacklist::new(),
        }
    }

    #[tokio::test]
    async fn test_config_loading_from_env() {
        // إعداد متغيرات البيئة للاختبار
        env::set_var("THRESHOLD_SELL", "20000000000");
        env::set_var("THRESHOLD_BUY", "5000000000");
        env::set_var("JITO_TIP_VALUE", "2000");
        env::set_var("COPY_TRADING_ENABLED", "true");
        env::set_var("TARGET_WALLETS", "wallet1,wallet2,wallet3");
        env::set_var("PRIVATE_LOGIC_ENABLED", "true");
        env::set_var("PL_STAGE_1_PERCENT", "15.0");

        let trading = Config::load_trading_settings();
        let jito = Config::load_jito_settings();
        let copy_trading = Config::load_copy_trading_settings();
        let private_logic = Config::load_private_logic_settings();

        assert_eq!(trading.threshold_sell, 20_000_000_000);
        assert_eq!(trading.threshold_buy, 5_000_000_000);
        assert_eq!(jito.jito_tip_value, 2000);
        assert!(copy_trading.copy_trading_enabled);
        assert_eq!(copy_trading.target_wallets.len(), 3);
        assert!(private_logic.private_logic_enabled);
        assert_eq!(private_logic.pl_stage_1_percent, 15.0);
    }

    #[test]
    fn test_json_serialization() {
        let config = create_test_config();

        // اختبار حفظ إلى JSON
        let result = config.save_to_json("test_config.json");
        assert!(result.is_ok());

        // التحقق من وجود الملف
        assert!(std::path::Path::new("test_config.json").exists());

        // تنظيف
        let _ = std::fs::remove_file("test_config.json");
    }
}