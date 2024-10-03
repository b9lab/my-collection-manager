use cosmwasm_std::{to_json_binary, Addr, Coin, Empty, Event, Uint128};
use cw721::msg::{Cw721ExecuteMsg, Cw721QueryMsg, OwnerOfResponse};
use cw_multi_test::{App, AppBuilder, ContractWrapper, Executor};
use my_collection_manager::{
    contract::{execute, instantiate, reply},
    msg::{ExecuteMsg, InstantiateMsg, PaymentParams},
};
use my_nameservice::{
    contract::{
        execute as execute_my_nameservice, instantiate as instantiate_my_nameservice,
        query as query_my_nameservice,
    },
    msg::InstantiateMsg as MyNameserviceInstantiateMsg,
};

pub type CollectionExecuteMsg = Cw721ExecuteMsg<Option<Empty>, Option<Empty>, Empty>;
pub type CollectionQueryMsg = Cw721QueryMsg<Option<Empty>, Option<Empty>, Empty>;

fn instantiate_nameservice(mock_app: &mut App, minter: String) -> (u64, Addr) {
    let nameservice_code = Box::new(ContractWrapper::new(
        execute_my_nameservice,
        instantiate_my_nameservice,
        query_my_nameservice,
    ));
    let nameservice_code_id = mock_app.store_code(nameservice_code);
    return (
        nameservice_code_id,
        mock_app
            .instantiate_contract(
                nameservice_code_id,
                Addr::unchecked("deployer-my-nameservice"),
                &MyNameserviceInstantiateMsg {
                    name: "my names".to_owned(),
                    symbol: "MYN".to_owned(),
                    creator: None,
                    minter: Some(minter),
                    collection_info_extension: None,
                    withdraw_address: None,
                },
                &[],
                "nameservice",
                None,
            )
            .expect("Failed to instantiate my nameservice"),
    );
}

fn instantiate_collection_manager(
    mock_app: &mut App,
    payment_params: PaymentParams,
) -> (u64, Addr) {
    let code = Box::new(
        ContractWrapper::new(execute, instantiate, |_, _, _: ()| {
            to_json_binary("mocked_manager_query")
        })
        .with_reply(reply),
    );
    let manager_code_id = mock_app.store_code(code);

    return (
        manager_code_id,
        mock_app
            .instantiate_contract(
                manager_code_id,
                Addr::unchecked("deployer-manager"),
                &InstantiateMsg { payment_params },
                &[],
                "my-collection-manager",
                None,
            )
            .expect("Failed to instantiate collection manager"),
    );
}

#[test]
fn test_mint_through() {
    // Arrange
    let mut mock_app = App::default();
    let beneficiary_addr = Addr::unchecked("beneficiary");
    let (_, addr_manager) = instantiate_collection_manager(
        &mut mock_app,
        PaymentParams {
            beneficiary: beneficiary_addr.to_owned(),
        },
    );
    let (_, addr_collection) = instantiate_nameservice(&mut mock_app, addr_manager.to_string());
    let owner_addr = Addr::unchecked("owner");
    let name_alice = "alice".to_owned();
    let sender_addr = Addr::unchecked("sender");
    let register_msg = ExecuteMsg::PassThrough {
        collection: addr_collection.to_string(),
        message: CollectionExecuteMsg::Mint {
            token_id: name_alice.clone(),
            owner: owner_addr.to_string(),
            token_uri: None,
            extension: None,
        },
    };

    // Act
    let result = mock_app.execute_contract(
        sender_addr.clone(),
        addr_manager.clone(),
        &register_msg,
        &[],
    );

    // Assert
    assert!(result.is_ok(), "Failed to pass through the message");
    let result = result.unwrap();
    let expected_cw721_event = Event::new("wasm")
        .add_attribute("_contract_address", addr_collection.to_string())
        .add_attribute("action", "mint")
        .add_attribute("token_id", name_alice.to_string())
        .add_attribute("owner", owner_addr.to_string());
    result.assert_event(&expected_cw721_event);
    let expected_manager_event =
        Event::new("wasm-my-collection-manager").add_attribute("token-count-before", "0");
    result.assert_event(&expected_manager_event);
    let expected_manager_event =
        Event::new("wasm-my-collection-manager").add_attribute("token-count-after", "1");
    result.assert_event(&expected_manager_event);
    let owner_query = CollectionQueryMsg::OwnerOf {
        token_id: name_alice.to_string(),
        include_expired: None,
    };
    let result = mock_app
        .wrap()
        .query_wasm_smart::<OwnerOfResponse>(addr_collection, &owner_query);
    assert!(result.is_ok(), "Failed to query alice name");
    assert_eq!(
        result.unwrap(),
        OwnerOfResponse {
            owner: owner_addr.to_string(),
            approvals: vec![],
        }
    );
}

