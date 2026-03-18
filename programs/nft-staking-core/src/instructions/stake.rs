use crate::errors::StakingError;
use crate::state::Config;
use anchor_lang::prelude::*;
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::{
        AddCollectionPluginV1CpiBuilder, AddPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder,
        UpdatePluginV1CpiBuilder,
    },
    types::{
        Attribute, Attributes, FreezeDelegate, Plugin, PluginAuthority, PluginType, UpdateAuthority,
    },
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct Stake<'info> {
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
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        let base_asset = BaseAssetV1::try_from(&self.asset.to_account_info())?;
        require!(
            base_asset.owner == self.stakeholder.key(),
            StakingError::InvalidOwner
        );
        require!(
            base_asset.update_authority == UpdateAuthority::Collection(self.collection.key()),
            StakingError::InvalidAuthority
        );
        let base_collection = BaseCollectionV1::try_from(&self.collection.to_account_info())?;
        require!(
            base_collection.update_authority == self.prog_auth.key(),
            StakingError::InvalidAuthority
        );

        let collection_key = self.collection.key();
        let signer_seeds = &[
            b"prog_auth",
            collection_key.as_ref(),
            &[bumps.prog_auth],
        ];

        let current_time = Clock::get()?.unix_timestamp;

        match fetch_plugin::<BaseAssetV1, Attributes>(
            &self.asset.to_account_info(),
            PluginType::Attributes,
        ) {
            Err(_) => {
                AddPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .asset(&self.asset.to_account_info())
                    .collection(Some(&self.collection.to_account_info()))
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.prog_auth.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::Attributes(Attributes {
                        attribute_list: vec![
                            Attribute {
                                key: "staked".to_string(),
                                value: "true".to_string(),
                            },
                            Attribute {
                                key: "staked_at".to_string(),
                                value: current_time.to_string(),
                            },
                        ],
                    }))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke_signed(&[signer_seeds])?;
            }
            Ok((_, fetched_attribute_list, _)) => {
                let mut attribute_list: Vec<Attribute> = Vec::new();
                let mut staked = false;
                let mut staked_at = false;
                for attribute in fetched_attribute_list.attribute_list {
                    if attribute.key == "staked" {
                        require!(attribute.value == "false", StakingError::AlreadyStaked);
                        attribute_list.push(Attribute {
                            key: "staked".to_string(),
                            value: "true".to_string(),
                        });
                        staked = true;
                    } else if attribute.key == "staked_at" {
                        attribute_list.push(Attribute {
                            key: "staked_at".to_string(),
                            value: current_time.to_string(),
                        });
                        staked_at = true;
                    } else {
                        attribute_list.push(attribute);
                    }
                }
                if !staked {
                    attribute_list.push(Attribute {
                        key: "staked".to_string(),
                        value: "true".to_string(),
                    });
                }
                if !staked_at {
                    attribute_list.push(Attribute {
                        key: "staked_at".to_string(),
                        value: current_time.to_string(),
                    });
                }
                UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .asset(&self.asset.to_account_info())
                    .collection(Some(&self.collection.to_account_info()))
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.prog_auth.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::Attributes(Attributes { attribute_list }))
                    .invoke_signed(&[signer_seeds])?;
            }
        }

        match fetch_plugin::<BaseAssetV1, FreezeDelegate>(
            &self.asset.to_account_info(),
            PluginType::FreezeDelegate,
        ) {
            Err(_) => {
                AddPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .asset(&self.asset.to_account_info())
                    .collection(Some(&self.collection.to_account_info()))
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.stakeholder.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke()?;
            }
            Ok(_) => {
                UpdatePluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .asset(&self.asset.to_account_info())
                    .collection(Some(&self.collection.to_account_info()))
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.prog_auth.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                    .invoke_signed(&[signer_seeds])?;
            }
        }

        match fetch_plugin::<BaseCollectionV1, Attributes>(
            &self.collection.to_account_info(),
            PluginType::Attributes,
        ) {
            Ok((_, fetched_plugin_attributes_list, _)) => {
                let mut plugin_attribute_list: Vec<Attribute> = Vec::new();
                let mut found = false;

                for attribute in fetched_plugin_attributes_list.attribute_list {
                    if attribute.key == "total_staked" {
                        let current_val = attribute.value.parse::<u64>().unwrap_or(0);
                        plugin_attribute_list.push(Attribute {
                            key: "total_staked".to_string(),
                            value: (current_val + 1).to_string(),
                        });
                        found = true;
                    } else {
                        plugin_attribute_list.push(attribute);
                    }
                }

                if !found {
                    plugin_attribute_list.push(Attribute {
                        key: "total_staked".to_string(),
                        value: "1".to_string(),
                    });
                }

                UpdateCollectionPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .collection(&self.collection.to_account_info())
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.prog_auth.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::Attributes(Attributes {
                        attribute_list: plugin_attribute_list,
                    }))
                    .invoke_signed(&[signer_seeds])?;
            }
            Err(_) => {
                AddCollectionPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .collection(&self.collection.to_account_info())
                    .payer(&self.stakeholder.to_account_info())
                    .authority(Some(&self.prog_auth.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::Attributes(Attributes {
                        attribute_list: vec![Attribute {
                            key: "total_staked".to_string(),
                            value: "1".to_string(),
                        }],
                    }))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke_signed(&[signer_seeds])?;
            }
        }

        Ok(())
    }
}
