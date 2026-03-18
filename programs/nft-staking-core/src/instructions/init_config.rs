use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};
use mpl_core::accounts::BaseCollectionV1;
use crate::state::Config;
use crate::errors::StakingError;

#[derive(Accounts)]
pub struct InitConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: Metaplex Core collection account
    pub collection: UncheckedAccount<'info>,
    /// CHECK: PDA authority seed-derived
    #[account(
        seeds = [b"prog_auth", collection.key().as_ref()],
        bump
    )]
    pub prog_auth: UncheckedAccount<'info>,
    #[account(
        init, 
        payer = admin, 
        space = 8 + Config::INIT_SPACE, 
        seeds = [b"cfg", collection.key().as_ref()], 
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        mint::decimals = 6,
        mint::authority = config,
        seeds = [b"rwrd", config.key().as_ref()],
        bump
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}
impl InitConfig<'_> {
    pub fn init_config(&mut self, points_per_stake: u32, freeze_period: u8, bumps: &InitConfigBumps) -> Result<()> {
        let base_collection = BaseCollectionV1::try_from(&self.collection.to_account_info())?;
        require!(base_collection.update_authority == self.prog_auth.key(), StakingError::InvalidAuthority);

        self.config.set_inner(Config {
            points_per_stake, 
            freeze_period, 
            rewards_bump: bumps.rewards_mint, 
            config_bump: bumps.config });
        Ok(())
    }
}