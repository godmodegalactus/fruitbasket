use crate::*;
use anchor_spl::dex::serum_dex::matching::{OrderType, Side};
use std::{num::NonZeroU64};
use anchor_spl::dex::serum_dex::instruction::SelfTradeBehavior;
use anchor_spl::dex::serum_dex::state::{ MarketState };
use solana_program::sysvar::clock::Clock;
use core::cell::RefMut;
use fixed::types::I80F48;

pub fn initialize_group(
    ctx: Context<InitializeGroup>,
    base_mint_name: String,
) -> ProgramResult {
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
    if name.len() > 10 {
        return Err(FruitBasketError::NameBufferOverflow.into());
    }
    let mut group = ctx.accounts.fruit_basket_grp.load_mut()?;
    let token_description = &mut ctx.accounts.token_desc;
    token_description.magic = TOKEN_DESC_MAGIC;
    token_description.id = group.token_count;

    token_description.token_mint = *ctx.accounts.mint.to_account_info().key;
    token_description.price_oracle = *ctx.accounts.price_oracle.key;
    token_description.product_oracle = *ctx.accounts.product_oracle.key;
    token_description.token_name[..name.len()].clone_from_slice(name[..].as_bytes());
    let (authority, bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);

    if authority != ctx.accounts.fruitbasket_authority.key() {
        return Err(FruitBasketError::UnknownAuthority.into());
    }
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
    token_description.token_pool = *ctx.accounts.token_pool.to_account_info().key;
    token_description.token_decimal = ctx.accounts.mint.decimals;
    
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

    token_description.token_open_orders = ctx.accounts.open_orders_account.key();
    token_description.market = ctx.accounts.market.key();
    //group.token
    Ok(())
}

pub fn add_basket(
    ctx: Context<AddBasket>,
    basket_number: u64,
    basket_name: String,
    basket_desc: String,
    basket_components: Vec<BasketComponentDescription>,
) -> ProgramResult {
    if basket_components.len() >= MAX_NB_COMPONENTS {
        return Err(FruitBasketError::ComponentCountOverflow.into());
    }
    if basket_components.len() < 2 {
        return Err(FruitBasketError::ComponentCountUnderflow.into());
    }
    let mut group = ctx.accounts.group.load_mut()?;
    if group.number_of_baskets != (basket_number as u64) {
        return Err(FruitBasketError::BasketNbMismatch.into());
    }

    let basket = &mut ctx.accounts.basket;
    basket.magic = BASKET_DESC_MAGIC;
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
    let oracle_data = ctx.accounts.oracle_ai.try_borrow_data()?;
    let oracle = pyth_client::cast::<Price>(&oracle_data);
    if oracle.agg.price <= 0 {
        return Err(FruitBasketError::PriceEqualOrLessThanZero.into());
    }
    
    let threshold : u64 = oracle.agg.price.checked_div(10).unwrap() as u64; // confidence should be within 10%
    if oracle.agg.conf > threshold {
        return Err(FruitBasketError::LowConfidenceInOracle.into());
    } 
    ctx.accounts.token_desc.cache.last_price = oracle.agg.price as u64;
    ctx.accounts.token_desc.cache.last_confidence = oracle.agg.conf;
    ctx.accounts.token_desc.cache.last_exp = oracle.expo;
    Ok(())
}

