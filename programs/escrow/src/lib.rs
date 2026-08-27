use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkgMQHGZ6TqZx");

#[program]
pub mod escrow {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, deal_id: u64, amount: u64) -> Result<()> {
        require!(amount > 0, EscrowError::AmountMustBePositive);
        require_keys_neq!(
            ctx.accounts.sender.key(),
            ctx.accounts.receiver.key(),
            EscrowError::ReceiverMustDiffer
        );

        let state = &mut ctx.accounts.escrow_state;
        state.sender = ctx.accounts.sender.key();
        state.receiver = ctx.accounts.receiver.key();
        state.mint = ctx.accounts.mint.key();
        state.amount = amount;
        state.deal_id = deal_id;
        state.bump = ctx.bumps.escrow_state;
        state.status = EscrowStatus::Created;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>) -> Result<()> {
        require!(
            ctx.accounts.escrow_state.status == EscrowStatus::Created,
            EscrowError::InvalidStatus
        );

        let cpi_accounts = TransferChecked {
            mint: ctx.accounts.mint.to_account_info(),
            from: ctx.accounts.sender_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.sender.to_account_info(),
        };
        let cpi_context = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
        token_interface::transfer_checked(
            cpi_context,
            ctx.accounts.escrow_state.amount,
            ctx.accounts.mint.decimals,
        )?;
        ctx.accounts.escrow_state.status = EscrowStatus::Funded;
        Ok(())
    }

    pub fn release(ctx: Context<Release>) -> Result<()> {
        require!(
            ctx.accounts.escrow_state.status == EscrowStatus::Funded,
            EscrowError::InvalidStatus
        );
        require!(
            ctx.accounts.vault.amount == ctx.accounts.escrow_state.amount,
            EscrowError::VaultAmountMismatch
        );

        let sender_key = ctx.accounts.escrow_state.sender;
        let deal_id = ctx.accounts.escrow_state.deal_id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"escrow",
            sender_key.as_ref(),
            &deal_id,
            &[ctx.accounts.escrow_state.bump],
        ];

        let transfer_accounts = TransferChecked {
            mint: ctx.accounts.mint.to_account_info(),
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.receiver_token_account.to_account_info(),
            authority: ctx.accounts.escrow_state.to_account_info(),
        };
        let signer_seeds_group = [signer_seeds];
        let transfer_context = CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_accounts,
            &signer_seeds_group,
        );
        token_interface::transfer_checked(
            transfer_context,
            ctx.accounts.escrow_state.amount,
            ctx.accounts.mint.decimals,
        )?;
        close_vault(ctx.accounts, signer_seeds)?;
        ctx.accounts.escrow_state.status = EscrowStatus::Released;
        Ok(())
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        require!(
            matches!(
                ctx.accounts.escrow_state.status,
                EscrowStatus::Created | EscrowStatus::Funded
            ),
            EscrowError::InvalidStatus
        );

        let sender_key = ctx.accounts.escrow_state.sender;
        let deal_id = ctx.accounts.escrow_state.deal_id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"escrow",
            sender_key.as_ref(),
            &deal_id,
            &[ctx.accounts.escrow_state.bump],
        ];
        if ctx.accounts.vault.amount > 0 {
            require!(
                ctx.accounts.vault.amount == ctx.accounts.escrow_state.amount,
                EscrowError::VaultAmountMismatch
            );
            let transfer_accounts = TransferChecked {
                mint: ctx.accounts.mint.to_account_info(),
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.sender_token_account.to_account_info(),
                authority: ctx.accounts.escrow_state.to_account_info(),
            };
            let signer_seeds_group = [signer_seeds];
            let transfer_context = CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                transfer_accounts,
                &signer_seeds_group,
            );
            token_interface::transfer_checked(
                transfer_context,
                ctx.accounts.escrow_state.amount,
                ctx.accounts.mint.decimals,
            )?;
        }
        close_vault(ctx.accounts, signer_seeds)?;
        ctx.accounts.escrow_state.status = EscrowStatus::Cancelled;
        Ok(())
    }
}

fn close_vault<'info>(accounts: &impl VaultAccounts<'info>, signer_seeds: &[&[u8]]) -> Result<()> {
    let close_accounts = CloseAccount {
        account: accounts.vault().to_account_info(),
        destination: accounts.sender_account_info(),
        authority: accounts.escrow_state_account_info(),
    };
    let signer_seeds_group = [signer_seeds];
    let close_context = CpiContext::new_with_signer(
        accounts.token_program_key(),
        close_accounts,
        &signer_seeds_group,
    );
    token_interface::close_account(close_context)
}

