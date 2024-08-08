#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod utils;
use crate::utils::*;


use ethers::{
    abi::Abi,
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256, TransactionRequest, Bytes as EthersBytes},
};

use std::str::FromStr;
use std::sync::Arc;
use std::fs;

use uniswap_v3_sdk::prelude::*;
use uniswap_v3_sdk::constants::FeeAmount;
use uniswap_v3_sdk::utils::compute_pool_address::compute_pool_address;
use uniswap_v3_sdk::entities::pool::Pool as PoolSDK;
use uniswap_v3_sdk::entities::position::Position;
use uniswap_v3_sdk::utils::nearest_usable_tick::nearest_usable_tick;
use uniswap_v3_sdk::nonfungible_position_manager::{add_call_parameters, AddLiquiditySpecificOptions, AddLiquidityOptions, RemoveLiquidityOptions, MintSpecificOptions}; 


use uniswap_sdk_core::{entities::token::Token, prelude::FractionLike};
use uniswap_sdk_core::entities::fractions::percent::Percent;
use uniswap_sdk_core::entities::token::TokenMeta;

use serde_json::{to_string_pretty};

use alloy_ethers_typecast::{ethers_u256_to_alloy};
use alloy_primitives::Uint;
use alloy_primitives::U256 as AlloyU256;

use chrono::Utc;
use num_integer::Integer;

use uniswap_v3_sdk::utils::price_tick_conversions::tick_to_price;
use ethers::utils::{parse_units, format_units};

use uniswap_sdk_core::entities::fractions::percent::IsPercent;




const RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";




fn calculate_amount(tokens: f64, decimals: u32) -> U256 {
    let decimal_adjustment = 10_u64.pow(decimals); 
    U256::from(tokens as u64) * U256::from(decimal_adjustment) 
}



pub fn amount_to_int(tokens: String, decimals: u32) -> U256 {
    parse_units(tokens, decimals).unwrap().into()
}




pub fn nearest_usable_tick_left(tick: i32, tick_spacing: i32) -> i32 {
    let (quotient, remainder) = tick.div_mod_floor(&tick_spacing);
    let rounded = (quotient) * tick_spacing;
    rounded
}