pub fn update_basket_price(ctx : Context<UpdateBasketPrice>) -> ProgramResult{
    let basket = &mut ctx.accounts.basket;
    // deserialize remaining accounts for the basket tokens
    let token_descs_deserailized = ctx.remaining_accounts.iter().map( |x| {
        let account_data = &x.try_borrow_data()?;
        let mut account_data_slice: &[u8] = &account_data;
        TokenDescription::try_deserialize_unchecked(&mut account_data_slice)

    } ).collect::<Vec<_>>();

    let res_error = token_descs_deserailized.iter().any(|x| x.is_err());
    if res_error {
        return Err(FruitBasketError::ErrorDeserializeTokeDesc.into());
    }
    let token_descs = token_descs_deserailized.iter().map(|x| x.as_ref().ok().unwrap()).collect::<Vec<_>>();
    
    let wrong_magic = token_descs.iter().any(|x| x.magic != TOKEN_DESC_MAGIC);
    if wrong_magic {
        return Err(FruitBasketError::ErrorDeserializeTokeDesc.into());
    }
    msg!("deserialization done");
    basket.update_price(&token_descs)?;
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
            basket.last_price.checked_add( basket.confidence ).unwrap()
        } else {
            basket.last_price.checked_sub( basket.confidence ).unwrap()
        };
    // we assume that the max worst case price is 10 percent of the actual price
    let mut worst_case_price = 
        if is_buy_side {
            possible_last_basket_price.checked_add(possible_last_basket_price.checked_div(10).unwrap()).unwrap()
        } else {
            possible_last_basket_price.checked_sub(possible_last_basket_price.checked_div(10).unwrap()).unwrap()
        };
    // maximum allowed price should be greater than current basket price plus the confidence of the price.
    // TODO update this check by taking into account spread in orderbook so there are far less transactions to be reverted.
    if is_buy_side {
        if max_buy_or_min_sell_price < possible_last_basket_price {
            return Err(FruitBasketError::TooLowMaximumBuyPrice.into());
        }
        // largest maximum price allowed is 10% of possible_last_basket_price
        worst_case_price = if max_buy_or_min_sell_price > worst_case_price { worst_case_price } else { max_buy_or_min_sell_price };

        if ctx.accounts.quote_token_transaction_pool.key() != group.quote_token_transaction_pool {
            return Err(FruitBasketError::AccountsMismatch.into());
        }
        

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
        // burn the tokens which user wants to sell.
        if max_buy_or_min_sell_price > possible_last_basket_price {
            return Err(FruitBasketError::TooHighMinimumSellPrice.into());
        }
        let (authority, bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);

        if authority != ctx.accounts.fruit_basket_authority.key() {
            return Err(FruitBasketError::UnknownAuthority.into());
        }
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
    // update trade context
    trade_context.magic = BASKET_TRADE_CONTEXT_MAGIC;
    trade_context.side = side;
    trade_context.basket = basket.key();
    trade_context.reverting = 0;
    trade_context.usdc_amount_left = if is_buy_side { worst_case_price } else { 0 };
    trade_context.amount = amount;
    trade_context.quote_token_account = ctx.accounts.quote_token_account.key();
    trade_context.basket_token_account = ctx.accounts.basket_token_account.key();
    trade_context.initial_usdc_transfer_amount = trade_context.usdc_amount_left;
    trade_context.tokens_treated = [0; MAX_NB_COMPONENTS];

    for component_index in 0..basket.number_of_components {
        let position = component_index as usize;
        let component : &BasketComponentDescription = &basket.components[position]; 
        trade_context.token_mints[position] = component.token_mint;
        // check if we found the token mint in our token list
        trade_context.tokens_treated[position] = 0;

        // calculate amount of tokens to transfer
        let amount_of_tokens = (amount as u128)
                                            .checked_mul(component.amount.into()).unwrap()
                                            .checked_div(10u128.pow(component.decimal.into())).unwrap();
        
        trade_context.token_amounts[position] = amount_of_tokens as u64;   
        trade_context.initial_token_amounts[position] = trade_context.token_amounts[position];
    }
    // set a timestamp on the context.
    let clock = Clock::get()?;
    trade_context.created_on = clock.unix_timestamp as u64;
    Ok(())
}

