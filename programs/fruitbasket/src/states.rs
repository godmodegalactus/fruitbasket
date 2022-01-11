use crate::*;
use fixed::types::I80F48;
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
    pub last_price : u64,
    pub confidence : u64,
    pub decimal : u8,
}

#[account(zero_copy)]
pub struct Cache {
    pub last_price: [u64; 20],
    pub last_exp: [i32; 20],
    pub last_confidence: [u64; 20],
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

impl Basket {
    pub fn update_price(&mut self, cache : &Cache) -> ProgramResult {
        let mut price  = I80F48::from_num(0);
        let mut confidence  = I80F48::from_num(0);
        let decimal : u8 = 6;
        
        for i in 0..self.number_of_components {
            let comp = self.components[i as usize];
            let token_index : usize = comp.token_index as usize;
            let mut comp_price = cache.last_price[token_index].checked_mul(comp.amount).unwrap().checked_div(10u64.pow(comp.decimal as u32)).unwrap();
            let mut comp_conf = cache.last_confidence[token_index].checked_mul(comp.amount).unwrap().checked_div(10u64.pow(comp.decimal as u32)).unwrap();

            //pyth decimal is negative usual decimal
            let pyth_decimal = if cache.last_exp[token_index] >= 0 { 0 } else {-cache.last_exp[token_index] as u8};
            
            if pyth_decimal != decimal {
                if pyth_decimal > decimal {
                    let exp : u32 = (pyth_decimal - decimal) as u32;
                    comp_price = comp_price.checked_div(10u64.pow(exp)).unwrap();
                    comp_conf = comp_conf.checked_div(10u64.pow(exp)).unwrap();
                }
                else {
                    let exp : u32 = (decimal - pyth_decimal) as u32;
                    comp_price = comp_price.checked_mul(10u64.pow(exp)).unwrap();
                    comp_conf = comp_conf.checked_mul(10u64.pow(exp)).unwrap();
                }
            }
            price = price.checked_add( I80F48::from_num(comp_price) ).unwrap();
            confidence = confidence.checked_add(I80F48::from_num(comp_conf) ).unwrap();
        }
        self.last_price = price.to_num::<u64>();
        self.confidence = confidence.to_num::<u64>();
        self.decimal = decimal;
        let msg2= format!("total price {} confidence {}", price.to_num::<u64>(), confidence.to_num::<u64>());
            msg!(&msg2[..]);
        assert!(self.last_price > 0);
        Ok(())
    }
}