use near_sdk::{json_types::U128, AccountId, NearToken};
use near_workspaces::{Account, Contract};
use serde_json::json;

const ACCESS_CONTROL_CONTRACT: &[u8] =
    include_bytes!("../../res/access_control.wasm");

const EXPLOIT_CONTRACT: &[u8] = include_bytes!("../../res/exploit.wasm");

struct Env {
    owner: Account,
    malicious_actor: Account,
    access_control_contract: Contract,
    exploit_contract: Contract,
    w_near: Contract,
}

async fn prepare() -> color_eyre::Result<Env> {
    let sandbox = near_workspaces::sandbox().await?;
    let mainnet = near_workspaces::mainnet().await?;

    let w_near = sandbox
        .import_contract(&"wrap.near".parse()?, &mainnet)
        .transact()
        .await?;
    let owner = sandbox.dev_create_account().await?;
    let malicious_actor = sandbox.dev_create_account().await?;
    let access_control_contract =
        sandbox.dev_deploy(&ACCESS_CONTROL_CONTRACT).await?;

    println!(
        "ACCESS_CONTROL_CONTRACT_DEPLOYED: {}\n",
        access_control_contract.id()
    );

    let exploit_contract = sandbox.dev_deploy(&EXPLOIT_CONTRACT).await?;

    println!("EXPLOIT_CONTRACT_DEPLOYED: {}\n", exploit_contract.id());

    access_control_contract
        .call("init")
        .args_json(json!({"owner": owner.id(), "w_near_contract": w_near.id()}))
        .transact()
        .await?
        .into_result()?;

    owner
        .call(w_near.id(), "new")
        .transact()
        .await?
        .into_result()?;

    println!("w_near contract deployed: {}\n", w_near.id());

    owner
        .call(w_near.id(), "near_deposit")
        .deposit(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;

    malicious_actor
        .call(w_near.id(), "near_deposit")
        .deposit(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;

    exploit_contract
        .as_account()
        .call(w_near.id(), "near_deposit")
        .deposit(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;

    access_control_contract
        .as_account()
        .call(w_near.id(), "near_deposit")
        .deposit(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;

    Ok(Env {
        owner,
        malicious_actor,
        access_control_contract,
        exploit_contract,
        w_near,
    })
}

#[tokio::test]
async fn pausable_access_control() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        access_control_contract,
        ..
    } = prepare().await?;

    let data = malicious_actor
        .view(access_control_contract.id(), "get_data")
        .args_json(json!({}))
        .await?
        .json::<String>()?;

    assert_eq!(data, "Hello World!");

    malicious_actor
        .call(access_control_contract.id(), "toggle_pause")
        .args_json(json!({}))
        .transact()
        .await?
        .into_result()?;

    malicious_actor
        .view(access_control_contract.id(), "get_data")
        .args_json(json!({}))
        .await
        .expect_err("Function is paused");

    Ok(())
}

#[tokio::test]
async fn pausable_exploit_signer() -> color_eyre::Result<()> {
    let Env {
        owner,
        malicious_actor,
        access_control_contract,
        exploit_contract,
        ..
    } = prepare().await?;

    let data = malicious_actor
        .view(access_control_contract.id(), "get_owner")
        .args_json(json!({}))
        .await?
        .json::<AccountId>()?;

    assert_eq!(&data, owner.id());

    owner
        .call(exploit_contract.id(), "exploit_signer")
        .args_json(json!({
            "target": access_control_contract.id(),
            "owner": malicious_actor.id(),
        }))
        .transact()
        .await?
        .into_result()?;

    let data = malicious_actor
        .view(access_control_contract.id(), "get_owner")
        .args_json(json!({}))
        .await?
        .json::<AccountId>()?;

    assert_eq!(&data, malicious_actor.id());

    Ok(())
}

#[tokio::test]
async fn public_callback_exploit() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        access_control_contract,
        exploit_contract,
        ..
    } = prepare().await?;

    let data = malicious_actor
        .view(access_control_contract.id(), "get_user_points")
        .args_json(json!({"account_id": malicious_actor.id()}))
        .await?
        .json::<U128>()?;

    assert_eq!(data, U128(0));

    malicious_actor
        .call(access_control_contract.id(), "resolve_withdraw")
        .args_json(json!({
            "account_id": malicious_actor.id(),
            "amount": 10000,
        }))
        .transact()
        .await?
        .into_result()
        .expect_err(
            "Smart contract panicked: Contract expected a result on the \
             callback",
        );

    let res = malicious_actor
        .call(exploit_contract.id(), "exploit_public_callback")
        .args_json(json!({
            "target": access_control_contract.id(),
            "account_id": malicious_actor.id(),
            "amount": 10000,
        }))
        .transact()
        .await?
        .into_result()?;

    println!("RES: {res:#?}");

    let data = malicious_actor
        .view(access_control_contract.id(), "get_user_points")
        .args_json(json!({"account_id": malicious_actor.id()}))
        .await?
        .json::<U128>()?;

    assert_eq!(data.0, 10000);

    Ok(())
}