trait VaultAccounts<'info> {
    fn vault(&self) -> &InterfaceAccount<'info, TokenAccount>;
    fn escrow_state_account_info(&self) -> AccountInfo<'info>;
    fn sender_account_info(&self) -> AccountInfo<'info>;
    fn token_program_key(&self) -> Pubkey;
}

#[derive(Accounts)]
#[instruction(deal_id: u64, amount: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    pub receiver: SystemAccount<'info>,
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(init, payer = sender, space = 8 + EscrowState::SPACE, seeds = [b"escrow", sender.key().as_ref(), &deal_id.to_le_bytes()], bump)]
    pub escrow_state: Account<'info, EscrowState>,
    #[account(init, payer = sender, seeds = [b"vault", escrow_state.key().as_ref()], bump, token::mint = mint, token::authority = escrow_state, token::token_program = token_program)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub sender: Signer<'info>,
    #[account(mut, has_one = sender, has_one = mint, constraint = escrow_state.status == EscrowStatus::Created @ EscrowError::InvalidStatus)]
    pub escrow_state: Account<'info, EscrowState>,
    #[account(mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint, token::authority = sender, token::token_program = token_program)]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, seeds = [b"vault", escrow_state.key().as_ref()], bump, token::mint = mint, token::authority = escrow_state, token::token_program = token_program)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    #[account(mut, close = sender, has_one = sender, has_one = receiver, has_one = mint, seeds = [b"escrow", sender.key().as_ref(), &escrow_state.deal_id.to_le_bytes()], bump = escrow_state.bump, constraint = escrow_state.status == EscrowStatus::Funded @ EscrowError::InvalidStatus)]
    pub escrow_state: Account<'info, EscrowState>,
    pub receiver: SystemAccount<'info>,
    #[account(mut, mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, seeds = [b"vault", escrow_state.key().as_ref()], bump, token::mint = mint, token::authority = escrow_state, token::token_program = token_program)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(init_if_needed, payer = sender, associated_token::mint = mint, associated_token::authority = receiver, associated_token::token_program = token_program)]
    pub receiver_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    #[account(mut, close = sender, has_one = sender, has_one = mint, seeds = [b"escrow", sender.key().as_ref(), &escrow_state.deal_id.to_le_bytes()], bump = escrow_state.bump, constraint = escrow_state.status != EscrowStatus::Released @ EscrowError::InvalidStatus, constraint = escrow_state.status != EscrowStatus::Cancelled @ EscrowError::InvalidStatus)]
    pub escrow_state: Account<'info, EscrowState>,
    #[account(mut, mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint, token::authority = sender, token::token_program = token_program)]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, seeds = [b"vault", escrow_state.key().as_ref()], bump, token::mint = mint, token::authority = escrow_state, token::token_program = token_program)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct EscrowState {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub deal_id: u64,
    pub bump: u8,
    pub status: EscrowStatus,
}

impl EscrowState {
    pub const SPACE: usize = 32 + 32 + 32 + 8 + 8 + 1 + 1;
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum EscrowStatus {
    Created,
    Funded,
    Released,
    Cancelled,
}

#[error_code]
pub enum EscrowError {
    #[msg("Amount must be greater than zero")]
    AmountMustBePositive,
    #[msg("Receiver must be different from sender")]
    ReceiverMustDiffer,
    #[msg("Escrow is not in the required status")]
    InvalidStatus,
    #[msg("Vault balance does not match escrow amount")]
    VaultAmountMismatch,
}

impl<'info> VaultAccounts<'info> for Release<'info> {
    fn vault(&self) -> &InterfaceAccount<'info, TokenAccount> {
        &self.vault
    }
    fn escrow_state_account_info(&self) -> AccountInfo<'info> {
        self.escrow_state.to_account_info()
    }
    fn sender_account_info(&self) -> AccountInfo<'info> {
        self.sender.to_account_info()
    }
    fn token_program_key(&self) -> Pubkey {
        self.token_program.key()
    }
}

impl<'info> VaultAccounts<'info> for Cancel<'info> {
    fn vault(&self) -> &InterfaceAccount<'info, TokenAccount> {
        &self.vault
    }
    fn escrow_state_account_info(&self) -> AccountInfo<'info> {
        self.escrow_state.to_account_info()
    }
    fn sender_account_info(&self) -> AccountInfo<'info> {
        self.sender.to_account_info()
    }
    fn token_program_key(&self) -> Pubkey {
        self.token_program.key()
    }
}
