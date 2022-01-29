use crate::*;
/// Fruit basket group
/// This contains all the data common for the market
/// Owner for fruit basket echosystem should initialize this class should initialize this class
#[account(zero_copy)]
pub struct FruitBasketGroup {
    pub owner: Pubkey,              // owner
    pub token_count: u64,            // number of tokens that can be handled
    pub base_mint: Pubkey,          // usdc public key
    pub base_mint_name : [u8; 10],  // name of base / USDC
    pub number_of_baskets : u64,    // number of baskets currenly create
    pub nb_users: u8,              // number of users registered
    pub quote_token_transaction_pool : Pubkey,
}

/// state to define a basket
#[account()]
pub struct Basket {
    pub magic : u32,
    pub basket_name: [u8; 128],      // basket name
    pub desc: [u8; 256],
    pub number_of_components: u8,    // basket description
    pub components : [BasketComponentDescription; 10],
    pub basket_mint : Pubkey,
    pub last_price : u64,
    pub confidence : u64,
    pub decimal : u8,               // always 6
}

#[account()]
pub struct TokenDescription
{
    pub magic : u32,
    pub id : u64,
    pub token_mint: Pubkey,     // token mints
    pub price_oracle: Pubkey,   // oracle keys
    pub product_oracle: Pubkey, // product info keys
    pub token_name: [u8; 10],      // token names
    pub token_pool : Pubkey, // pool for each token 
    pub token_decimal : u8,     // number of decimal places for token (1 SOL -> 10^9 lamports = 9 decimal places )
    pub token_open_orders : Pubkey,
    pub market : Pubkey,
    pub cache : Cache,
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
#[repr(C)]
pub struct Cache {
    pub last_price: u64,
    pub last_exp: i32,
    pub last_confidence: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
#[repr(C)]
pub struct BasketComponentDescription{
    pub token_mint : Pubkey,
    pub amount : u64,
    pub decimal : u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ContextSide {
    Buy,
    Sell,
}

#[account(zero_copy)]
pub struct BasketTradeContext
{
    // to find current trade context which are bieng processed by offchain programs.
    pub magic : u32,
    pub side: ContextSide,
    pub basket: Pubkey,
    pub reverting : u8,
    // amount of basket tokens
    pub amount : u64,
    // tracks usdc used
    pub usdc_amount_left : u64,
    pub quote_token_account: Pubkey,
    pub basket_token_account : Pubkey,
    // contains number of usdc deposited by user
    pub initial_usdc_transfer_amount : u64,
    pub created_on : u64,
    pub token_mints : [Pubkey; 10],
    // tracks number of tokens to be treated
    pub token_amounts: [u64; 10],
    // initial amount of tokens to be transfered
    pub initial_token_amounts: [u64; 10],
    pub tokens_treated: [u8; 10],
}

pub const BASKET_TRADE_CONTEXT_MAGIC : u32 = 0xba873cfd;
pub const BASKET_DESC_MAGIC : u32 = 0xa435efbb;
pub const TOKEN_DESC_MAGIC : u32 = 0xcde78987;