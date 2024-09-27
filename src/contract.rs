use crate::{
    error::ContractError,
    msg::{CollectionExecuteMsg, ExecuteMsg, InstantiateMsg},
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, DepsMut, Env, MessageInfo, Response, WasmMsg};

type ContractResult = Result<Response, ContractError>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(_: DepsMut, _: Env, _: MessageInfo, _: InstantiateMsg) -> ContractResult {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> ContractResult {
    match msg {
        ExecuteMsg::PassThrough {
            collection,
            message,
        } => execute_pass_through(deps, env, info, collection, message),
    }
}

fn execute_pass_through(
    _: DepsMut,
    _: Env,
    info: MessageInfo,
    collection: String,
    message: CollectionExecuteMsg,
) -> ContractResult {
    let onward_exec_msg = WasmMsg::Execute {
        contract_addr: collection.to_owned(),
        msg: to_json_binary(&message)?,
        funds: info.funds,
    };
    Ok(Response::default().add_message(onward_exec_msg))
}

#[cfg(test)]
mod tests {
    use crate::msg::{CollectionExecuteMsg, ExecuteMsg};
    use cosmwasm_std::{testing, to_json_binary, Addr, Coin, Response, Uint128, WasmMsg};

    #[test]
    fn test_pass_through() {
        // Arrange
        let mut mocked_deps_mut = testing::mock_dependencies();
        let mocked_env = testing::mock_env();
        let executer = Addr::unchecked("executer");
        let fund_sent = Coin {
            denom: "gold".to_owned(),
            amount: Uint128::from(335u128),
        };
        let mocked_msg_info = testing::mock_info(&executer.to_string(), &[fund_sent.to_owned()]);
        let name = "alice".to_owned();
        let owner = Addr::unchecked("owner");
        let inner_msg = CollectionExecuteMsg::Mint {
            token_id: name.to_owned(),
            owner: owner.to_string(),
            token_uri: None,
            extension: None,
        };
        let execute_msg = ExecuteMsg::PassThrough {
            collection: "collection".to_owned(),
            message: inner_msg.to_owned(),
        };

        // Act
        let contract_result = super::execute(
            mocked_deps_mut.as_mut(),
            mocked_env,
            mocked_msg_info,
            execute_msg,
        );

        // Assert
        assert!(contract_result.is_ok(), "Failed to pass message through");
        let received_response = contract_result.unwrap();
        let expected_response = Response::default().add_message(WasmMsg::Execute {
            contract_addr: "collection".to_owned(),
            msg: to_json_binary(&inner_msg).expect("Failed to serialize inner message"),
            funds: vec![fund_sent],
        });
        assert_eq!(received_response, expected_response);
    }
}
