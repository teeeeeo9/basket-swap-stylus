#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod utils;
use crate::utils::*;

use ethers::{
    abi::Abi, addressbook::Contract, middleware::SignerMiddleware, prelude::abigen, providers::{Http, Middleware, Provider}, signers::{LocalWallet, Signer}, types::{Address, Bytes as EthersBytes, TransactionRequest, U256}
};
use utils::ethers_address_to_alloy;

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
// use uniswap_v3_sdk::abi::IQuoterV2;


use uniswap_sdk_core::{entities::token::Token, prelude::{FractionBase, Integer}};
use uniswap_sdk_core::entities::fractions::percent::Percent;
use uniswap_sdk_core::entities::token::TokenMeta;

use serde_json::{self, de};

use alloy_ethers_typecast::{ethers_u256_to_alloy};
use alloy_primitives::{Uint, address};
use alloy_primitives::U256 as AlloyU256;

use chrono::Utc;

// use eei::ethereum_callStatic;


use uniswap_v3_sdk::quoter::quote_call_parameters;
use uniswap_sdk_core::prelude::TradeType;


use uniswap_sdk_core::prelude::CurrencyAmount;

use uniswap_v3_sdk::extensions::EphemeralTickDataProvider;
use ethers::prelude::{BlockId, ContractError};

use rust_decimal::Decimal;
use uniswap_sdk_core::prelude::ToPrimitive;

use uniswap_v3_sdk::extensions::get_liquidity_array_for_pool;



use uniswap_v3_sdk::extensions::price_to_sqrt_ratio_x96;
use uniswap_v3_sdk::extensions::sqrt_ratio_x96_to_price;

use uniswap_v3_sdk::extensions::get_all_positions_by_owner;

use serde_json::{json, to_string_pretty};



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
        ]"#
    );

    abigen!(
        Quoter,
        r#"[
            function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn,uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
        ]"#
    );

    abigen!(
        TickLens,
        r#"[
            function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn,uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
        ]"#
    );


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


    // setup addresses, contracts
    let quote_token_address = TOKEN_2_ADDRESS; // UPD
    let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse()?;
    let token_0_address: Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address: Address = quote_token_address.parse()?;
    let position_manager_address: Address = POSITION_MANAGER_ADDRESS.parse()?;
    let quoter_address: Address = QUOTER_ADDRESS.parse()?;

    let token_0_address_alloy: alloy_primitives::Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address_alloy: alloy_primitives::Address = quote_token_address.parse()?;
    let factory_address_alloy: alloy_primitives::Address = UNI_FACTORY_ADDRESS.parse()?;
    let position_manager_address_alloy: alloy_primitives::Address = POSITION_MANAGER_ADDRESS.parse()?;
    
    let fee = FeeAmount::HIGH;
    let current_pool_address = compute_pool_address(factory_address_alloy, token_0_address_alloy, token_1_address_alloy, fee, None);
    println!("current_pool_address: {}", current_pool_address);

    // contracts
    let token_0_contract = Erc20::new(token_0_address, Arc::clone(&client));
    let token_1_contract = Erc20::new(token_1_address, Arc::clone(&client));
    let pool_contract = Pool::new(alloy_address_to_ethers(current_pool_address), Arc::clone(&client));
    let position_manager_contract = NonfungiblePositionManager::new(position_manager_address, Arc::clone(&client));
    let quoter_contract = Quoter::new(quoter_address, Arc::clone(&client));



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
    
    pub struct TokenConfig {
        pub token_in: Token,
        pub amount_in: f64,  
        pub token_out: Token,
        pub pool_fee: u32, 
    }



    


    // let quoted_amount_out = quoter_contract.callStatic.quote_exact_input_single(
        
    // ).call().await?;

    let liquidity = pool_contract.liquidity().call().await?;
    let slot0 = pool_contract.slot_0().call().await?;
    let sqrt_price_x96 = slot0.0;
    let tick_current = slot0.1;





    let configured_pool = PoolSDK::new(
        token_0_sdk.clone(),
        token_1_sdk.clone(), 
        fee,
        ethers_u256_to_alloy(sqrt_price_x96),
        liquidity
    ).expect("Failed to create pool"); 

    let token0_res = pool_contract.token_0().call().await?;
    let token1_res = pool_contract.token_1().call().await?;
    let fee_res = pool_contract.fee().call().await?;
    println!("liquidity: {}", liquidity);
    println!("token0_res: {}", token0_res);
    println!("token1_res: {}", token1_res);
    println!("fee_res: {}", fee_res);
    println!("Slot0 values: {:?}", slot0);

    let sqrt_price_x96 = slot0.0;
    println!("sqrt_price_x96: {}", sqrt_price_x96);
    println!("tick: {}", slot0.1);
    println!("observationIndex: {}", slot0.2);
    println!("observationCardinality: {}", slot0.3);
    println!("observationCardinalityNext: {}", slot0.4);
    println!("feeProtocol: {}", slot0.5);
    println!("unlocked: {}", slot0.6);

    let current_price = configured_pool.price_of(&token_0_sdk);
    println!("current_price: {}", fraction_to_big_decimal(&current_price));




    let owner_positions  = get_all_positions_by_owner(
        position_manager_address_alloy, 
        wallet.address().to_fixed_bytes().into(), // etheres address to alloy
        Arc::clone(&client),
        None
    ).await?;

    println!("owner_positions: ");
    println!("{}", to_string_pretty(&owner_positions).unwrap());






    Ok(())
}