pub fn process_token_for_context(ctx : Context<ProcessTokenOnContext>) -> ProgramResult {
    let fruitbasket = &ctx.accounts.fruitbasket;
    let _component_in_basket = fruitbasket.components.iter().position(|x| x.token_mint == ctx.accounts.token_mint.key());
    // check if token is component of the basket
    if _component_in_basket == None 
    {
        return Ok(());
    }
    let token_index = _component_in_basket.unwrap();
    let token_desc = &ctx.accounts.token_desc;
    let mut trade_context = ctx.accounts.trade_context.load_mut()?;

    // check if token is already treated
    if trade_context.tokens_treated[token_index] == 1 {
        return Ok(());
    }
    let is_buy_side = ( trade_context.side == ContextSide::Buy && trade_context.reverting == 0) // check if buy while not reverting
                            || ( trade_context.side == ContextSide::Sell && trade_context.reverting == 1); // check if sell if reverting

    // checks to verify if we are in right context
    if fruitbasket.key() != trade_context.basket {
        return Err( FruitBasketError::UnknownBasket.into() );
    }

    // get authority bump and verify authority
    let (pda, bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
    if ctx.accounts.fruit_basket_authority.key() != pda {
        return Err(FruitBasketError::UnknownAuthority.into());
    }

    let seeds = &[&FRUIT_BASKET_AUTHORITY[..], &[bump]];
    // set side
    let side : Side = if is_buy_side { Side::Bid } else { Side::Ask };
    // create new order
    let quote_token_transaction_pool = &ctx.accounts.quote_token_transaction_pool.to_account_info();
    let token_pool = &ctx.accounts.token_pool.to_account_info();

    // recalculate amount by taking token decimals under consideration
    let token_amount = if token_desc.token_decimal != 6 { 
                                trade_context.token_amounts[token_index]
                                    .checked_mul(10u64.pow(token_desc.token_decimal.into())).unwrap()
                                    .checked_div(10u64.pow(6)).unwrap() 
                            } else {
                                trade_context.token_amounts[token_index]
                            };
    let (lot_size ,max_coin_qty ) = {
        let market_state = MarketState::load(&ctx.accounts.market, ctx.accounts.dex_program.key)?;
        (market_state.coin_lot_size, token_amount.checked_div(market_state.coin_lot_size).unwrap())
    };
    let max_native_token = if is_buy_side {trade_context.usdc_amount_left} else {u64::MAX};

    // get value before transaction
    let value_before_transaction = token::accessor::amount(quote_token_transaction_pool)?;
    let tokens_before_transaction = token::accessor::amount(token_pool)?;
    // Create a new order on serum
    ctx.accounts.create_new_order(side, max_coin_qty, max_native_token, &[seeds])?;
    // settle order on serum
    ctx.accounts.settle_accounts(&[seeds])?;

    let value_after_transaction = token::accessor::amount(quote_token_transaction_pool)?;
    let tokens_after_transaction = token::accessor::amount(token_pool)?;
    // check how many tokens were really transfered. If all tokens were not transfered we have to redo the process
    if is_buy_side {
        let tokens_transfered = tokens_after_transaction.checked_sub(tokens_before_transaction).unwrap();
        trade_context.token_amounts[token_index] = token_amount.checked_sub(tokens_transfered).unwrap();
        if trade_context.token_amounts[token_index] < lot_size {
            trade_context.tokens_treated[token_index] = 1;
        }
    }
    else {
        let tokens_transfered = tokens_before_transaction.checked_sub(tokens_after_transaction).unwrap();
        trade_context.token_amounts[token_index] = token_amount.checked_sub(tokens_transfered).unwrap();
        if trade_context.token_amounts[token_index] < lot_size {
            trade_context.tokens_treated[token_index] = 1;
        }
    }
    trade_context.usdc_amount_left = if is_buy_side {
        trade_context.usdc_amount_left.checked_sub(value_before_transaction.checked_sub(value_after_transaction).unwrap()).unwrap()
    } else {
        trade_context.usdc_amount_left.checked_add(value_after_transaction.checked_sub(value_before_transaction).unwrap()).unwrap()
    };
    Ok(())
}

pub fn finalize_context(ctx : Context<FinalizeContext>) -> ProgramResult {
    let trade_context = ctx.accounts.trade_context.load_mut()?;
    // check if all tokens are treated
    for i in 0..ctx.accounts.fruitbasket.number_of_components {
        if trade_context.tokens_treated[i as usize] != 1 {
            return Err(FruitBasketError::NotAllTokensTreatedBeforeFinalize.into());
        }
    }
    // some more checks
    if trade_context.basket != ctx.accounts.fruitbasket.key() {
        return Err(FruitBasketError::UnknownBasket.into());
    }
    if trade_context.quote_token_account != ctx.accounts.quote_token_account.key() ||
        trade_context.basket_token_account != ctx.accounts.basket_token_account.key() {
            return Err(FruitBasketError::AccountsMismatch.into());
        }
    let (authority, bump) = Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
    if authority != ctx.accounts.fruit_basket_authority.key() {
        return Err(FruitBasketError::UnknownAuthority.into());
    }
    let seeds = [&FRUIT_BASKET_AUTHORITY[..], &[bump]];
    let signer = &[&seeds[..]];

    if trade_context.reverting == 1 {
        return finalize_for_revert_context(&ctx, trade_context, signer);
    }

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

pub fn finalize_for_revert_context<'info>(ctx : &Context<FinalizeContext>,
                                            trade_context : RefMut<BasketTradeContext>,
                                            signer : &[&[&[u8]]]) -> ProgramResult {
    if trade_context.side == ContextSide::Buy {
        // user was trying to buy the context and the transaction was reverted mostly due to failure.
        // So we give back user the original amount
        let accounts = token::Transfer {
            from: ctx.accounts.quote_token_transaction_pool.to_account_info().clone(),
            to: ctx.accounts.quote_token_account.to_account_info().clone(),
            authority:  ctx.accounts.fruit_basket_authority.clone(),
        };
        let transfer_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.clone(), accounts, signer);
        token::transfer( transfer_ctx, trade_context.initial_usdc_transfer_amount)?;
    }
    else {
        // user tried to sell the tokens but the transaction failed.
        // So we will mint token back to the user
        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.basket_token_mint.to_account_info(),
            to: ctx.accounts.basket_token_account.to_account_info(),
            authority: ctx.accounts.fruit_basket_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, trade_context.amount)?;
    }
    
    Ok(())
}

