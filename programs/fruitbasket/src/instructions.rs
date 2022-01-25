use crate::*;
/// Initialize Group
/// To initialize a group i.e inital data for the market
/// This should be done only by owner of the market
#[derive(Accounts)]
#[instruction(bump_group: u8, bump_cache: u8)]
pub struct InitializeGroup<'info> {
    #[account(mut, signer)]
    pub owner : AccountInfo<'info>,

    #[account( init,
        seeds = [FRUIT_BASKET_GROUP, &owner.key.to_bytes()],
        bump = bump_group, 
        payer = owner, 
        space = 8 + size_of::<FruitBasketGroup>() )]
    pub fruit_basket_grp : AccountLoader<'info, FruitBasketGroup>,

    #[account( init,
        seeds = [FRUIT_BASKET_CACHE, &owner.key.to_bytes()],
        bump = bump_cache, 
        payer = owner, 
        space = 8 + size_of::<Cache>() )]
    pub cache : AccountLoader<'info, Cache>,

    pub quote_token_mint : Box<Account<'info, Mint>>,
    
    #[account(mut,
              constraint = quote_token_transaction_pool.mint == quote_token_mint.key())]
    pub quote_token_transaction_pool : Box<Account<'info, TokenAccount>>,

    pub system_program : Program<'info, System>,
    pub token_program : AccountInfo<'info>,
}

/// Add Token ->  to add new token to the market.
/// To add a token we need to know the market and pyth price and product keys
#[derive(Accounts)]
pub struct AddToken<'info>{
    #[account(signer)]
    pub owner : AccountInfo<'info>,

    #[account(mut)]
    pub fruit_basket_grp : AccountLoader<'info, FruitBasketGroup>,

    pub mint : Account<'info, Mint>,
    pub price_oracle : AccountInfo<'info>,
    pub product_oracle : AccountInfo<'info>,
    #[account(mut, 
              constraint = token_pool.owner == *owner.key,
              constraint = token_pool.mint == *mint.to_account_info().key)]
    pub token_pool : Account<'info, TokenAccount>,

    pub market : AccountInfo<'info>,
    #[account(mut)]
    pub open_orders_account : AccountInfo<'info>,
    pub fruitbasket_authority : AccountInfo<'info>,
    pub token_program : AccountInfo<'info>,
    pub dex_program : AccountInfo<'info>,
    pub rent : AccountInfo<'info>,
}

/// Add basket -> To create a new basket.
/// Need to pass all tokens and amounts by instruction
/// This will create a basket key and a basket mint key
/// Basket mint are special mint for each basket that will be minted when you buy a basket
#[derive(Accounts)]
#[instruction(basket_number : u8, basket_bump : u8, basket_mint_bump : u8)]
pub struct AddBasket<'info> {
    #[account(mut, signer)]
    pub client : AccountInfo<'info>,

    #[account(mut)]
    pub group : AccountLoader<'info, FruitBasketGroup>,
    #[account(init,
               seeds = [FRUIT_BASKET, &[basket_number]],
               bump = basket_bump,
               payer = client,
               space = 8 + size_of::<Basket>())]
    pub basket : Box<Account<'info, Basket>>,

    #[account(init,
              seeds = [FRUIT_BASKET_MINT, &[basket_number]],
              bump = basket_mint_bump,
              payer = client,
              owner = token::ID,
              space = Mint::LEN)]
    pub basket_mint : AccountInfo<'info>,

    pub system_program : Program<'info, System>,
    pub token_program : Program<'info, anchor_spl::token::Token>,
    pub rent : Sysvar<'info, Rent>,
}

// Permissionless instruction which should be called to update price in cache
#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    pub group : AccountLoader<'info, FruitBasketGroup>,
    #[account(mut)]
    pub cache : AccountLoader<'info, Cache>,
    pub oracle_ai : AccountInfo<'info>,
}

// permissionless instruction which should be called to update the basket price from the cache
#[derive(Accounts)]
pub struct UpdateBasketPrice<'info> {
    #[account(mut)]
    pub basket : Box<Account<'info, Basket>>,

    pub cache : AccountLoader<'info, Cache>,
}

