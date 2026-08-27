use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct CreateToken<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = authority,
        mint::freeze_authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(_ctx: Context<CreateToken>, decimals: u8) -> Result<()> {
    msg!("Created mint with {} decimals", decimals);
    Ok(())
}
