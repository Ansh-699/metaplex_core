use anchor_lang::prelude::*;
use mpl_core::{
    accounts::BaseAssetV1,
    fetch_plugin,
    instructions::{AddPluginV1CpiBuilder, CreateV2CpiBuilder},
    types::{BurnDelegate, Plugin, PluginAuthority, PluginType},
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct Mint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub asset: Signer<'info>,
    /// CHECK: Metaplex Core collection account
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,
    /// CHECK: PDA authority seed-derived
    #[account(
        seeds = [b"prog_auth", collection.key().as_ref()],
        bump
    )]
    pub prog_auth: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: Metaplex Core program ID
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}
impl<'info> Mint<'info> {
    pub fn mint_nft(&mut self, name: String, uri: String, bumps: &MintBumps) -> Result<()> {
        let collection_key = self.collection.key();
        let signer_seeds = &[
            b"prog_auth",
            collection_key.as_ref(),
            &[bumps.prog_auth],
        ];

        CreateV2CpiBuilder::new(&self.mpl_core_program.to_account_info())
            .asset(&self.asset.to_account_info())
            .collection(Some(&self.collection.to_account_info()))
            .authority(Some(&self.prog_auth.to_account_info()))
            .payer(&self.payer.to_account_info())
            .owner(Some(&self.payer.to_account_info()))
            .update_authority(None)
            .system_program(&self.system_program.to_account_info())
            .name(name)
            .uri(uri)
            .invoke_signed(&[signer_seeds])?;

        match fetch_plugin::<BaseAssetV1, BurnDelegate>(
            &self.asset.to_account_info(),
            PluginType::BurnDelegate,
        ) {
            Err(_) => {
                AddPluginV1CpiBuilder::new(&self.mpl_core_program.to_account_info())
                    .asset(&self.asset.to_account_info())
                    .collection(Some(&self.collection.to_account_info()))
                    .payer(&self.payer.to_account_info())
                    .authority(Some(&self.payer.to_account_info()))
                    .system_program(&self.system_program.to_account_info())
                    .plugin(Plugin::BurnDelegate(BurnDelegate {}))
                    .init_authority(PluginAuthority::UpdateAuthority)
                    .invoke()?;
            }
            Ok(_) => return Ok(()),
        }

        Ok(())
    }
}
