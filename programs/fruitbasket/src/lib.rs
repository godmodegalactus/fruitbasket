use anchor_lang::prelude::*;
use std::{mem::size_of, str::Utf8Error};
use anchor_spl::token::{self, Token, SetAuthority, TokenAccount};
use spl_token::instruction::{AuthorityType};
use spl_token::instruction::{initialize_account};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
const MAX_NB_TOKENS : usize = 20;
const MAX_NB_COMPONENTS: usize = 10;
const FRUIT_BASKET_GROUP : &[u8] = b"fruitbasket_group";
const FRUIT_BASKET_CACHE : &[u8] = b"fruitbasket_cache";
const FRUIT_BASKET_AUTHORITY : &[u8] = b"fruitbasket_auth";

#[program]
pub mod fruitbasket {
    use super::*;

    pub fn initialize_group(ctx: Context<InitializeGroup>, 
            _bump_group: u8, 
            _bump_cache: u8,
            base_mint: Pubkey,
            base_mint_name: String) -> ProgramResult {
        // init cache
        ctx.accounts.cache.load_init()?;
        // init gr
        let mut group = ctx.accounts.fruit_basket_grp.load_init()?;
        group.owner = *ctx.accounts.owner.key;
        group.token_count = 0;
        group.base_mint = base_mint;
        let size : usize = if base_mint_name.len() > 10 {10} else {base_mint_name.len()} ;
        let mint_name : &[u8] = base_mint_name[..size].as_bytes();
        group.base_mint_name[..size].clone_from_slice(mint_name);
        group.number_of_baskets = 0;
        Ok(())
    }

    pub fn add_token(ctx: Context<AddToken>, name: String) -> ProgramResult {
        let mut group = ctx.accounts.fruit_basket_grp.load_mut()?;
        let current : usize = group.token_count as usize;
        assert!(current < MAX_NB_TOKENS);
        group.token_mints[current] = *ctx.accounts.mint.key;
        group.oracles[current] = *ctx.accounts.oracle.key;
        let size : usize = if name.len() > 10 {10} else {name.len()};
        group.token_names[10*current..10*(10*current + size)].clone_from_slice(&name[..size].as_bytes());
        if size < 10 {
            group.token_names[10*current + size + 1] = 0;
        }
        let (authority, _bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
        {
            // change authority of token pool to authority            
            let cpi_accounts = SetAuthority {
                account_or_mint: ctx.accounts.token_pool.to_account_info().clone(),
                current_authority: ctx.accounts.owner.clone(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi =  CpiContext::new(cpi_program, cpi_accounts);
            token::set_authority(cpi, AuthorityType::AccountOwner, Some(authority))?;
        }
        group.token_pools[current] = *ctx.accounts.token_pool.to_account_info().key;

        group.token_count = group.token_count + 1;
        //group.token
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(bump_group: u8, bump_cache: u8)]
pub struct InitializeGroup<'info> {
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account( init,
        seeds = [FRUIT_BASKET_GROUP, &owner.key.to_bytes()],
        bump = bump_group, 
        payer = owner, 
        space = 8 + size_of::<FruitBasketGroup>() )]
    fruit_basket_grp : AccountLoader<'info, FruitBasketGroup>,

    #[account( init,
        seeds = [FRUIT_BASKET_CACHE, &owner.key.to_bytes()],
        bump = bump_cache, 
        payer = owner, 
        space = 8 + size_of::<Cache>() )]
    cache : AccountLoader<'info, Cache>,

    system_program : Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddToken<'info>{
    #[account(signer)]
    owner : AccountInfo<'info>,

    #[account(mut)]
    fruit_basket_grp : AccountLoader<'info, FruitBasketGroup>,

    mint : AccountInfo<'info>,
    oracle : AccountInfo<'info>,
    #[account(mut, 
              constraint = token_pool.owner == *owner.key,
              constraint = token_pool.mint == *mint.key)]
    token_pool : Account<'info, TokenAccount>,
    token_program : Program<'info, anchor_spl::token::Token>,
}

/// Fruit basket group
/// This contains all the data common for the market
/// Owner for fruit basket echosystem should initialize this class should initialize this class
#[account(zero_copy)]
pub struct FruitBasketGroup {
    pub owner: Pubkey,              // owner
    pub token_count: u8,            // number of tokens that can be handled
    pub base_mint: Pubkey,          // usdc public key
    pub base_mint_name : [u8; 10],  // name of base / USDC
    pub token_mints: [Pubkey; 20],  // token mints
    pub oracles: [Pubkey;20],       // oracle keys
    pub token_names: [u8; 20],      // token names
    pub number_of_baskets : u32,    // number of baskets currenly created
    pub token_pools : [Pubkey; 20], // pool for each token  
}

#[account()]
pub struct Basket {
    pub basket_name: [u8; 128],
    pub number_of_components: u8,    // basket description
    pub components : [u8; 10], // components by index in the fruit basket group
    pub component_size : [u64; 10],
}

#[account(zero_copy)]
pub struct Cache {
    pub last_price: [u64; 20],
    pub last_exp: [u8; 20],
    pub last_confidence: [u32; 20],
}

impl<'info> FruitBasketGroup {
    pub fn get_token_name( &self, token_index : usize) -> Result<&str, Utf8Error> {
        assert!(token_index < MAX_NB_TOKENS);
        std::str::from_utf8(&self.token_names[10*token_index..10*(token_index+1)])
    }
}