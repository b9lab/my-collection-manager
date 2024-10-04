use crate::{
    error::ContractError,
    msg::{
        CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg, GetPaymentParamsResponse,
        InstantiateMsg, NameServiceExecuteMsgResponse, PaymentParams, QueryMsg, SudoMsg,
    },
    state::PAYMENT_PARAMS,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, BankMsg, Coin, CosmosMsg, Deps, DepsMut, Empty, Env, Event,
    MessageInfo, QueryRequest, QueryResponse, Reply, ReplyOn, Response, StdError, SubMsg, Uint128,
    WasmMsg, WasmQuery,
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
pub fn instantiate(deps: DepsMut, _: Env, _: MessageInfo, msg: InstantiateMsg) -> ContractResult {
    msg.payment_params.validate()?;
    PAYMENT_PARAMS.save(deps.storage, &msg.payment_params)?;
    let instantiate_event = Event::new("my-collection-manager");
    let instantiate_event = append_payment_params_attributes(instantiate_event, msg.payment_params);
    Ok(Response::default().add_event(instantiate_event))
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
    let response = Response::default();
    let response = match message {
        CollectionExecuteMsg::Mint { .. } => match handle_pre_mint_funds(&deps, &info) {
            Err(err) => Err(err)?,
            Ok(bank_msgs) => response.add_messages(bank_msgs),
        },
        _ => {
            if !info.funds.is_empty() {
                let refund_msg = BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: info.funds,
                };
                response.add_message(refund_msg)
            } else {
                response
            }
        }
    };
    let onward_exec_msg = WasmMsg::Execute {
        contract_addr: collection.to_owned(),
        msg: to_json_binary(&message)?,
        funds: vec![],
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
    Ok(response
        .add_submessage(onward_sub_msg)
        .add_event(token_count_event))
}

fn handle_pre_mint_funds(
    deps: &DepsMut,
    info: &MessageInfo,
) -> Result<Vec<BankMsg>, ContractError> {
    let payment_params = PAYMENT_PARAMS.load(deps.storage)?;
    let (payment, change) = match payment_params.mint_price {
        None => (None, info.funds.to_owned()),
        Some(minting_price) if minting_price.amount.le(&Uint128::zero()) => {
            Err(ContractError::ZeroPrice)?
        }
        Some(minting_price) => {
            let (aggregated, mut others) = split_fund_denom(&minting_price.denom, &info.funds);
            match aggregated.checked_sub(minting_price.amount) {
                Err(_) => Err(ContractError::MissingPayment {
                    missing_payment: minting_price.to_owned(),
                })?,
                Ok(change_in_denom) if change_in_denom.le(&Uint128::zero()) => {}
                Ok(change_in_denom) => others.push(Coin {
                    denom: minting_price.denom.clone(),
                    amount: change_in_denom,
                }),
            };
            (Some(minting_price), others)
        }
    };
    let mut bank_msgs = Vec::<BankMsg>::new();
    if let Some(paid) = payment {
        bank_msgs.push(BankMsg::Send {
            to_address: payment_params.beneficiary.to_string(),
            amount: vec![paid],
        });
    }
    if !change.is_empty() {
        bank_msgs.push(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: change,
        })
    };
    Ok(bank_msgs)
}

