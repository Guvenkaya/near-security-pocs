use near_sdk::{json_types::U128, near, AccountId, NearToken};
use near_workspaces::{
    operations::Function, types::SecretKey, Account, Contract,
};
use serde_json::json;

const STORAGE_COLLISIONS_CONTRACT: &[u8] =
    include_bytes!("../../res/storage_key_collisions.wasm");

#[near(serializers = [borsh, json])]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MoneyJar {
    pub amount: U128,
    pub id: String,
}

struct Env {
    owner: Account,
    malicious_actor: Account,
    malicious_actor2: Account,
    storage_collisions_contract: Contract,
}

async fn prepare() -> color_eyre::Result<Env> {
    let sandbox = near_workspaces::sandbox().await?;

    let owner = sandbox.dev_create_account().await?;
    let malicious_actor = sandbox
        .create_tla(
            "account_id".parse().unwrap(),
            SecretKey::from_random(near_workspaces::types::KeyType::ED25519),
        )
        .await?
        .into_result()?;
    let malicious_actor2 = sandbox
        .create_tla(
            "1account_id".parse().unwrap(),
            SecretKey::from_random(near_workspaces::types::KeyType::ED25519),
        )
        .await?
        .into_result()?;

    let storage_collisions_contract =
        sandbox.dev_deploy(&STORAGE_COLLISIONS_CONTRACT).await?;

    println!(
        "STORAGE_KEY_COLLISION_CONTRACT: {}\n",
        storage_collisions_contract.id()
    );

    storage_collisions_contract
        .call("new")
        .args_json(json!({"managers": vec![owner.id()]}))
        .transact()
        .await?
        .into_result()?;

    Ok(Env {
        owner,
        malicious_actor,
        malicious_actor2,
        storage_collisions_contract,
    })
}

#[tokio::test]
async fn storage_key_collision() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        malicious_actor2,
        storage_collisions_contract,
        ..
    } = prepare().await?;

    malicious_actor
        .call(storage_collisions_contract.id(), "create_jar")
        .args_json(json!({"amount": U128(NearToken::from_near(2).as_yoctonear()), "id": "11"}))
        .transact().await?.into_result()?;

    malicious_actor2
        .call(storage_collisions_contract.id(), "create_jar")
        .args_json(json!({"amount": U128(NearToken::from_near(6).as_yoctonear()), "id": "1"}))
        .transact().await?.into_result()?;

    let jar_1 = storage_collisions_contract
        .view("get_jars")
        .args_json(json!({"account_id": malicious_actor.id()}))
        .await?
        .json::<Vec<MoneyJar>>()?;

    let jar_2 = storage_collisions_contract
        .view("get_jars")
        .args_json(json!({"account_id": malicious_actor2.id()}))
        .await?
        .json::<Vec<MoneyJar>>()?;

    println!("JAR1: {jar_1:#?}");

    println!("JAR2: {jar_2:#?}");

    assert_eq!(jar_1.len(), 1);
    assert_eq!(jar_2.len(), 1);

    assert_eq!(
        jar_1[0].amount,
        U128(NearToken::from_near(6).as_yoctonear())
    );
    assert_eq!(
        jar_2[0].amount,
        U128(NearToken::from_near(6).as_yoctonear())
    );

    Ok(())
}

#[tokio::test]
async fn storage_key_collision_timestamp() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        storage_collisions_contract,
        ..
    } = prepare().await?;

    malicious_actor
        .batch(storage_collisions_contract.id())
        .call(Function::new("create_jar_timestamp").args_json(json!(
            {"amount": U128(NearToken::from_near(2).as_yoctonear()), "id": "11"}
        )))
        .call(Function::new("create_jar_timestamp").args_json(json!(
            {"amount": U128(NearToken::from_near(7).as_yoctonear()), "id": "1"}
        )))
        .transact()
        .await?
        .into_result()?;

    let jar_1 = storage_collisions_contract
        .view("get_jars")
        .args_json(json!({"account_id": malicious_actor.id()}))
        .await?
        .json::<Vec<MoneyJar>>()?;

    println!("JAR1: {jar_1:#?}");

    assert_eq!(jar_1.len(), 2);

    Ok(())
}
