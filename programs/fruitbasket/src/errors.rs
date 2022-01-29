use crate::*;

#[error]
pub enum FruitBasketError {
    #[msg("Token name limit is 10 chars")]
    NameBufferOverflow,
    #[msg("Token limit reached")]
    TokenCountLimitReached,
    #[msg("Unknown auhority")]
    UnknownAuthority,
    #[msg("Current maximum component count is 10")]
    ComponentCountOverflow,
    #[msg("There should be atleast 2 basket components")]
    ComponentCountUnderflow,
    #[msg("While adding a new basket number should match the basket count")]
    BasketNbMismatch,
    #[msg("Token not found in the token list")]
    TokenNotFound,
    #[msg("Price should be greater than 0")]
    PriceEqualOrLessThanZero,
    #[msg("Confidence too low")]
    LowConfidenceInOracle,
    #[msg("Too low maximum buy price may result into trade failure")]
    TooLowMaximumBuyPrice,
    #[msg("Accounts mismatch")]
    AccountsMismatch,
    #[msg("Too high minimum sell price may result into trade failure")]
    TooHighMinimumSellPrice,
    #[msg("Unknown basket")]
    UnknownBasket,
    #[msg("Not all tokens were treated before calling Finalize")]
    NotAllTokensTreatedBeforeFinalize,
    #[msg("Error fetching token description")]
    ErrorDeserializeTokeDesc,
    #[msg("Unknown market")]
    UnknownMarket,
    #[msg("Unknown Open Orders")]
    UnknownOpenOrders
}