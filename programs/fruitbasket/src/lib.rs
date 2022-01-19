use anchor_lang::prelude::*;
use std::{mem::size_of};
use anchor_spl::token::{self, SetAuthority, TokenAccount, Mint, InitializeMint};
use spl_token::instruction::{AuthorityType};
use pyth_client::Price;
use anchor_spl::dex;

mod instructions;
use instructions::*;
mod states;
use states::*;
mod processor;


declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
const MAX_NB_TOKENS : usize = 20;
const MAX_NB_COMPONENTS: usize = 10;
const FRUIT_BASKET_GROUP : &[u8] = b"fruitbasket_group";
const FRUIT_BASKET_CACHE : &[u8] = b"fruitbasket_cache";
const FRUIT_BASKET_AUTHORITY : &[u8] = b"fruitbasket_auth";
const FRUIT_BASKET : &[u8] = b"fruitbasket";
const FRUIT_BASKET_MINT : &[u8] = b"fruitbasket_mint";
const FRUIT_BASKET_CONTEXT : &[u8] = b"fruitbasket_context";

mod empty {
    use super::*;
    declare_id!("HJt8Tjdsc9ms9i4WCZEzhzr4oyf3ANcdzXrNdLPFqm3M");
}

#[program]
pub mod fruitbasket {
    use super::*;

    pub fn initialize_group(ctx: Context<InitializeGroup>, 
            _bump_group: u8, 
            _bump_cache: u8,
            base_mint_name: String) -> ProgramResult {
        processor::initialize_group(ctx, base_mint_name)
    }

    pub fn add_token(ctx: Context<AddToken>, name: String) -> ProgramResult {
        processor::add_token(ctx, name)
    }

    // add basket
    pub fn add_basket(ctx : Context<AddBasket>, 
        basket_number : u8, 
        _basket_bump : u8, 
        _basket_mint_bump : u8,
        basket_name : String, 
        basket_desc : String,
        basket_components : Vec<BasketComponentDescription>) -> ProgramResult {
        processor::add_basket(ctx, basket_number, basket_name, basket_desc, basket_components)
    }

    pub fn update_price(ctx : Context<UpdatePrice>) -> ProgramResult {
        processor::update_price(ctx)
    }

    pub fn update_basket_price(ctx : Context<UpdateBasketPrice>) -> ProgramResult{
        processor::update_basket_price(ctx)
    }

    pub fn init_buy_basket(
        ctx: Context<InitBuyBasket>,
        _order_id: u8, 
        _context_bump : u8, 
        amount : u64,
        maximum_allowed_price : u64,
    ) -> ProgramResult {
        processor::init_buy_basket(ctx, amount, maximum_allowed_price)
    }

    pub fn process_token_for_context(ctx : Context<ProcessTokenOnContext>) -> ProgramResult {
        processor::process_token_for_context(ctx)
    }
}
