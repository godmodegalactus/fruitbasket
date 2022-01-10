use anchor_lang::prelude::*;
use std::{mem::size_of, str::Utf8Error};
use anchor_spl::token::{self, Token, SetAuthority, TokenAccount, Mint, InitializeMint};
use spl_token::instruction::{AuthorityType};
use spl_token::instruction::{initialize_account};

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
    use anchor_spl::token::accessor::authority;

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
        assert!(basket_components.len()<10);
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
    price_oracle : AccountInfo<'info>,
    product_oracle : AccountInfo<'info>,
    #[account(mut, 
              constraint = token_pool.owner == *owner.key,
              constraint = token_pool.mint == *mint.key)]
    token_pool : Account<'info, TokenAccount>,
    token_program : Program<'info, anchor_spl::token::Token>,
}

#[derive(Accounts)]
#[instruction(basket_number : u8, basket_bump : u8, basket_mint_bump : u8)]
pub struct AddBasket<'info> {
    #[account(mut, signer)]
    client : AccountInfo<'info>,

    #[account(mut)]
    group : AccountLoader<'info, FruitBasketGroup>,
    #[account(init,
               seeds = [FRUIT_BASKET, &[basket_number]],
               bump = basket_bump,
               payer = client,
               space = 8 + size_of::<Basket>())]
    basket : Box<Account<'info, Basket>>,

    #[account(init,
              seeds = [FRUIT_BASKET_MINT, &[basket_number]],
              bump = basket_mint_bump,
              payer = client,
              owner = token::ID,
              space = Mint::LEN)]
    basket_mint : AccountInfo<'info>,

    system_program : Program<'info, System>,
    token_program : Program<'info, anchor_spl::token::Token>,
    rent : Sysvar<'info, Rent>,
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