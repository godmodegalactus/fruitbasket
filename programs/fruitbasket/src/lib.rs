use anchor_lang::prelude::*;
use std::{mem::size_of, str::Utf8Error};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
const MAX_NB_TOKENS : usize = 100;
const MAX_NB_COMPONENTS: usize = 10;
const FRUIT_BASKET_GROUP : &[u8] = b"fruitbasket_group";
const FRUIT_BASKET_CACHE : &[u8] = b"fruitbasket_cache";

#[program]
pub mod fruitbasket {
    use super::*;

    pub fn initialize_group(ctx: Context<InitializeGroup>, 
            _bump_group: u8, 
            _bump_cache: u8,
            base_mint: Pubkey,
            base_mint_name: String,
            token_data: Vec<TokenInfo>) -> ProgramResult {
        assert!( token_data.len() < MAX_NB_TOKENS );
        
        let mut group = ctx.accounts.fruit_basket_grp.load_init()?;
        group.owner = *ctx.accounts.owner.key;
        group.token_count = token_data.len() as u8;
        group.base_mint = base_mint;
        let mint_name : &[u8] = base_mint_name[..].as_bytes();
        group.base_mint_name[..8].clone_from_slice(mint_name);
        group.number_of_baskets = 0;
        let i = 0;
        for token in token_data.iter() {
            group.token_mints[i] = token.mint;
            group.oracles[i] = token.oracle;
            group.token_names[10*i..10*(i+1)].clone_from_slice(&token.name);
        }
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(bump_group: u8, bump_cache: u8)]
pub struct InitializeGroup<'info> {
    #[account(signer)]
    owner : AccountInfo<'info>,

    #[account( init,
        seeds = [FRUIT_BASKET_GROUP, &owner.key.to_bytes()],
        bump = bump_group, 
        payer = owner, 
        space = 8 + size_of::<FruitBasketGroup>() )]
    fruit_basket_grp : AccountLoader<'info, FruitBasketGroup>,

    #[account( init,
        seeds = [FRUIT_BASKET_CACHE, &owner.key.to_bytes()],
        bump = bump_group, 
        payer = owner, 
        space = 8 + size_of::<Cache>() )]
    cache : AccountLoader<'info, Cache>,

    system_program : Program<'info, System>,
}

/// Fruit basket group
/// This contains all the data common for the market
/// Owner for fruit basket echosystem should initialize this class should initialize this class
#[account(zero_copy)]
pub struct FruitBasketGroup {
    pub owner: Pubkey,              // owner
    pub token_count: u8,           // number of tokens that can be handled
    pub base_mint: Pubkey,          // usdc public key
    pub base_mint_name : [u8; 10],  // name of base / USDC
    pub token_mints: [Pubkey; MAX_NB_TOKENS], // token mints
    pub oracles: [Pubkey;MAX_NB_TOKENS],      // oracle keys
    pub token_names: [u8; MAX_NB_TOKENS*10],
    pub number_of_baskets : u32,
}

#[account()]
pub struct Basket {
    pub basket_name: [u8; 128],
    pub number_of_components: u8,    // basket description
    pub components : [u8; MAX_NB_COMPONENTS], // components by index in the fruit basket group
    pub component_size : [u64; MAX_NB_COMPONENTS],
}

#[account(zero_copy)]
pub struct Cache {
    pub last_price: [u64; MAX_NB_TOKENS],
    pub last_exp: [u8; MAX_NB_TOKENS],
    pub last_confidence: [u32; MAX_NB_TOKENS],
}

#[derive(AnchorSerialize, AnchorDeserialize, Default, Clone, Copy)]
pub struct TokenInfo {
    name : [u8;10],
    mint : Pubkey,
    oracle : Pubkey,
}


impl<'info> FruitBasketGroup {
    pub fn get_token_name( &self, token_index : usize) -> Result<&str, Utf8Error> {
        assert!(token_index < MAX_NB_TOKENS);
        std::str::from_utf8(&self.token_names[10*token_index..10*(token_index+1)])
    }
}