#[test]
fn test_paid_mint_through() {
    // Arrange
    let sender_addr = Addr::unchecked("sender");
    let extra_fund_sent = Coin {
        denom: "gold".to_owned(),
        amount: Uint128::from(335u128),
    };
    let mut mock_app = AppBuilder::default().build(|router, _api, storage| {
        router
            .bank
            .init_balance(
                storage,
                &sender_addr,
                vec![extra_fund_sent.to_owned()],
            )
            .expect("Failed to init bank balances");
    });
    let beneficiary = Addr::unchecked("beneficiary");
    let (_, addr_manager) = instantiate_collection_manager(
        &mut mock_app,
        PaymentParams {
            beneficiary: beneficiary.to_owned(),
        },
    );
    let (_, addr_collection) = instantiate_nameservice(&mut mock_app, addr_manager.to_string());
    let owner_addr = Addr::unchecked("owner");
    let name_alice = "alice".to_owned();
    let register_msg = ExecuteMsg::PassThrough {
        collection: addr_collection.to_string(),
        message: CollectionExecuteMsg::Mint {
            token_id: name_alice.clone(),
            owner: owner_addr.to_string(),
            token_uri: None,
            extension: None,
        },
    };

    // Act
    let result = mock_app.execute_contract(
        sender_addr.clone(),
        addr_manager.clone(),
        &register_msg,
        &[extra_fund_sent.to_owned()],
    );

    // Assert
    assert!(result.is_ok(), "Failed to pass through the message");
    let result = result.unwrap();
    let expected_beneficiary_bank_event = Event::new("transfer")
        .add_attribute("recipient", "beneficiary")
        .add_attribute("sender", "contract0")
        .add_attribute("amount", "335gold");
    result.assert_event(&expected_beneficiary_bank_event);
    assert_eq!(
        Vec::<Coin>::new(),
        mock_app
            .wrap()
            .query_all_balances(sender_addr)
            .expect("Failed to get sender balances")
    );
    assert_eq!(
        vec![extra_fund_sent],
        mock_app
            .wrap()
            .query_all_balances(beneficiary)
            .expect("Failed to get beneficiary balances")
    );
    assert_eq!(
        Vec::<Coin>::new(),
        mock_app
            .wrap()
            .query_all_balances(addr_manager)
            .expect("Failed to get manager balances")
    );
    assert_eq!(
        Vec::<Coin>::new(),
        mock_app
            .wrap()
            .query_all_balances(addr_collection)
            .expect("Failed to get collection balances")
    );
}

#[test]
fn test_mint_num_tokens() {
    // Arrange
    let mut mock_app = App::default();
    let beneficiary_addr = Addr::unchecked("beneficiary");
    let (_, addr_manager) = instantiate_collection_manager(
        &mut mock_app,
        PaymentParams {
            beneficiary: beneficiary_addr.to_owned(),
        },
    );
    let (_, addr_collection) = instantiate_nameservice(&mut mock_app, addr_manager.to_string());
    let owner_addr = Addr::unchecked("owner");
    let name_alice = "alice".to_owned();
    let name_bob = "bob".to_owned();
    let sender_addr = Addr::unchecked("sender");
    let register_msg = ExecuteMsg::PassThrough {
        collection: addr_collection.to_string(),
        message: CollectionExecuteMsg::Mint {
            token_id: name_alice.clone(),
            owner: owner_addr.to_string(),
            token_uri: None,
            extension: None,
        },
    };
    let _ = mock_app
        .execute_contract(
            sender_addr.clone(),
            addr_manager.clone(),
            &register_msg,
            &[],
        )
        .expect("Failed to pass through the first mint message");
    let register_msg = ExecuteMsg::PassThrough {
        collection: addr_collection.to_string(),
        message: CollectionExecuteMsg::Mint {
            token_id: name_bob.clone(),
            owner: owner_addr.to_string(),
            token_uri: None,
            extension: None,
        },
    };

    // Act
    let result = mock_app.execute_contract(
        sender_addr.clone(),
        addr_manager.clone(),
        &register_msg,
        &[],
    );

    // Assert
    assert!(
        result.is_ok(),
        "Failed to pass through the second mint message"
    );
    let result = result.unwrap();
    let expected_cw721_event = Event::new("wasm")
        .add_attribute("_contract_address", addr_collection.to_string())
        .add_attribute("action", "mint")
        .add_attribute("token_id", name_bob.to_string())
        .add_attribute("owner", owner_addr.to_string());
    result.assert_event(&expected_cw721_event);
    let expected_manager_event =
        Event::new("wasm-my-collection-manager").add_attribute("token-count-before", "1");
    result.assert_event(&expected_manager_event);
    let expected_manager_event =
        Event::new("wasm-my-collection-manager").add_attribute("token-count-after", "2");
    result.assert_event(&expected_manager_event);
    assert_eq!(
        mock_app
            .wrap()
            .query_wasm_smart::<OwnerOfResponse>(
                addr_collection.to_owned(),
                &CollectionQueryMsg::OwnerOf {
                    token_id: name_alice.to_string(),
                    include_expired: None,
                }
            )
            .expect("Failed to query alice name"),
        OwnerOfResponse {
            owner: owner_addr.to_string(),
            approvals: vec![],
        }
    );
    assert_eq!(
        mock_app
            .wrap()
            .query_wasm_smart::<OwnerOfResponse>(
                addr_collection,
                &CollectionQueryMsg::OwnerOf {
                    token_id: name_bob.to_string(),
                    include_expired: None,
                }
            )
            .expect("Failed to query bob name"),
        OwnerOfResponse {
            owner: owner_addr.to_string(),
            approvals: vec![],
        }
    );
}
