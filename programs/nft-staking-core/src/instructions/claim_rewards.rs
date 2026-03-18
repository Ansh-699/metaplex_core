use anchor_lang::prelude::*;
use anchor_spl::{token_interface::{Mint, TokenInterface, TokenAccount, MintToChecked, mint_to_checked}, associated_token::AssociatedToken};
use mpl_core::{
    ID as MPL_CORE_ID,
    accounts::{BaseAssetV1, BaseCollectionV1}, 
    fetch_plugin, 
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority}
};
use crate::state::Config;
use crate::errors::StakingError;

// Constant for time calculations
const SECONDS_PER_DAY: i64 = 86400;

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub stakeholder: Signer<'info>,
    #[account(
        seeds = [b"prog_auth", collection.key().as_ref()],
        bump
    )]
    pub prog_auth: UncheckedAccount<'info>,
    #[account(
        seeds = [b"cfg", collection.key().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut, 
        seeds = [b"rwrd", config.key().as_ref()],
        bump = config.rewards_bump
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init_if_needed,
        payer = stakeholder,
        associated_token::mint = rewards_mint,
        associated_token::authority = stakeholder,
    )]
    pub stakeholder_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> ClaimRewards<'info> {
    pub fn claim_rewards(&mut self, bumps: &ClaimRewardsBumps) -> Result<()>{
        let base_asset = BaseAssetV1::try_from(&self.asset.to_account_info())?;
        require!(base_asset.owner == self.stakeholder.key(), StakingError::InvalidOwner);
        require!(base_asset.update_authority == UpdateAuthority::Collection(self.collection.key()), StakingError::InvalidAuthority);
        let base_collection = BaseCollectionV1::try_from(&self.collection.to_account_info())?;
        require!(base_collection.update_authority == self.prog_auth.key(), StakingError::InvalidAuthority);

        let collection_key = self.collection.key();
        let signer_seeds = &[
            b"prog_auth",
            collection_key.as_ref(),
            &[bumps.prog_auth],
        ];

        let current_timestamp = Clock::get()?.unix_timestamp;

        let fetched_attribute_list = match fetch_plugin::<BaseAssetV1, Attributes>(&self.asset.to_account_info(), PluginType::Attributes) {
            Err(_) => {
                return Err(StakingError::NotStaked.into());
            }
            Ok((_, attributes, _)) => attributes,
        };

        let mut attribute_list: Vec<Attribute> = Vec::with_capacity(fetched_attribute_list.attribute_list.len());
        let mut is_staked: Option<&str> = None;
        let mut staked_at_value: Option<&str> = None;

        for attribute in &fetched_attribute_list.attribute_list {
            match attribute.key.as_str() {
                "staked" => {
                    is_staked = Some(&attribute.value);
                    attribute_list.push(attribute.clone());
                }
                "staked_at" => {
                    staked_at_value = Some(&attribute.value);
                    attribute_list.push(Attribute { 
                        key: "staked_at".to_string(), 
                        value: current_timestamp.to_string() 
                    });
                }
                _ => {
                    attribute_list.push(attribute.clone());
                }
            }
        }

        let raw_staked_at_value = staked_at_value.ok_or(StakingError::InvalidTimestamp)?;
        let clean_staked_at_value = raw_staked_at_value.trim_matches(char::from(0)).trim();
        require!(is_staked == Some("true"), StakingError::NotStaked);

        let staked_at_timestamp = clean_staked_at_value.parse::<i64>().map_err(|_| StakingError::InvalidTimestamp)?;

        let elapsed_seconds = current_timestamp.checked_sub(staked_at_timestamp).ok_or(StakingError::InvalidTimestamp)?;
        let staked_time_days = elapsed_seconds.checked_div(SECONDS_PER_DAY).ok_or(StakingError::InvalidTimestamp)?;

        require!(staked_time_days > 0, StakingError::FreezePeriodNotElapsed);

        let total_points_over_days = (self.config.points_per_stake as u64).checked_mul(staked_time_days as u64).ok_or(StakingError::Overflow)?;

        let config_seeds = &[
            b"cfg",
            collection_key.as_ref(),
            &[self.config.config_bump],
        ];
        let config_signer_seeds = &[&config_seeds[..]];
        
        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = MintToChecked {
            mint: self.rewards_mint.to_account_info(),
            to: self.stakeholder_ata.to_account_info(),
            authority: self.config.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, config_signer_seeds);
        mint_to_checked(cpi_ctx, total_points_over_days, self.rewards_mint.decimals)?;

        UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .payer(&self.stakeholder.to_account_info())
            .authority(Some(&self.prog_auth.to_account_info()))
            .system_program(&self.system_program.to_account_info())
            .plugin(Plugin::Attributes(Attributes { attribute_list }))
            .invoke_signed(&[signer_seeds])?;
        
        Ok(())
    }
}