use anchor_lang::prelude::*;
use std::{mem::size_of};
use anchor_spl::token::{self, SetAuthority, TokenAccount, Mint, InitializeMint};
use spl_token::instruction::{AuthorityType};
use pyth_client::Price;

mod instructions;
use instructions::*;
mod states;
use states::*;


declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
const MAX_NB_TOKENS : usize = 20;
const MAX_NB_COMPONENTS: usize = 10;
const FRUIT_BASKET_GROUP : &[u8] = b"fruitbasket_group";
const FRUIT_BASKET_CACHE : &[u8] = b"fruitbasket_cache";
const FRUIT_BASKET_AUTHORITY : &[u8] = b"fruitbasket_auth";
const FRUIT_BASKET : &[u8] = b"fruitbasket";
const FRUIT_BASKET_MINT : &[u8] = b"fruitbasket_mint";

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
        group.nb_users = 0;
        
        //pre allocate programming addresses
        Pubkey::find_program_address(&[FRUIT_BASKET.as_ref(), &[0]], ctx.program_id);
        Pubkey::find_program_address(&[FRUIT_BASKET_MINT.as_ref(), &[0]], ctx.program_id);
        Ok(())
    }

    pub fn add_token(ctx: Context<AddToken>, name: String) -> ProgramResult {
        assert!(name.len() <=10 );
        let mut group = ctx.accounts.fruit_basket_grp.load_mut()?;
        let current : usize = group.token_count as usize;
        assert!(current < MAX_NB_TOKENS);
        group.token_description[current].token_mint = *ctx.accounts.mint.key;
        group.token_description[current].price_oracle = *ctx.accounts.price_oracle.key;
        group.token_description[current].price_oracle = *ctx.accounts.product_oracle.key;
        group.token_description[current].token_name[..name.len()].clone_from_slice(name[..].as_bytes());
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
        group.token_description[current].token_pool = *ctx.accounts.token_pool.to_account_info().key;

        group.token_count += 1;
        //group.token
        Ok(())
    }

    // add basket
    pub fn add_basket(ctx : Context<AddBasket>, 
        basket_number : u8, 
        _basket_bump : u8, 
        _basket_mint_bump : u8,
        basket_name : String, 
        basket_desc : String,
        basket_components : Vec<BasketComponentDescription>) -> ProgramResult
    {
        assert!(basket_components.len()<MAX_NB_COMPONENTS);
        assert!(basket_components.len()>1);
        let mut group = ctx.accounts.group.load_mut()?;
        assert!(group.number_of_baskets == basket_number);
        let basket = &mut ctx.accounts.basket;
        basket.basket_name[..basket_name.len()].copy_from_slice(basket_name[..].as_bytes());
        basket.desc[..basket_desc.len()].copy_from_slice(basket_desc[..].as_bytes());

        for i in 0..basket_components.len(){
            let component = basket_components[i];
            basket.components[i] = component;
        }
        
        let (authority, _bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
        // initialize mint
        {
            let cpi = CpiContext::new(
                ctx.accounts.token_program.to_account_info().clone(),
                InitializeMint {
                    mint: ctx.accounts.basket_mint.to_account_info().clone(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            );
            token::initialize_mint(cpi, 6, &authority, Some(&authority))?;
        }
        group.number_of_baskets += 1;
        Ok(())
    }

    pub fn update_price(ctx : Context<UpdatePrice>) -> ProgramResult 
    {

        let group = ctx.accounts.group.load()?;
        let cache = &mut ctx.accounts.cache.load_mut()?;
        let pos = group.token_description.iter().position(|x| x.price_oracle == *ctx.accounts.oracle_ai.key);
        // check if oracle is registered in token list
        assert_ne!(pos, None);
        let token_index = pos.unwrap();
        let oracle_data = ctx.accounts.oracle_ai.try_borrow_data()?;
        let oracle = pyth_client::cast::<Price>(&oracle_data);
        assert!( oracle.agg.price < 0 );
        let threshold = oracle.agg.price.checked_div(10).unwrap(); // confidence should be within 10%
        assert!(oracle.agg.conf < threshold as u64);
        cache.last_price[token_index] = oracle.agg.price as u64;
        cache.last_confidence[token_index] = oracle.agg.conf;
        cache.last_exp[token_index] = oracle.expo as u8;
        Ok(())
    }

    pub fn update_basket_price(ctx : Context<UpdateBasketPrice>) -> ProgramResult{
        let basket = &mut ctx.accounts.basket;
        let cache = ctx.accounts.cache.load()?;
        basket.update_price(&cache);
        Ok(())
    }
}