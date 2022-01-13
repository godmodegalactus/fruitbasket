use crate::*;

pub fn initialize_group(
    ctx: Context<InitializeGroup>,
    base_mint: Pubkey,
    base_mint_name: String,
) -> ProgramResult {
    // init cache
    ctx.accounts.cache.load_init()?;
    // init gr
    let mut group = ctx.accounts.fruit_basket_grp.load_init()?;
    group.owner = *ctx.accounts.owner.key;
    group.token_count = 0;
    group.base_mint = base_mint;
    let size: usize = if base_mint_name.len() > 10 {
        10
    } else {
        base_mint_name.len()
    };
    let mint_name: &[u8] = base_mint_name[..size].as_bytes();
    group.base_mint_name[..size].clone_from_slice(mint_name);
    group.number_of_baskets = 0;
    group.nb_users = 0;

    //pre allocate programming addresses
    Pubkey::find_program_address(&[FRUIT_BASKET.as_ref(), &[0]], ctx.program_id);
    Pubkey::find_program_address(&[FRUIT_BASKET_MINT.as_ref(), &[0]], ctx.program_id);
    Ok(())
}

pub fn add_token(ctx: Context<AddToken>, name: String) -> ProgramResult {
    assert!(name.len() <= 10);
    let mut group = ctx.accounts.fruit_basket_grp.load_mut()?;
    let current: usize = group.token_count as usize;
    assert!(current < MAX_NB_TOKENS);
    group.token_description[current].token_mint = *ctx.accounts.mint.key;
    group.token_description[current].price_oracle = *ctx.accounts.price_oracle.key;
    group.token_description[current].product_oracle = *ctx.accounts.product_oracle.key;
    group.token_description[current].token_name[..name.len()].clone_from_slice(name[..].as_bytes());
    let (authority, _bump) =
        Pubkey::find_program_address(&[FRUIT_BASKET_AUTHORITY], ctx.program_id);
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

    group.token_count += 1;
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


pub fn buy_basket<'info>(
    ctx: Context<'_, '_, '_, 'info, BuyBasket<'info>>,
    amount : u64,
    exp : u8,
    number_of_markets : u64,
    maximum_allowed_price : u64,
) -> ProgramResult {

    let markets: &mut Vec<MarketAccounts> = &mut Vec::new();
    for i in 0..number_of_markets {
        markets.push(
            MarketAccounts {
                base_token_mint : ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                market: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                open_orders: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                request_queue: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                event_queue: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                bids: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                asks: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                token_vault: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                quote_token_vault: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                vault_signer: ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
                token_pool : ctx.remaining_accounts.iter().next().map(Clone::clone).unwrap(),
            }
        );
    }
    let group = &ctx.accounts.group.load()?;
    let basket = &ctx.accounts.basket;
    let value_before_buying = token::accessor::amount(&ctx.accounts.paying_token_mint.to_account_info())?;
    // swap coins one by one
    for component_index in 0..basket.number_of_components {
        let component = &basket.components[component_index as usize];
        let token = &group.token_description[component.token_index as usize];
        let position = markets.iter().position( |x| *x.base_token_mint.key == token.token_mint);
        
        // check if we found the token mint in our token list
        assert_ne!(position, None);
        // check if the two tokens swaping are not the name.
        assert_ne!( token.token_mint, *ctx.accounts.paying_account.to_account_info().key);

        let market = &markets[position.unwrap()];
        assert_eq!(*market.token_pool.key, token.token_pool);
        // swap usdc to coin
        let settle_accs = dex::SettleFunds {
            market: market.market.clone(),
            open_orders: market.open_orders.clone(),
            open_orders_authority: ctx.accounts.user.clone(),
            coin_vault: market.token_vault.clone(),
            pc_vault: market.quote_token_vault.clone(),
            coin_wallet: market.token_pool.clone(),
            pc_wallet: ctx.accounts.paying_account.to_account_info().clone(),
            vault_signer: market.vault_signer.clone(),
            token_program: ctx.accounts.token_program.clone(),
        };
        let mut ctx = CpiContext::new(ctx.accounts.dex_program.clone(), settle_accs);
        dex::settle_funds(ctx)?;
    }
    let value_after_buying = token::accessor::amount(&ctx.accounts.paying_account.to_account_info())?;
    assert!(value_before_buying - value_after_buying < maximum_allowed_price);

    Ok(())
}