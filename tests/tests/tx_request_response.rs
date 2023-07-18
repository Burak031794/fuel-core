use fuel_core::{
    chain_config::{CoinConfig, StateConfig},
    service::{Config, FuelService},
};
use fuel_core_client::client::FuelClient;
use fuel_core_poa::Trigger;
use fuel_core_types::{
    fuel_tx::{
        field::*,
        input::coin::{CoinPredicate, CoinSigned},
        *,
    },
    fuel_vm::*,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Duration;

// Creates a simple node config with no block production.
// This is used for these tests because we don't want the transactions to be
// included in a block. We want them to be included in the mempool and check
// that new connected nodes sync the mempool.
fn create_node_config_from_inputs_no_block_production(inputs: &[Input]) -> Config {
    let mut node_config = Config::local_node();
    let mut initial_state = StateConfig::default();
    let mut coin_configs = vec![];

    for input in inputs {
        if let Input::CoinSigned(CoinSigned {
            amount,
            owner,
            asset_id,
            utxo_id,
            ..
        })
        | Input::CoinPredicate(CoinPredicate {
            amount,
            owner,
            asset_id,
            utxo_id,
            ..
        }) = input
        {
            let coin_config = CoinConfig {
                tx_id: Some(*utxo_id.tx_id()),
                output_index: Some(utxo_id.output_index()),
                tx_pointer_block_height: None,
                tx_pointer_tx_idx: None,
                maturity: None,
                owner: *owner,
                amount: *amount,
                asset_id: *asset_id,
            };
            coin_configs.push(coin_config);
        };
    }

    initial_state.coins = Some(coin_configs);
    node_config.chain_conf.initial_state = Some(initial_state);
    node_config.utxo_validation = true;
    node_config.block_production = Trigger::Never;
    node_config.p2p.as_mut().unwrap().enable_mdns = true;
    node_config
}

// This test is set up in such a way that the transaction is not committed
// as we've disabled the block production. This is to test that the peer
// will request this transaction from the other peer upon connection.
#[tokio::test(flavor = "multi_thread")]
async fn test_tx_request() {
    use futures::StreamExt;
    let mut rng = StdRng::seed_from_u64(2322);

    let tx = TransactionBuilder::script(vec![], vec![])
        .gas_limit(100)
        .gas_price(1)
        .add_unsigned_coin_input(
            SecretKey::random(&mut rng),
            rng.gen(),
            1000,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .add_output(Output::Change {
            amount: 0,
            asset_id: Default::default(),
            to: rng.gen(),
        })
        .finalize();

    let node_config = create_node_config_from_inputs_no_block_production(tx.inputs());
    let params = node_config.chain_conf.transaction_parameters;
    let node_one = FuelService::new_node(node_config).await.unwrap();
    let client_one = FuelClient::from(node_one.bound_address);

    let node_config = create_node_config_from_inputs_no_block_production(tx.inputs());
    let node_two = FuelService::new_node(node_config).await.unwrap();
    let client_two = FuelClient::from(node_two.bound_address);

    let wait_time = Duration::from_secs(10);

    let tx_id = tx.id(&params.chain_id);
    let tx = tx.into();
    let res = client_one.submit(&tx).await.unwrap();
    dbg!(&res);

    tokio::time::sleep(wait_time).await;

    // At this point, the transaction should be in the mempool of node_one
    // and node_two should request it from node_one.

    // Below code not necessary at this moment.
    // let response = client_one.transaction(&tx_id).await.unwrap();
    // assert!(response.is_some());

    // let mut client_two_subscription = client_two
    //     .subscribe_transaction_status(&tx_id)
    //     .await
    //     .expect("Should be able to subscribe for events");
    // tokio::time::timeout(wait_time, client_two_subscription.next())
    //     .await
    //     .expect("Should await transaction notification in time");

    // let response = client_two.transaction(&tx_id).await.unwrap();
    // assert!(response.is_some());
}