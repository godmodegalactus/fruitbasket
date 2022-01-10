use crate::*;
/// Fruit basket group
/// This contains all the data common for the market
/// Owner for fruit basket echosystem should initialize this class should initialize this class
#[account(zero_copy)]
pub struct FruitBasketGroup {
    pub owner: Pubkey,              // owner
    pub token_count: u8,            // number of tokens that can be handled
    pub base_mint: Pubkey,          // usdc public key
    pub base_mint_name : [u8; 10],  // name of base / USDC
    pub number_of_baskets : u8,    // number of baskets currenly create
    pub nb_users: u8,              // number of users registered
    pub token_description : [TokenDescription; 20],
}

/// state to define a basket
#[account()]
pub struct Basket {
    pub basket_name: [u8; 128],      // basket name
    pub desc: [u8; 256],
    pub number_of_components: u8,    // basket description
    pub components : [BasketComponentDescription; 10],
    pub basket_mint : Pubkey,
}

#[account(zero_copy)]
pub struct Cache {
    pub last_price: [u64; 20],
    pub last_exp: [u8; 20],
    pub last_confidence: [u32; 20],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
#[repr(C)]
pub struct TokenDescription
{
    pub token_mint: Pubkey,     // token mints
    pub price_oracle: Pubkey,   // oracle keys
    pub product_oracle: Pubkey, // product info keys
    pub token_name: [u8; 10],      // token names
    pub token_pool : Pubkey, // pool for each token 
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy)]
#[repr(C)]
pub struct BasketComponentDescription{
    token_index : u8,
    amount : u64,
    decimal : u8,
}