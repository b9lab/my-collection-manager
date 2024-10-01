use crate::{
    error::ContractError,
    msg::{
        CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg, InstantiateMsg,
        NameServiceExecuteMsgResponse,
    },
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, CosmosMsg, DepsMut, Empty, Env, Event, MessageInfo, QueryRequest,
    Reply, ReplyOn, Response, StdError, SubMsg, WasmMsg, WasmQuery,
};
use cw721::msg::NumTokensResponse;

type ContractResult = Result<Response, ContractError>;

enum ReplyCode {
    PassThrough = 1,
}

impl TryFrom<u64> for ReplyCode {
    type Error = ContractError;

    fn try_from(item: u64) -> Result<Self, Self::Error> {
        match item {
            1 => Ok(ReplyCode::PassThrough),
            _ => panic!("invalid ReplyCode({})", item),
        }
    }
}

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
    let onward_sub_msg = SubMsg {
        id: ReplyCode::PassThrough as u64,
        msg: CosmosMsg::<Empty>::Wasm(onward_exec_msg),
        reply_on: ReplyOn::Success,
        gas_limit: None,
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
        .add_submessage(onward_sub_msg)
        .add_event(token_count_event))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> ContractResult {
    match ReplyCode::try_from(msg.id)? {
        ReplyCode::PassThrough => reply_pass_through(deps, env, msg),
    }
}

fn reply_pass_through(_deps: DepsMut, _env: Env, msg: Reply) -> ContractResult {
    let resp = msg.result.into_result().map_err(StdError::generic_err)?;
    let data = if let Some(data) = resp.data {
        data.0[2..].to_vec()
    } else {
        return Ok(Response::default());
    };
    let value = if let Ok(value) = from_json::<NameServiceExecuteMsgResponse>(data) {
        value
    } else {
        return Ok(Response::default());
    };
    let event = Event::new("my-collection-manager")
        .add_attribute("token-count-after", value.num_tokens.to_string());
    Ok(Response::default().add_event(event))
}

#[cfg(test)]
mod tests {
    use crate::{
        contract::ReplyCode,
        msg::{
            CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg, NameServiceExecuteMsgResponse,
        },
    };
    use cosmwasm_std::{
        from_json,
        testing::{self, MockApi, MockQuerier, MockStorage},
        to_json_binary, Addr, Binary, Coin, ContractResult, CosmosMsg, Empty, Event, OwnedDeps,
        Querier, QuerierResult, QueryRequest, Reply, ReplyOn, Response, SubMsg, SubMsgResponse,
        SubMsgResult, SystemError, SystemResult, Uint128, WasmMsg, WasmQuery,
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
            .add_submessage(SubMsg {
                id: ReplyCode::PassThrough as u64,
                msg: CosmosMsg::<Empty>::Wasm(WasmMsg::Execute {
                    contract_addr: "collection".to_owned(),
                    msg: to_json_binary(&inner_msg).expect("Failed to serialize inner message"),
                    funds: vec![fund_sent],
                }),
                reply_on: ReplyOn::Success,
                gas_limit: None,
            })
            .add_event(
                Event::new("my-collection-manager").add_attribute("token-count-before", "3"),
            );
        assert_eq!(received_response, expected_response);
    }

    #[test]
    fn test_reply_pass_through() {
        // Arrange
        let mut mocked_deps_mut = mock_deps(NumTokensResponse { count: 3 });
        let mocked_env = testing::mock_env();
        let num_tokens = to_json_binary(&NameServiceExecuteMsgResponse { num_tokens: 4 })
            .expect("Failed to serialize counter");
        let mut prefixed_num_tokens = vec![10, 16];
        prefixed_num_tokens.extend_from_slice(&num_tokens.as_slice());
        let reply = Reply {
            id: ReplyCode::PassThrough as u64,
            result: SubMsgResult::Ok(SubMsgResponse {
                data: Some(Binary::from(prefixed_num_tokens)),
                events: vec![],
            }),
        };

        // Act
        let contract_result = super::reply(mocked_deps_mut.as_mut(), mocked_env, reply);

        // Assert
        assert!(contract_result.is_ok(), "Failed to pass reply through");
        let received_response = contract_result.unwrap();
        let expected_response = Response::default()
            .add_event(Event::new("my-collection-manager").add_attribute("token-count-after", "4"));
        assert_eq!(received_response, expected_response);
    }
}
