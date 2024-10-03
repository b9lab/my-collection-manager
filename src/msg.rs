use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Empty};
use cw721::msg::{Cw721ExecuteMsg, Cw721QueryMsg};

#[cw_serde]
pub struct InstantiateMsg {
    pub payment_params: PaymentParams,
}

#[cw_serde]
pub struct PaymentParams {
    pub beneficiary: Addr,
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
