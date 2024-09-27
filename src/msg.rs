use cosmwasm_schema::cw_serde;
use cosmwasm_std::Empty;
use cw721::msg::{Cw721ExecuteMsg, Cw721QueryMsg};

#[cw_serde]
pub struct InstantiateMsg {}

pub type CollectionExecuteMsg = Cw721ExecuteMsg<Option<Empty>, Option<Empty>, Empty>;
pub type CollectionQueryMsg = Cw721QueryMsg<Option<Empty>, Option<Empty>, Empty>;

#[cw_serde]
pub enum ExecuteMsg {
    PassThrough {
        collection: String,
        message: CollectionExecuteMsg,
    },
}