/// Creates a context for a basket trade {buying, selling}
/// To trade a basket we have to first create a trade context using this instruction. 
/// Then use the address of context and process it for each token in the basket.
/// After processing every token you have to use FinalizeContext to finish the trade.
/// USDC/BasketTokens will be taken during the init phase and swap will be done during finalize phase.
/// Only init context should require a signer.
/// We have to adopt this strategy as we cannot pass a lot of accounts during single call (i.e accounts related to market of all available tokens)
#[derive(Accounts)]
#[instruction( order_id: u8, context_bump : u8,)]
pub struct InitTradeContext<'info> {
    pub group : AccountLoader<'info, FruitBasketGroup>,

    #[account(signer, mut)]
    pub user : AccountInfo<'info>,

    pub basket : Box<Account<'info, Basket>>,
    
    pub cache : AccountLoader<'info, Cache>,

    // user quote token account i.e usdc account
    #[account(mut,
                constraint = quote_token_account.owner == *user.key,
                constraint = quote_token_account.mint == *quote_token_mint.to_account_info().key)]
    pub quote_token_account : Account<'info, TokenAccount>,

    // basket token account belonging to the user
    #[account(mut,
              constraint = basket_token_account.owner == *user.key,
              constraint = basket_token_account.mint == basket.basket_mint, )]
    pub basket_token_account : Account<'info, TokenAccount>,

    // USDC mint i.e which token is used to pay for the basket
    pub quote_token_mint : Account<'info, Mint>,

    // basket mint
    #[account(mut, constraint = basket.basket_mint == basket_token_mint.key(),)]
    pub basket_token_mint : Account<'info, Mint>,
    
    // creates a trade context to be processed by all underlying tokens
    #[account(init,
                seeds = [FRUIT_BASKET_CONTEXT, &user.key.to_bytes(), &[order_id]],
                bump = context_bump,
                payer = user,
                space = 8 + size_of::<BasketTradeContext>(),
            )]
    pub trade_context : AccountLoader<'info, BasketTradeContext>,

    #[account(mut)]
    pub quote_token_transaction_pool : Account<'info, TokenAccount>,
    pub fruit_basket_authority : AccountInfo<'info>,

    pub token_program : AccountInfo<'info>,
    pub system_program : Program<'info, System>,
}

/// Process a token and its market for a context
/// This instruction will buy/sell a specific token in the basket.
/// token will be deposited/taken in/from the pools
/// This method should be always permissionless
#[derive(Accounts)]
pub struct ProcessTokenOnContext<'info> {
    pub fruitbasket_group : AccountLoader<'info, FruitBasketGroup>,

    #[account(mut)]
    pub trade_context : AccountLoader<'info, BasketTradeContext>,

    pub token_mint : Account<'info, Mint>,

    pub quote_token_mint : Account<'info, Mint>,
    #[account(mut)]
    pub basket_token_mint : Account<'info, Mint>,

    pub fruitbasket : Box<Account<'info, Basket>>,
    // accounts related to market and serum
    #[account(mut)]
    pub market: AccountInfo<'info>,
    #[account(mut)]
    pub open_orders: AccountInfo<'info>,
    #[account(mut)]
    pub request_queue: AccountInfo<'info>,
    #[account(mut)]
    pub event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub bids: AccountInfo<'info>,
    #[account(mut)]
    pub asks: AccountInfo<'info>,
    #[account(mut)]
    pub token_vault: AccountInfo<'info>,
    #[account(mut)]
    pub quote_token_vault: AccountInfo<'info>,
    pub vault_signer: AccountInfo<'info>,
    // pool where all tokens are kept
    #[account(mut)]
    pub token_pool : AccountInfo<'info>,
    // pool where all usdc in transaction are kept belonging baskets
    #[account(mut)]
    pub quote_token_transaction_pool : Box<Account<'info, TokenAccount>>,

    pub fruit_basket_authority : AccountInfo<'info>,
    // Programs.
    pub dex_program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    // // Sysvars.
    pub rent: AccountInfo<'info>,
}

/// Finalize and close the context
/// Verify all tokens have been treated.
/// Do all required check and give either baskettoken or usdc to the user
/// context rent returned to the user
#[derive(Accounts)]
pub struct FinalizeContext <'info> {
    pub fruitbasket_group : AccountLoader<'info, FruitBasketGroup>,

    #[account(mut, close = user)]
    pub trade_context : AccountLoader<'info, BasketTradeContext>,

    pub fruitbasket : Box<Account<'info, Basket>>,

    #[account(mut,
        constraint = quote_token_account.owner == user.key(),
        constraint = quote_token_account.mint == quote_token_mint.key())]
    pub quote_token_account : Account<'info, TokenAccount>,

    #[account(mut,
        constraint = basket_token_account.owner == user.key(),
        constraint = basket_token_account.mint == fruitbasket.basket_mint)]
    pub basket_token_account : Account<'info, TokenAccount>,

    #[account(mut)]
    pub quote_token_transaction_pool : Account<'info, TokenAccount>,

    pub fruit_basket_authority : AccountInfo<'info>,

    pub quote_token_mint : Account<'info, Mint>,
    #[account(mut)]
    pub basket_token_mint : Account<'info, Mint>,
    #[account(mut)]
    pub user : AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub system_program : Program<'info, System>,
}
