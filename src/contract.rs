use crate::{
    error::ContractError,
    msg::{CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg, InstantiateMsg},
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, DepsMut, Env, Event, MessageInfo, QueryRequest, Response, WasmMsg, WasmQuery,
};
use cw721::msg::NumTokensResponse;

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
    deps: DepsMut,
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
    let token_count_result =
        deps.querier
            .query::<NumTokensResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: collection,
                msg: to_json_binary(&CollectionQueryMsg::NumTokens {})?,
            }));
    let token_count_event = Event::new("my-collection-manager")
        .add_attribute("token-count-before", token_count_result?.count.to_string());
    Ok(Response::default()
        .add_message(onward_exec_msg)
        .add_event(token_count_event))
}

#[cfg(test)]
mod tests {
    use crate::msg::{CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg};
    use cosmwasm_std::{
        from_json,
        testing::{self, MockApi, MockQuerier, MockStorage},
        to_json_binary, Addr, Coin, ContractResult, Empty, Event, OwnedDeps, Querier,
        QuerierResult, QueryRequest, Response, SystemError, SystemResult, Uint128, WasmMsg,
        WasmQuery,
    };
    use cw721::msg::NumTokensResponse;
    use std::marker::PhantomData;

    pub fn mock_deps(
        response: NumTokensResponse,
    ) -> OwnedDeps<MockStorage, MockApi, NumTokensMockQuerier, Empty> {
        OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: NumTokensMockQuerier::new(MockQuerier::new(&[]), response),
            custom_query_type: PhantomData,
        }
    }

    pub struct NumTokensMockQuerier {
        base: MockQuerier,
        response: NumTokensResponse,
    }

    impl Querier for NumTokensMockQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            let request: QueryRequest<Empty> = match from_json(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };

            self.handle_query(&request)
        }
    }

    impl NumTokensMockQuerier {
        pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
            match &request {
                QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: _,
                    msg,
                }) => {
                    let expected = to_json_binary(&CollectionQueryMsg::NumTokens {})
                        .expect("Failed to create expected query");
                    assert_eq!(expected.to_vec(), msg.to_vec(), "Query is not num tokens");
                    SystemResult::Ok(ContractResult::Ok(
                        to_json_binary(&self.response)
                            .expect("Failed to serialize num tokens response"),
                    ))
                }
                _ => self.base.handle_query(request),
            }
        }

        pub fn new(base: MockQuerier<Empty>, response: NumTokensResponse) -> Self {
            NumTokensMockQuerier { base, response }
        }
    }

    #[test]
    fn test_pass_through() {
        // Arrange
        let mut mocked_deps_mut = mock_deps(NumTokensResponse { count: 3 });
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
        let expected_response = Response::default()
            .add_message(WasmMsg::Execute {
                contract_addr: "collection".to_owned(),
                msg: to_json_binary(&inner_msg).expect("Failed to serialize inner message"),
                funds: vec![fund_sent],
            })
            .add_event(
                Event::new("my-collection-manager").add_attribute("token-count-before", "3"),
            );
        assert_eq!(received_response, expected_response);
    }
}
