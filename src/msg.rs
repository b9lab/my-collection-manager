use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin, Empty, Uint128};
use cw721::msg::{Cw721ExecuteMsg, Cw721QueryMsg};

use crate::error::ContractError;

#[cw_serde]
pub struct InstantiateMsg {
    pub payment_params: PaymentParams,
}

#[cw_serde]
pub struct PaymentParams {
    pub beneficiary: Addr,
    pub mint_price: Option<Coin>,
}

impl PaymentParams {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.mint_price {
            Some(coin) if coin.amount.le(&Uint128::zero()) => Err(ContractError::ZeroPrice),
            None | Some(_) => Ok(()),
        }
    }
}

pub type CollectionExecuteMsg = Cw721ExecuteMsg<Option<Empty>, Option<Empty>, Empty>;
pub type CollectionQueryMsg = Cw721QueryMsg<Option<Empty>, Option<Empty>, Empty>;

#[cw_serde]
pub enum ExecuteMsg {
    PassThrough {
        collection: String,
        message: CollectionExecuteMsg,
    },
}

#[cw_serde]
pub struct NameServiceExecuteMsgResponse {
    pub num_tokens: u64,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GetPaymentParamsResponse)]
    GetPaymentParams,
}

#[cw_serde]
pub struct GetPaymentParamsResponse {
    pub payment_params: PaymentParams,
}

#[cw_serde]
pub enum SudoMsg {
    UpdatePaymentParams(PaymentParams),
}

#[cw_serde]
pub struct MigrateMsg {
    pub payment_params: PaymentParams,
}
