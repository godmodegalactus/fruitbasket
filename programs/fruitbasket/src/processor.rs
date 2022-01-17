use fixed::traits::Fixed;

use crate::*;
use anchor_spl::dex::serum_dex::matching::{OrderType, Side};
use std::{num::NonZeroU64};
use anchor_spl::dex::serum_dex::instruction::SelfTradeBehavior;

pub fn initialize_group(
    ctx: Context<InitializeGroup>,
    base_mint_name: String,
) -> ProgramResult {
    // init cache
    ctx.accounts.cache.load_init()?;
    // init gr
    let mut group = ctx.accounts.fruit_basket_grp.load_init()?;
    group.owner = *ctx.accounts.owner.key;
    group.token_count = 0;
    group.base_mint = ctx.accounts.quote_token_mint.key();
    let size: usize = if base_mint_name.len() > 10 {
        10
    } else {
        base_mint_name.len()
    };
    let mint_name: &[u8] = base_mint_name[..size].as_bytes();
    group.base_mint_name[..size].clone_from_slice(mint_name);
    group.number_of_baskets = 0;
    group.nb_users = 0;
    group.quote_token_transaction_pool = ctx.accounts.quote_token_transaction_pool.key();

    //pre allocate programming addresses
    Pubkey::find_program_address(&[FRUIT_BASKET.as_ref(), &[0]], ctx.program_id);
    Pubkey::find_program_address(&[FRUIT_BASKET_MINT.as_ref(), &[0]], ctx.program_id);

    let (authority, _bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);

    // change authority of the pool fruitbasket authority
    change_authority(&ctx.accounts.quote_token_transaction_pool.to_account_info(), 
                    &ctx.accounts.owner, 
                    authority, &ctx.accounts.token_program, 
                    None)?;
    Ok(())
}