pub fn revert_trade_context( ctx: Context<RevertTradeContext> ) -> ProgramResult {
    let mut trade_context = ctx.accounts.trade_context.load_mut()?;
    // check if trade is already reverting
    if trade_context.reverting == 1 {
        return Ok(())
    }
    let basket = &ctx.accounts.fruitbasket;
    if basket.key() != trade_context.basket {
        return Err( FruitBasketError::UnknownBasket.into() );
    }

    trade_context.reverting = 1;
    for index in 0..basket.number_of_components {
        let token_index = index as usize;
        if trade_context.tokens_treated[token_index] == 1 {
            trade_context.tokens_treated[token_index] = 0;
            trade_context.token_amounts[token_index] = trade_context.initial_token_amounts[token_index];
        } else {
            // token has not been processed yet.
            if trade_context.token_amounts[token_index] == trade_context.initial_token_amounts[token_index] {
                trade_context.tokens_treated[token_index] = 1;
            }
            else {
                // update the token count that should be processed
                trade_context.token_amounts[token_index] = trade_context.initial_token_amounts[token_index].checked_sub(trade_context.token_amounts[token_index]).unwrap();
            }
        }
    }
    // update usdc amount left
    // TODO smarter way to decide these token amounts
    if trade_context.side == ContextSide::Buy {
        trade_context.usdc_amount_left = 0;
    } else  {
        // TODO ASAP smarter way to calculate the usdc limit to buy back the tokens in case of revert.
        // Multiple strategies available.
        trade_context.usdc_amount_left = token::accessor::amount(&ctx.accounts.quote_token_transaction_pool)?;
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

impl<'info> ProcessTokenOnContext<'info>{
    fn create_new_order(&self, 
                        side:Side, 
                        max_coin_qty: u64, 
                        max_native_token : u64,
                        seeds:&[&[&[u8]]]) -> ProgramResult {
        let is_buy_side = side == Side::Bid;
        let client_order_id = 0;
        let limit = 65535;
        let new_orders = dex::NewOrderV3 {
            market: self.market.clone(),
            open_orders: self.open_orders.clone(),
            request_queue: self.request_queue.clone(),
            event_queue: self.event_queue.clone(),
            market_bids: self.bids.clone(),
            market_asks: self.asks.clone(),
            order_payer_token_account: if is_buy_side { self.quote_token_transaction_pool.to_account_info().clone() } else { self.token_pool.clone() },
            open_orders_authority: self.fruit_basket_authority.clone(),
            coin_vault: self.token_vault.clone(),
            pc_vault: self.quote_token_vault.clone(),
            token_program: self.token_program.clone(),
            rent: self.rent.clone(),
        };
        let ctx_orders = CpiContext::new(self.dex_program.clone(), new_orders);
        // TODO Decide limit price and native token price more approriately.
        let limit_price = if is_buy_side { u64::MAX } else { 1 };
        dex::new_order_v3(
            ctx_orders.with_signer(seeds),
            side,
            NonZeroU64::new(limit_price).unwrap(),
            NonZeroU64::new(max_coin_qty).unwrap(),
            NonZeroU64::new(max_native_token).unwrap(),
            SelfTradeBehavior::DecrementTake,
            OrderType::ImmediateOrCancel,
            client_order_id,
            limit,
        )
    }

    fn settle_accounts(&self,
                        seeds : &[&[&[u8]]]) -> ProgramResult {
        let settle_accs = dex::SettleFunds {
            market: self.market.clone(),
            open_orders: self.open_orders.clone(),
            open_orders_authority: self.fruit_basket_authority.clone(),
            coin_vault: self.token_vault.clone(),
            pc_vault: self.quote_token_vault.clone(),
            coin_wallet: self.token_pool.clone(),
            pc_wallet: self.quote_token_transaction_pool.to_account_info().clone(),
            vault_signer: self.vault_signer.clone(),
            token_program: self.token_program.clone(),
        };
        let settle_ctx = CpiContext::new(self.dex_program.clone(), settle_accs);
        dex::settle_funds(settle_ctx.with_signer(seeds))
    }
}


impl Basket {
    pub fn update_price(&mut self, token_descs : &Vec<&TokenDescription>) -> ProgramResult {
        let mut price  = I80F48::from_num(0);
        let mut confidence  = I80F48::from_num(0);
        let decimal : u8 = 6;
        
        for i in 0..self.number_of_components {
            let comp = self.components[i as usize];
            let position = token_descs.iter().position(|x| x.token_mint == comp.token_mint);
            if position == None {
                return Err(FruitBasketError::TokenNotFound.into());
            }
            let token_index = position.unwrap();
            let cache = token_descs[token_index].cache;
            
            let mut comp_price = cache.last_price.checked_mul(comp.amount).unwrap().checked_div(10u64.pow(comp.decimal as u32)).unwrap();
            let mut comp_conf = cache.last_confidence.checked_mul(comp.amount).unwrap().checked_div(10u64.pow(comp.decimal as u32)).unwrap();

            //pyth decimal is negative usual decimal
            let pyth_decimal = if cache.last_exp >= 0 { 0 } else {-cache.last_exp as u8};
            
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
        if self.last_price <= 0 {
            return Err(FruitBasketError::PriceEqualOrLessThanZero.into());
        }
        Ok(())
    }
}
