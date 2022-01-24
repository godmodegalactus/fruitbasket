use anchor_lang::prelude::*;
use std::io::Write as IoWrite;
declare_id!("FkVPe3KrRE18jXFq4P5CADUSd5YoWiJukBYsxpm2wyB8");

#[program]
pub mod writer_utils {
    use super::*;
    pub fn write(ctx: Context<Write>, offset: u64, data: Vec<u8>) -> ProgramResult {
        let account_data = ctx.accounts.target.to_account_info().data;
        let borrow_data = &mut *account_data.borrow_mut();
        let offset = offset as usize;

        Ok((&mut borrow_data[offset..]).write_all(&data[..])?)
    }
}

#[derive(Accounts)]
pub struct Write<'info> {
    #[account(mut, signer)]
    target: AccountInfo<'info>,
}

