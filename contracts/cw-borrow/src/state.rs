use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use cosmwasm_std::{Addr};
use cw_storage_plus::{Item};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub token_address: Addr,
    pub cr: u32,
}

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);