pub fn add_token(ctx: Context<AddToken>, name: String) -> ProgramResult {
    assert!(name.len() <= 10);
    let mut group = ctx.accounts.fruit_basket_grp.load_mut()?;
    let current: usize = group.token_count as usize;
    assert!(current < MAX_NB_TOKENS);
    group.token_description[current].token_mint = *ctx.accounts.mint.to_account_info().key;
    group.token_description[current].price_oracle = *ctx.accounts.price_oracle.key;
    group.token_description[current].product_oracle = *ctx.accounts.product_oracle.key;
    group.token_description[current].token_name[..name.len()].clone_from_slice(name[..].as_bytes());
    let (authority, bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);

    assert_eq!(authority, ctx.accounts.fruitbasket_authority.key());
    {
        // change authority of token pool to authority
        let cpi_accounts = SetAuthority {
            account_or_mint: ctx.accounts.token_pool.to_account_info().clone(),
            current_authority: ctx.accounts.owner.clone(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi = CpiContext::new(cpi_program, cpi_accounts);
        token::set_authority(cpi, AuthorityType::AccountOwner, Some(authority))?;
    }
    group.token_description[current].token_pool = *ctx.accounts.token_pool.to_account_info().key;
    group.token_description[current].token_decimal = ctx.accounts.mint.decimals;
    
    group.token_count += 1;
    if ctx.accounts.market.key() == empty::ID {
        return Ok(());
    }
    let seeds = &[&FRUIT_BASKET_AUTHORITY[..], &[bump]];
    //create and assign open order
    let open_order_instruction = dex::InitOpenOrders {
        open_orders: ctx.accounts.open_orders_account.to_account_info().clone(),
        authority: ctx.accounts.fruitbasket_authority.clone(),
        market: ctx.accounts.market.clone(),
        rent: ctx.accounts.rent.clone(),
    };
    let oo_ctx = CpiContext::new(ctx.accounts.dex_program.clone(), open_order_instruction);
    dex::init_open_orders(oo_ctx.with_signer(&[seeds]))?;

    group.token_description[current].token_open_orders = ctx.accounts.open_orders_account.key();
    //group.token
    Ok(())
}

pub fn add_basket(
    ctx: Context<AddBasket>,
    basket_number: u8,
    basket_name: String,
    basket_desc: String,
    basket_components: Vec<BasketComponentDescription>,
) -> ProgramResult {
    assert!(basket_components.len() < MAX_NB_COMPONENTS);
    assert!(basket_components.len() > 1);
    let mut group = ctx.accounts.group.load_mut()?;
    assert!(group.number_of_baskets == basket_number);

    let basket = &mut ctx.accounts.basket;
    basket.basket_name[..basket_name.len()].copy_from_slice(basket_name[..].as_bytes());
    basket.desc[..basket_desc.len()].copy_from_slice(basket_desc[..].as_bytes());
    basket.number_of_components = basket_components.len() as u8;
    basket.basket_mint = *ctx.accounts.basket_mint.to_account_info().key;

    for i in 0..basket_components.len() {
        let component = basket_components[i];
        basket.components[i] = component;
    }

    let (authority, _bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
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

pub fn update_price(ctx: Context<UpdatePrice>) -> ProgramResult {
    let group = ctx.accounts.group.load()?;
    let cache = &mut ctx.accounts.cache.load_mut()?;

    let pos = group
        .token_description
        .iter()
        .position(|x| x.price_oracle == *ctx.accounts.oracle_ai.key);
    // check if oracle is registered in token list
    assert_ne!(pos, None);
    let token_index = pos.unwrap();
    let oracle_data = ctx.accounts.oracle_ai.try_borrow_data()?;
    let oracle = pyth_client::cast::<Price>(&oracle_data);
    assert!(oracle.agg.price > 0);
    let threshold = oracle.agg.price.checked_div(10).unwrap(); // confidence should be within 10%
    assert!(oracle.agg.conf < threshold as u64);
    cache.last_price[token_index] = oracle.agg.price as u64;
    cache.last_confidence[token_index] = oracle.agg.conf;
    cache.last_exp[token_index] = oracle.expo;
    Ok(())
}

pub fn update_basket_price(ctx : Context<UpdateBasketPrice>) -> ProgramResult{
    let basket = &mut ctx.accounts.basket;
    let cache = ctx.accounts.cache.load()?;
    basket.update_price(&cache)?;
    Ok(())
}


pub fn init_buy_basket<'info>(
    ctx: Context<'_, '_, '_, 'info, InitBuyBasket<'info>>,
    amount : u64,
    maximum_allowed_price : u64,
) -> ProgramResult {
    
    let group = ctx.accounts.group.load()?;
    let basket = &ctx.accounts.basket;
    let mut buy_context = ctx.accounts.buy_context.load_init()?;

    // price after taking into account the confidence
    let possible_last_basket_price : u64 = basket.last_price + basket.confidence;
    // worst case price = maximum price + 1%
    let worst_case_price =  possible_last_basket_price + possible_last_basket_price.checked_div(100).unwrap();
    // check if maximum allowed price is 1 percent higher than the current price
    assert!(maximum_allowed_price > worst_case_price);

    assert!(ctx.accounts.quote_token_transaction_pool.key() == group.quote_token_transaction_pool);

    // transfer usdc from client to account
    let accounts = token::Transfer {
        from: ctx.accounts.paying_account.to_account_info().clone(),
        to: ctx.accounts.quote_token_transaction_pool.to_account_info().clone(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let transfer_ctx = CpiContext::new(ctx.accounts.token_program.clone(), accounts);
    token::transfer( transfer_ctx, worst_case_price)?;

    buy_context.side = ContextSide::Buy;
    buy_context.basket = basket.key();
    buy_context.reverting = 0;
    buy_context.usdc_amount_left = worst_case_price;
    buy_context.paying_account = ctx.accounts.paying_account.key();
    buy_context.user_basket_token_account = ctx.accounts.user_basket_token_account.key();
    buy_context.initial_usdc_transfer_amount = worst_case_price;
    buy_context.tokens_treated = [1; MAX_NB_TOKENS];

    for component_index in 0..basket.number_of_components {
        let component : &BasketComponentDescription = &basket.components[component_index as usize];
        let token = &group.token_description[component.token_index as usize];
        let position = component.token_index as usize;

        // check if we found the token mint in our token list
        buy_context.tokens_treated[position] = 0;

        // calculate amount of tokens to transfer
        let amount_of_tokens = (amount as u128)
                                            .checked_mul(component.amount.into()).unwrap()
                                            .checked_div(10u128.pow(6)).unwrap()
                                            .checked_mul(10u128.pow(token.token_decimal.into())).unwrap()
                                            .checked_div(10u128.pow(component.decimal.into())).unwrap();
        
        buy_context.token_amounts[position] = amount_of_tokens as u64;   
    }
    Ok(())
}

/*
fn order<'info>( ctx: &Context<'_, '_, '_, 'info, BuyBasket<'info>>,
    market : &MarketAccounts<'info>,
    limit_price: u64,
    max_coin_qty: u64,
    max_native_pc_qty: u64,
    side: Side,
    signer_seeds : &[&[&[u8]]]
) -> ProgramResult {
    // Client order id is only used for cancels. Not used here so hardcode.
    let client_order_id = 0;
    let limit = 65535;

    let new_orders = dex::NewOrderV3 {
        market: market.market.clone(),
        open_orders: market.open_orders.clone(),
        request_queue: market.request_queue.clone(),
        event_queue: market.event_queue.clone(),
        market_bids: market.bids.clone(),
        market_asks: market.asks.clone(),
        order_payer_token_account: market.token_pool.clone(),
        open_orders_authority: ctx.accounts.authority.clone(),
        coin_vault: market.token_vault.clone(),
        pc_vault: market.quote_token_vault.clone(),
        token_program: ctx.accounts.token_program.clone(),
        rent: ctx.accounts.rent.clone(),
    };

    let ctx_orders = CpiContext::new(ctx.accounts.dex_program.clone(), new_orders).with_signer(signer_seeds);

    dex::new_order_v3(
        ctx_orders,
        side,
        NonZeroU64::new(limit_price).unwrap(),
        NonZeroU64::new(max_coin_qty).unwrap(),
        NonZeroU64::new(max_native_pc_qty).unwrap(),
        SelfTradeBehavior::DecrementTake,
        OrderType::ImmediateOrCancel,
        client_order_id,
        limit,
    )
}

fn settle_funds<'info>( ctx: &Context<'_, '_, '_, 'info, BuyBasket<'info>>,
                        market : &MarketAccounts<'info> ) -> ProgramResult {
    let settle_accs = dex::SettleFunds {
        market: market.market.clone(),
        open_orders: market.open_orders.clone(),
        open_orders_authority: ctx.accounts.authority.clone(),
        coin_vault: market.token_vault.clone(),
        pc_vault: market.quote_token_vault.clone(),
        coin_wallet: market.token_pool.clone(),
        pc_wallet: ctx.accounts.paying_account.to_account_info().clone(),
        vault_signer: market.vault_signer.clone(),
        token_program: ctx.accounts.token_program.clone(),
    };
    let settle_ctx = CpiContext::new(ctx.accounts.dex_program.clone(), settle_accs);
    dex::settle_funds(settle_ctx)
}
*/

fn change_authority<'info>(acc : &AccountInfo<'info>, 
                          from : &AccountInfo<'info>, 
                          to: Pubkey, 
                          token_program: &AccountInfo<'info>, 
                          seeds : Option<&[&[&[u8]]]>) -> ProgramResult{

    let cpi_acc = SetAuthority {
        account_or_mint: acc.clone(),
        current_authority: from.clone(),
    };
    let mut cpi = CpiContext::new(token_program.clone(), cpi_acc);
    if let Some(signer_seeds) = seeds {
        cpi = cpi.with_signer(signer_seeds);
    }
    token::set_authority( cpi,  AuthorityType::AccountOwner, Some(to))
}

impl<'info> MarketAccounts<'info> {
    fn print(&self){
        {
            let msg = format!(" market base_token_mint : {}", self.base_token_mint.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market market : {}", self.market.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market open_orders : {}", self.open_orders.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market request_queue : {}", self.request_queue.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market event_queue : {}", self.event_queue.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market bids : {}", self.bids.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market asks : {}", self.asks.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market token_vault : {}", self.token_vault.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market quote_token_vault : {}", self.quote_token_vault.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market vault_signer : {}", self.vault_signer.key.to_string());
            msg!(&msg[..]);
        }

        {
            let msg = format!(" market token_pool : {}", self.token_pool.key.to_string());
            msg!(&msg[..]);
        }
    }
}