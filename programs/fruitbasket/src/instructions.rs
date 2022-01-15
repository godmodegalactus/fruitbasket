use anchor_spl::token::Token;
use anchor_spl::dex::serum_dex::state::OpenOrders;
use crate::*;

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

    pub system_program : Program<'info, System>,
}

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
    pub basket : Account<'info, Basket>,

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

#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    pub group : AccountLoader<'info, FruitBasketGroup>,
    #[account(mut)]
    pub cache : AccountLoader<'info, Cache>,
    pub oracle_ai : AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UpdateBasketPrice<'info> {
    #[account(mut)]
    pub basket : Account<'info, Basket>,

    pub cache : AccountLoader<'info, Cache>,
}

#[derive(Accounts)]
pub struct BuyBasket<'info> {
    pub group : AccountLoader<'info, FruitBasketGroup>,

    #[account(signer)]
    pub user : AccountInfo<'info>,

    pub basket : Account<'info, Basket>,

    #[account(mut,
                constraint = paying_account.owner == *user.key,
                constraint = paying_account.mint == *paying_token_mint.to_account_info().key)]
    pub paying_account : Account<'info, TokenAccount>,

    #[account(mut,
              constraint = user_basket_token_account.owner == *user.key,
              constraint = user_basket_token_account.mint == basket.basket_mint, )]
    pub user_basket_token_account : Account<'info, TokenAccount>,

    // USDC mint i.e which token is used to pay for the basket
    pub paying_token_mint : Account<'info, Mint>,
    // Basket tokens
    #[account(mut,
                constraint = *fruit_basket_mint.to_account_info().key == basket.basket_mint )]
    pub fruit_basket_mint: Account<'info, Mint>,

    #[account(mut,
              constraint = fruit_basket_account.mint == *fruit_basket_mint.to_account_info().key)]
    pub fruit_basket_account : Account<'info, TokenAccount>,

    pub authority : AccountInfo<'info>,

    // Programs.
    pub dex_program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    // Sysvars.
    pub rent: AccountInfo<'info>,
}
