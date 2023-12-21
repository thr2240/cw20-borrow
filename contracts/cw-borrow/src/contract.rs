use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

#[warn(deprecated)]
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, WasmMsg, QuerierWrapper,  BalanceResponse as NativeBalanceResponse, BankQuery, WasmQuery,
    QueryRequest
};
use cw2::set_contract_version;
use cw20::{Balance, Cw20ExecuteMsg, Denom, BalanceResponse as CW20BalanceResponse, Cw20QueryMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-borrow";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let config = Config {
        owner: _info.sender.clone(),
        token_address: msg.token_address,
        cr: 200,
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            admin,
            token_address,
            cr,
        } => execute_update_config(deps, info.sender, admin, token_address, cr),
        ExecuteMsg::Deposit { receiver, amount } => {
            execute_deposit(deps, receiver, amount, info)
        }
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    sender: Addr,
    new_admin: Addr,
    token_address: Addr,
    cr: u32,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    if sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.owner = new_admin;
        exists.token_address = token_address;
        exists.cr = cr;
        Ok(exists)
    })?;

    Ok(Response::new().add_attribute("action", "update_constants"))
}

fn get_amount_of_denom(balance: Balance, denom: Denom) -> Result<Uint128, ContractError> {
    match denom.clone() {
        Denom::Native(native_str) => match balance {
            Balance::Native(native_balance) => {
                let zero_coin = &Coin {
                    denom: String::from("empty"),
                    amount: Uint128::zero(),
                };
                let (_index, coin) = native_balance
                    .0
                    .iter()
                    .enumerate()
                    .find(|(_i, c)| c.denom == native_str)
                    .unwrap_or((0, zero_coin));

                if coin.amount == Uint128::zero() {
                    return Err(ContractError::NativeInputZero {});
                }
                return Ok(coin.amount);
            }
            Balance::Cw20(_) => {
                return Err(ContractError::TokenTypeMismatch {});
            }
        },
        Denom::Cw20(cw20_address) => match balance {
            Balance::Native(_) => {
                return Err(ContractError::TokenTypeMismatch {});
            }
            Balance::Cw20(token) => {
                if cw20_address != token.address {
                    return Err(ContractError::TokenTypeMismatch {});
                }
                if token.amount == Uint128::zero() {
                    return Err(ContractError::Cw20InputZero {});
                }
                return Ok(token.amount);
            }
        },
    }
}

pub fn transfer_token_message(
    denom: Denom,
    amount: Uint128,
    receiver: Addr,
) -> Result<CosmosMsg, ContractError> {
    match denom.clone() {
        Denom::Native(native_str) => {
            return Ok(BankMsg::Send {
                to_address: receiver.clone().into(),
                amount: vec![Coin {
                    denom: native_str,
                    amount,
                }],
            }
            .into());
        }
        Denom::Cw20(cw20_address) => {
            return Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: cw20_address.clone().into(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: receiver.clone().into(),
                    amount,
                })?,
            }));
        }
    }
}

fn get_token_amount(
    querier: QuerierWrapper,
    denom: Denom,
    contract_addr: Addr,
) -> Result<Uint128, ContractError> {
    match denom.clone() {
        Denom::Native(native_str) => {
            let native_response: NativeBalanceResponse =
                querier.query(&QueryRequest::Bank(BankQuery::Balance {
                    address: contract_addr.clone().into(),
                    denom: native_str,
                }))?;
            return Ok(native_response.amount.amount);
        }
        Denom::Cw20(cw20_address) => {
            let balance_response: CW20BalanceResponse =
                querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: cw20_address.clone().into(),
                    msg: to_binary(&Cw20QueryMsg::Balance {
                        address: contract_addr.clone().into(),
                    })?,
                }))?;
            return Ok(balance_response.balance);
        }
    }
}

pub fn execute_deposit(
    deps: DepsMut,
    receiver: String,
    _amount: Uint128,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let amount = get_amount_of_denom(
        Balance::from(info.funds),
        Denom::Native(String::from("ucore")),
    )?;

    let mut msgs: Vec<CosmosMsg> = vec![];

    msgs.push(transfer_token_message(
        Denom::Cw20(cfg.token_address.clone()),
        amount * Uint128::from(cfg.cr),
        info.sender,
    )?);

    Ok(Response::new()
        .add_attribute("method", "deposit")
        .add_attribute("receiver", receiver)
        .add_messages(msgs))
}


pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    let tot = get_token_amount(
        deps.querier,
        Denom::Native(String::from("inj")),
        env.contract.address.clone(),
    )?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    msgs.push(transfer_token_message(
        Denom::Native(String::from("inj")),
        tot,
        info.sender.clone(),
    )?);
    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_messages(msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => query_get_config(deps),
    }
}

pub fn query_get_config(deps: Deps) -> StdResult<Binary> {
    let cfg = CONFIG.load(deps.storage)?;
    
    to_binary(&ConfigResponse {
        owner: cfg.owner,
        token_address: cfg.token_address,
        cr: cfg.cr,
    })
}