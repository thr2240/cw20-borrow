use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub token_address: Addr,
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        admin: Addr,
        token_address: Addr,
        cr: u32,
    },
    Deposit {
        receiver: String,
        amount: Uint128,
    },
    Withdraw {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    GetConfig {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: Addr,
    pub token_address: Addr,
    pub cr: u32,
}