#[tokio::main]
async fn main() -> eyre::Result<()> {

    
    abigen!(
        Pool,
        r#"[
            function liquidity() external view returns (uint128)
            function slot0() external view returns (uint160 sqrtPriceX96,  int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
            function initialize(uint160 sqrtPriceX96) external override
        ]"#
    );

    abigen!(
        NonfungiblePositionManager,
        r#"[
            function mint(address recipient, int24 tickLower, int24 tickUpper, uint128 amount, bytes calldata data) external returns (uint256 amount0, uint256 amount1)
        ]"#
    );

    // setup rpc, wallet
    let rpc_url = RPC_URL;
    let program_address = PROGRAM_ADDRESS;
    abigen!(
        Erc20,
        r#"[
            function approve(address spender, uint256 amount) external returns (bool)
        ]"#
    );
    abigen!(
        PositionManager,
        r#"[
            function approve(address spender, uint256 amount) external returns (bool)
        ]"#
    );
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let address: Address = program_address.parse()?;

    let privkey = " ".to_string();
    let wallet = LocalWallet::from_str(&privkey)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.clone().with_chain_id(chain_id),
    ));

    



    
    // setup addresses, contracts
    let quote_token_address = TOKEN_2_ADDRESS; // UPD
    let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse()?;
    let token_0_address: Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address: Address = quote_token_address.parse()?;
    let position_manager_address: Address = POSITION_MANAGER_ADDRESS.parse()?;

    let token_0_address_alloy: alloy_primitives::Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address_alloy: alloy_primitives::Address = quote_token_address.parse()?;
    let factory_address_alloy: alloy_primitives::Address = UNI_FACTORY_ADDRESS.parse()?;
    

    let fee = FeeAmount::HIGH;
    let current_pool_address = compute_pool_address(factory_address_alloy, token_0_address_alloy, token_1_address_alloy, fee, None);
    println!("current_pool_address: {}", current_pool_address);

    // contracts

    let token_0_contract = Erc20::new(token_0_address, Arc::clone(&client));
    let token_1_contract = Erc20::new(token_1_address, Arc::clone(&client));
    let pool_contract = Pool::new(alloy_address_to_ethers(current_pool_address), Arc::clone(&client));
    let position_manager_contract = NonfungiblePositionManager::new(position_manager_address, Arc::clone(&client));




    // sdk instances

    let token_0_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_0_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };    
    let token_1_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_1_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };     


    let liquidity = pool_contract.liquidity().call().await?;
    let slot0 = pool_contract.slot_0().call().await?;
    println!("liquidity: {}", liquidity);
    println!("Slot0 values: {:?}", slot0);
    println!("slot0.1: {}", slot0.1);

    let sqrt_price_x96 = slot0.0;



    // configure the pool
    let configured_pool = PoolSDK::new(
        token_0_sdk,
        token_1_sdk, 
        fee,
        ethers_u256_to_alloy(sqrt_price_x96),
        liquidity
    ).expect("Failed to create pool"); // Add proper error handling here

    // ADD LIQUIDITY 
    // let tokens_str = String::from("90.83220773");
    let tokens_str = String::from("135");
    let amount_quote_token = amount_to_int(tokens_str, 18);
    println!("amount_quote_token: {:?}", amount_quote_token);


    // approve tokens
    let pending = token_0_contract.approve(
        position_manager_address,
        amount_quote_token,
    );
    if let Some(receipt) = pending.send().await?.await? {
        println!("Receipt = {:?}", receipt);
    }
    let pending = token_1_contract.approve(
        position_manager_address,
        amount_quote_token,
    );
    if let Some(receipt) = pending.send().await?.await? {
        println!("Receipt = {:?}", receipt);
    }



    
    println!("configured_pool.tick_current: {:?}", configured_pool.tick_current);

    println!("nearest_usable_tick_left(configured_pool.tick_current, configured_pool.tick_spacing()): {:?}", nearest_usable_tick_left(configured_pool.tick_current, configured_pool.tick_spacing()));

    let tick_a = nearest_usable_tick_left(configured_pool.tick_current, configured_pool.tick_spacing());
    let tick_b = nearest_usable_tick_left(configured_pool.tick_current, configured_pool.tick_spacing()) + configured_pool.tick_spacing();


    let token_0_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_0_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };    
    let token_1_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_1_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };     

    let price_a = tick_to_price(token_0_sdk, token_1_sdk, tick_a).unwrap();
    
    let token_0_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_0_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };    
    let token_1_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_1_address_alloy, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };     

    
    
    let price_b = tick_to_price(token_0_sdk, token_1_sdk, tick_b).unwrap();

    // println!("price_a: {:?}", price_a);
    println!("price_a: {}", fraction_to_big_decimal(&price_a));
    println!("price_b: {}", fraction_to_big_decimal(&price_b));


    let mut position = Position::from_amount1(
        configured_pool.clone(),
        tick_a,
        tick_b,
        ethers_u256_to_alloy(amount_quote_token)
    ).unwrap();
    println!("position: {:?}", position);


    



    let alloy_client_address: alloy_primitives::Address = client.address().to_fixed_bytes().into();

    // Add 20 minutes (60 seconds * 20 minutes)
    let timestamp_secs = Utc::now().timestamp(); 
    let deadline = timestamp_secs + 60 * 20;


    // ADD LIQUIDITY ///////////////////
    
    let add_liquidity_options = AddLiquidityOptions {
        slippage_tolerance: Percent::new(50, 10_000),
        deadline: AlloyU256::from(deadline as u64),
        use_native: None,
        token0_permit: None,
        token1_permit: None,
        specific_opts: AddLiquiditySpecificOptions::Mint(MintSpecificOptions {
            recipient: alloy_client_address,
            create_pool: false, // Set to true if you want to create a new pool
        }),
    };

    
    let add_call_parameters_res = add_call_parameters(&mut position, add_liquidity_options);
    let calldata = add_call_parameters_res.unwrap().calldata;




    // Construct the transaction
    let tx = TransactionRequest::new()
    .to(POSITION_MANAGER_ADDRESS)
    .from(client.address())
    .data(EthersBytes::from(calldata.0));

    let pending_tx = client.send_transaction(tx, None).await?;

    // Wait for the transaction to be mined
    let receipt = pending_tx.confirmations(1).await?;
    
    println!("Transaction Receipt: {}", serde_json::to_string_pretty(&receipt)?);





    Ok(())
}

