// old code - add "create pool", uncomment initialization
//Create And Initi...	есть мтеоддва в одном
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod utils;
use crate::utils::*;

use ethers::{
    abi::Abi, contract::Contract, middleware::SignerMiddleware, prelude::abigen, providers::{Http, Middleware, Provider}, signers::{LocalWallet, Signer}, types::{Address, Bytes as EthersBytes, TransactionRequest, U256}
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
use uniswap_v3_sdk::nonfungible_position_manager::{add_call_parameters, AddLiquiditySpecificOptions, AddLiquidityOptions, MintSpecificOptions}; 


use uniswap_sdk_core::entities::token::Token;
use uniswap_sdk_core::entities::fractions::percent::Percent;
use uniswap_sdk_core::entities::token::TokenMeta;

use serde_json;

// use alloy_ethers_typecast::{ethers_address_to_alloy, ethers_u256_to_alloy};
use alloy_primitives::Uint;
use alloy_primitives::U256 as AlloyU256;

use chrono::Utc;


use uniswap_v3_sdk::extensions::price_to_sqrt_ratio_x96;
use uniswap_v3_sdk::extensions::sqrt_ratio_x96_to_price;
use rust_decimal::Decimal;
use uniswap_sdk_core::prelude::BigDecimal;


use uniswap_v3_sdk::extensions::fraction_to_big_decimal;


use alloy_ethers_typecast::alloy_u256_to_ethers;
use ethers::core::utils::to_checksum;





fn calculate_amount(tokens: f64, decimals: u32) -> U256 {
    let decimal_adjustment = 10_u64.pow(decimals); 
    U256::from(tokens as u64) * U256::from(decimal_adjustment) 
}


fn read_abi_from_file(file_path: &str) -> String {
    let abi_str = fs::read_to_string(file_path).expect("Failed to read ABI file");
    abi_str
}

#[tokio::main]
async fn main() -> eyre::Result<()> {

    abigen!(
        Pool,
        r#"[
            function liquidity() external view returns (uint128)
            function slot0() external view returns (uint160 sqrtPriceX96,  int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
            function initialize(uint160 sqrtPriceX96) external override
            function token0() external view returns (address)
            function token1() external view returns (address)
            function fee() external view returns (uint24) 
        ]"#
    );

    abigen!(
        NonfungiblePositionManager,
        r#"[
            function mint(address recipient, int24 tickLower, int24 tickUpper, uint128 amount, bytes calldata data) external returns (uint256 amount0, uint256 amount1)
            function createAndInitializePoolIfNecessary(address token0, address token1, uint24 fee, uint160 sqrtPriceX96) external payable returns (address pool)
            function approve(address spender, uint256 amount) external returns (bool)
        ]"#
    );


    abigen!(
        Erc20,
        r#"[
            function approve(address spender, uint256 amount) external returns (bool)
        ]"#
    );

    // setup rpc, wallet
    let rpc_url = RPC_URL;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let privkey = read_secret_from_file(&PRIV_KEY_PATH)?;
    let wallet = LocalWallet::from_str(&privkey)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.clone().with_chain_id(chain_id),
    ));

    
    // setup addresses
    let quote_token_address = TOKEN_2_ADDRESS;
    let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse()?;
    let token_0_address: Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address: Address = quote_token_address.parse()?;
    let position_manager_address: Address = POSITION_MANAGER_ADDRESS.parse()?;


    // contracts
    let token_0_contract = Erc20::new(token_0_address, Arc::clone(&client));
    let token_1_contract = Erc20::new(token_1_address, Arc::clone(&client));
    let position_manager_contract = NonfungiblePositionManager::new(position_manager_address, Arc::clone(&client));


    // sdk instances
    let token_0_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: TOKEN_0_ADDRESS.parse()?, 
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
            address: quote_token_address.parse()?, 
            buy_fee_bps: None,  
            sell_fee_bps: None   
        }
    };     



    // create and initialize pool
    // calc prices
    let price = BigDecimal::from_str("5").unwrap();   
    let sqrt_ratio_x96 = price_to_sqrt_ratio_x96(&price);
    println!("sqrt_ratio_x96: {:?}", sqrt_ratio_x96);
    let init_price = sqrt_ratio_x96_to_price(sqrt_ratio_x96, token_0_sdk, token_1_sdk);
    println!("init_price: {:?}", init_price);
    let init_price_decimal = fraction_to_big_decimal(&init_price.unwrap());
    println!("init_price_decimal: {:?}", init_price_decimal);

    let fee: u32 = FeeAmount::HIGH as u32;
    
    // create pool
    let pending = position_manager_contract.create_and_initialize_pool_if_necessary(
        token_0_address, 
        token_1_address,
        fee,
        alloy_u256_to_ethers(sqrt_ratio_x96)
    );

    if let Some(receipt) = pending.send().await?.await? {
        println!("Receipt = {:?}", receipt);
    }


    Ok(())
}

