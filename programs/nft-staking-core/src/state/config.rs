use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub points_per_stake: u32,
    pub freeze_period: u8,
    pub rewards_bump: u8,
    pub config_bump: u8,
}