fn split_fund_denom(denom: &String, funds: &[Coin]) -> (Uint128, Vec<Coin>) {
    let (amount, others) = funds.iter().fold(
        (Uint128::zero(), Vec::with_capacity(funds.len())),
        |(aggregated, mut others), fund| {
            if &fund.denom == denom {
                (aggregated.strict_add(fund.amount), others)
            } else {
                others.push(fund.clone());
                (aggregated, others)
            }
        },
    );
    (amount, others)
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<QueryResponse, ContractError> {
    match msg {
        QueryMsg::GetPaymentParams {} => Ok(to_json_binary(&GetPaymentParamsResponse {
            payment_params: PAYMENT_PARAMS.load(deps.storage)?,
        })?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> ContractResult {
    match msg {
        SudoMsg::UpdatePaymentParams(payment_params) => {
            sudo_update_payment_params(deps, payment_params)
        }
    }
}

fn sudo_update_payment_params(deps: DepsMut, payment_params: PaymentParams) -> ContractResult {
    payment_params.validate()?;
    PAYMENT_PARAMS.save(deps.storage, &payment_params)?;
    let sudo_event = Event::new("my-collection-manager");
    let sudo_event = append_payment_params_attributes(sudo_event, payment_params);
    Ok(Response::default().add_event(sudo_event))
}

fn append_payment_params_attributes(my_event: Event, payment_params: PaymentParams) -> Event {
    let my_event = my_event.add_attribute(
        "update-payment-params-beneficiary",
        payment_params.beneficiary,
    );
    match payment_params.mint_price {
        None => my_event.add_attribute("update-payment-params-mint-price", "none"),
        Some(mint_price) => my_event
            .add_attribute("update-payment-params-mint-price-denom", mint_price.denom)
            .add_attribute(
                "update-payment-params-mint-price-amount",
                mint_price.amount.to_string(),
            ),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        contract::ReplyCode,
        msg::{
            CollectionExecuteMsg, CollectionQueryMsg, ExecuteMsg, InstantiateMsg,
            NameServiceExecuteMsgResponse, PaymentParams, SudoMsg,
        },
        state::PAYMENT_PARAMS,
    };
    use cosmwasm_std::{
        from_json,
        testing::{self, MockApi, MockQuerier, MockStorage},
        to_json_binary, Addr, BankMsg, Binary, Coin, ContractResult, CosmosMsg, Empty, Event,
        OwnedDeps, Querier, QuerierResult, QueryRequest, Reply, ReplyOn, Response, SubMsg,
        SubMsgResponse, SubMsgResult, SystemError, SystemResult, Uint128, WasmMsg, WasmQuery,
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
        let deployer = Addr::unchecked("deployer");
        let mocked_msg_info = testing::mock_info(&deployer.to_string(), &[]);
        let instantiate_msg = InstantiateMsg {
            payment_params: PaymentParams {
                beneficiary: deployer.to_owned(),
                mint_price: None,
            },
        };
        let _ = super::instantiate(
            mocked_deps_mut.as_mut(),
            mocked_env.to_owned(),
            mocked_msg_info,
            instantiate_msg,
        )
        .expect("Failed to instantiate manager");
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
            .add_message(BankMsg::Send {
                to_address: executer.to_string(),
                amount: vec![fund_sent],
            })
            .add_submessage(SubMsg {
                id: ReplyCode::PassThrough as u64,
                msg: CosmosMsg::<Empty>::Wasm(WasmMsg::Execute {
                    contract_addr: "collection".to_owned(),
                    msg: to_json_binary(&inner_msg).expect("Failed to serialize inner message"),
                    funds: vec![],
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
    fn test_paid_mint_pass_through() {
        // Arrange
        let mut mocked_deps_mut = mock_deps(NumTokensResponse { count: 3 });
        let mocked_env = testing::mock_env();
        let beneficiary = Addr::unchecked("beneficiary");
        let deployer = Addr::unchecked("deployer");
        let mocked_msg_info = testing::mock_info(&deployer.to_string(), &[]);
        let minting_price = Coin {
            amount: Uint128::from(55u16),
            denom: "silver".to_owned(),
        };
        let instantiate_msg = InstantiateMsg {
            payment_params: PaymentParams {
                beneficiary: beneficiary.to_owned(),
                mint_price: Some(minting_price.to_owned()),
            },
        };
        let _ = super::instantiate(
            mocked_deps_mut.as_mut(),
            mocked_env.to_owned(),
            mocked_msg_info,
            instantiate_msg,
        )
        .expect("Failed to instantiate manager");
        let executer = Addr::unchecked("executer");
        let extra_fund_sent = Coin {
            denom: "gold".to_owned(),
            amount: Uint128::from(335u128),
        };
        let fistful_silver = Coin {
            amount: Uint128::from(30u16),
            denom: "silver".to_owned(),
        };
        let mocked_msg_info = testing::mock_info(
            &executer.to_string(),
            &[
                extra_fund_sent.to_owned(),
                fistful_silver.to_owned(),
                fistful_silver,
            ],
        );
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
            mocked_msg_info.to_owned(),
            execute_msg,
        );

        // Assert
        assert!(contract_result.is_ok(), "Failed to pass message through");
        let received_response = contract_result.unwrap();
        let expected_denom_change = Coin {
            amount: Uint128::from(5u16),
            denom: "silver".to_owned(),
        };
        let expected_response = Response::default()
            .add_message(BankMsg::Send {
                to_address: beneficiary.to_string(),
                amount: vec![minting_price],
            })
            .add_message(BankMsg::Send {
                to_address: mocked_msg_info.sender.to_string(),
                amount: vec![extra_fund_sent, expected_denom_change],
            })
            .add_submessage(SubMsg {
                id: ReplyCode::PassThrough as u64,
                msg: CosmosMsg::<Empty>::Wasm(WasmMsg::Execute {
                    contract_addr: "collection".to_owned(),
                    msg: to_json_binary(&inner_msg).expect("Failed to serialize inner message"),
                    funds: vec![],
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

    #[test]
    fn test_sudo_update_payment_params() {
        // Arrange
        let mut mocked_deps_mut = testing::mock_dependencies();
        let mocked_env = testing::mock_env();
        let beneficiary = Addr::unchecked("beneficiary");
        let new_payment_params = PaymentParams {
            beneficiary: beneficiary.to_owned(),
            mint_price: Some(Coin {
                denom: "silver".to_owned(),
                amount: Uint128::one(),
            }),
        };
        let sudo_msg = SudoMsg::UpdatePaymentParams(new_payment_params.to_owned());

        // Act
        let contract_result = super::sudo(mocked_deps_mut.as_mut(), mocked_env, sudo_msg);

        // Assert
        assert!(contract_result.is_ok(), "Failed to sudo");
        let received_response = contract_result.unwrap();
        let expected_response = Response::default().add_event(
            Event::new("my-collection-manager")
                .add_attribute("update-payment-params-beneficiary", beneficiary)
                .add_attribute("update-payment-params-mint-price-denom", "silver")
                .add_attribute("update-payment-params-mint-price-amount", "1"),
        );
        assert_eq!(received_response, expected_response);
        let payment_params = PAYMENT_PARAMS
            .load(&mocked_deps_mut.storage)
            .expect("Failed to load payment params");
        assert_eq!(payment_params, new_payment_params);
    }
}
