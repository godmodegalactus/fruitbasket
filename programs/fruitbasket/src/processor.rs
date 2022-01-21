use crate::*;
use anchor_spl::dex::serum_dex::matching::{OrderType, Side};
use std::ops::Div;
use std::{num::NonZeroU64};
use anchor_spl::dex::serum_dex::instruction::SelfTradeBehavior;
use anchor_spl::dex::serum_dex::state::{ MarketState };
use solana_program::sysvar::clock::Clock;

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
    group.token_description[current].market = ctx.accounts.market.key();
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


pub fn init_trade_context(
    ctx: Context<InitTradeContext>,
    side: ContextSide,
    amount : u64,
    max_buy_or_min_sell_price : u64,
) -> ProgramResult {
    
    let group = ctx.accounts.group.load()?;
    let basket = &ctx.accounts.basket;
    let mut trade_context = ctx.accounts.trade_context.load_init()?;
    let is_buy_side = side == ContextSide::Buy;

    // price after taking into account the confidence
    let possible_last_basket_price : u64 = 
        if is_buy_side { 
            basket.last_price + basket.confidence 
        } else {
            basket.last_price - basket.confidence
        };
    let mut worst_case_price = 
        if is_buy_side {
            possible_last_basket_price + possible_last_basket_price.div(10)
        } else {
            possible_last_basket_price - possible_last_basket_price.div(10)
        };
    // maximum allowed price should be greater than current basket price plus the confidence of the price.
    if is_buy_side {
        assert!(max_buy_or_min_sell_price > possible_last_basket_price);
        // largest maximum price allowed is 10% of possible_last_basket_price
        worst_case_price = if max_buy_or_min_sell_price > worst_case_price { worst_case_price } else { max_buy_or_min_sell_price };

        assert!(ctx.accounts.quote_token_transaction_pool.key() == group.quote_token_transaction_pool);

        // transfer usdc from client to pool account
        let accounts = token::Transfer {
            from: ctx.accounts.quote_token_account.to_account_info().clone(),
            to: ctx.accounts.quote_token_transaction_pool.to_account_info().clone(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let transfer_ctx = CpiContext::new(ctx.accounts.token_program.clone(), accounts);
        token::transfer( transfer_ctx, worst_case_price)?;
    }
    else {
        assert!(max_buy_or_min_sell_price < possible_last_basket_price);
        let (authority, bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
        assert_eq!(authority, ctx.accounts.fruit_basket_authority.key());
        let seeds = &[&FRUIT_BASKET_AUTHORITY[..], &[bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = token::Burn {
            mint: ctx.accounts.basket_token_mint.to_account_info(),
            to: ctx.accounts.basket_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::burn(cpi_ctx, amount)?;
    }
    
    trade_context.side = side;
    trade_context.basket = basket.key();
    trade_context.reverting = 0;
    trade_context.usdc_amount_left = if is_buy_side { worst_case_price } else { 0 };
    trade_context.amount = amount;
    trade_context.quote_token_account = ctx.accounts.quote_token_account.key();
    trade_context.basket_token_account = ctx.accounts.basket_token_account.key();
    trade_context.initial_usdc_transfer_amount = trade_context.usdc_amount_left;
    trade_context.tokens_treated = [1; MAX_NB_TOKENS];

    for component_index in 0..basket.number_of_components {
        let component : &BasketComponentDescription = &basket.components[component_index as usize];
        let token = &group.token_description[component.token_index as usize];
        let position = component.token_index as usize;

        // check if we found the token mint in our token list
        trade_context.tokens_treated[position] = 0;

        // calculate amount of tokens to transfer
        let amount_of_tokens = (amount as u128)
                                            .checked_mul(component.amount.into()).unwrap()
                                            .checked_div(10u128.pow(6)).unwrap()
                                            .checked_mul(10u128.pow(token.token_decimal.into())).unwrap()
                                            .checked_div(10u128.pow(component.decimal.into())).unwrap();
        
        trade_context.token_amounts[position] = amount_of_tokens as u64;   
    }
    // set a timestamp on the context.
    let clock = Clock::get()?;
    trade_context.created_on = clock.unix_timestamp as u64;
    Ok(())
}

pub fn process_token_for_context(ctx : Context<ProcessTokenOnContext>) -> ProgramResult {
    let client_order_id = 0;
    let limit = 65535;
    let basket_group = ctx.accounts.fruitbasket_group.load()?;
    let _token_index = basket_group.token_description.iter().position(|x| x.token_mint == ctx.accounts.token_mint.key());
    assert_ne!(_token_index, None);
    let token_index = _token_index.unwrap();
    let fruitbasket = &ctx.accounts.fruitbasket;
    let _component_in_basket = fruitbasket.components.iter().position(|x| (x.token_index as usize) == token_index);
    if _component_in_basket == None 
    {
        return Ok(());
    }
    let mut trade_context = ctx.accounts.trade_context.load_mut()?;
    let is_buy_side = trade_context.side == ContextSide::Buy;
    // checks to verify if we are in right context
    assert_eq!(fruitbasket.key(), trade_context.basket); // same basketS
    assert_eq!(trade_context.tokens_treated[token_index], 0); // check if the token is not already treated

    let (pda, bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
    assert_eq!(ctx.accounts.fruit_basket_authority.key(), pda);
    let seeds = &[&FRUIT_BASKET_AUTHORITY[..], &[bump]];
    let side : Side = if is_buy_side { Side::Bid } else { Side::Ask };
    let market = &ctx.accounts.market;
    let dex_program = &ctx.accounts.dex_program;
    // create new order
    let quote_token_transaction_pool = &ctx.accounts.quote_token_transaction_pool.to_account_info();
    let token_pool = &ctx.accounts.token_pool.to_account_info();
    let value_before_transaction = token::accessor::amount(quote_token_transaction_pool)?;
    let tokens_before_transaction = token::accessor::amount(token_pool)?;
    {
        let max_coin_qty = {
            let market_state = MarketState::load(market, dex_program.key)?;
            trade_context.token_amounts[token_index].checked_div(market_state.coin_lot_size).unwrap()
        };
        let new_orders = dex::NewOrderV3 {
            market: market.clone(),
            open_orders: ctx.accounts.open_orders.clone(),
            request_queue: ctx.accounts.request_queue.clone(),
            event_queue: ctx.accounts.event_queue.clone(),
            market_bids: ctx.accounts.bids.clone(),
            market_asks: ctx.accounts.asks.clone(),
            order_payer_token_account: if is_buy_side { quote_token_transaction_pool.clone() } else { token_pool.clone() },
            open_orders_authority: ctx.accounts.fruit_basket_authority.clone(),
            coin_vault: ctx.accounts.token_vault.clone(),
            pc_vault: ctx.accounts.quote_token_vault.clone(),
            token_program: ctx.accounts.token_program.clone(),
            rent: ctx.accounts.rent.clone(),
        };
        let ctx_orders = CpiContext::new(dex_program.clone(), new_orders);
        let limit_price = if is_buy_side { u64::MAX } else { 1 };
        let max_native_token = if is_buy_side { trade_context.usdc_amount_left } else { u64::MAX };
        dex::new_order_v3(
            ctx_orders.with_signer(&[seeds]),
            side,
            NonZeroU64::new(limit_price).unwrap(),
            NonZeroU64::new(max_coin_qty).unwrap(),
            NonZeroU64::new(max_native_token).unwrap(),
            SelfTradeBehavior::DecrementTake,
            OrderType::ImmediateOrCancel,
            client_order_id,
            limit,
        )?;
    }
    let tokens_during_transaction = token::accessor::amount(token_pool)?;
    {
        let msg = format!("tokens during transaction : {}", tokens_during_transaction);
        msg!(&msg[..]);
    }
    // settle transaction
    {
        let settle_accs = dex::SettleFunds {
            market: ctx.accounts.market.clone(),
            open_orders: ctx.accounts.open_orders.clone(),
            open_orders_authority: ctx.accounts.fruit_basket_authority.clone(),
            coin_vault: ctx.accounts.token_vault.clone(),
            pc_vault: ctx.accounts.quote_token_vault.clone(),
            coin_wallet: token_pool.clone(),
            pc_wallet: quote_token_transaction_pool.clone(),
            vault_signer: ctx.accounts.vault_signer.clone(),
            token_program: ctx.accounts.token_program.clone(),
        };
        let settle_ctx = CpiContext::new(ctx.accounts.dex_program.clone(), settle_accs);
        dex::settle_funds(settle_ctx.with_signer(&[seeds]))?;
    }
    let value_after_transaction = token::accessor::amount(quote_token_transaction_pool)?;

    let tokens_after_transaction = token::accessor::amount(token_pool)?;
    {
        let msg = format!("tokens after transaction : {}", tokens_after_transaction);
        msg!(&msg[..]);
    }
    {
        let msg = format!("usdc transfered : {}", value_after_transaction - value_before_transaction);
        msg!(&msg[..]);
    }
    // check if trade has been really done
    if is_buy_side {
        assert_eq!( tokens_after_transaction - tokens_before_transaction,  trade_context.token_amounts[token_index]);
    } else {
        assert_eq!( tokens_before_transaction - tokens_after_transaction,  trade_context.token_amounts[token_index]);
    }

    trade_context.tokens_treated[token_index] = 1;
    
    trade_context.usdc_amount_left = if is_buy_side {
        trade_context.usdc_amount_left - (value_before_transaction - value_after_transaction)
    } else {
        trade_context.usdc_amount_left + (value_after_transaction - value_before_transaction)
    };
    Ok(())
}

pub fn finalize_context(ctx : Context<FinalizeContext>) -> ProgramResult {
    let basket_group = ctx.accounts.fruitbasket_group.load()?;
    let trade_context = ctx.accounts.trade_context.load_mut()?;
    // check if all tokens are treated
    for i in 0..basket_group.token_count {
        assert_eq!(trade_context.tokens_treated[i as usize], 1);
    }
    // some more checks
    assert!( trade_context.basket == ctx.accounts.fruitbasket.key() );
    assert!( trade_context.quote_token_account == ctx.accounts.quote_token_account.key() );
    assert!( trade_context.basket_token_account == ctx.accounts.basket_token_account.key() );
    let (authority, bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
    assert_eq!(authority, ctx.accounts.fruit_basket_authority.key());
    let seeds = [&FRUIT_BASKET_AUTHORITY[..], &[bump]];
    let signer = &[&seeds[..]];

    if trade_context.side == ContextSide::Buy {
        // buy side
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.basket_token_mint.to_account_info(),
            to: ctx.accounts.basket_token_account.to_account_info(),
            authority: ctx.accounts.fruit_basket_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, trade_context.amount)?;
    }
    // transfer remaining usdc back to client for buy context
    // transfer result usdc back to client for sell context
    if trade_context.usdc_amount_left > 0 { 
        let accounts = token::Transfer {
            from: ctx.accounts.quote_token_transaction_pool.to_account_info().clone(),
            to: ctx.accounts.quote_token_account.to_account_info().clone(),
            authority:  ctx.accounts.fruit_basket_authority.clone(),
        };
        let transfer_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.clone(), accounts, signer);
        token::transfer( transfer_ctx, trade_context.usdc_amount_left)?;
    }
    Ok(())
}

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