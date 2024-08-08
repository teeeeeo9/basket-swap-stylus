#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

mod utils;
use crate::utils::*;

use ethers::{
    abi::Abi, addressbook::Contract, middleware::SignerMiddleware, prelude::abigen, providers::{Http, Middleware, Provider}, signers::{LocalWallet, Signer}, types::{Address, Bytes as EthersBytes, TransactionRequest, U256}, utils::format_units
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
// use uniswap_v3_sdk::abi::IQuoterV2;


use uniswap_sdk_core::{entities::{fractions::{currency_amount::CurrencyMeta, price::PriceMeta}, token::Token}, prelude::{CurrencyLike, FractionBase}};
use uniswap_sdk_core::entities::fractions::percent::Percent;
use uniswap_sdk_core::entities::token::TokenMeta;


use serde_json;

use alloy_ethers_typecast::{ethers_u256_to_alloy};
use alloy_primitives::{Uint, address};
use alloy_primitives::U256 as AlloyU256;

use chrono::Utc;



use uniswap_v3_sdk::quoter::quote_call_parameters;
use uniswap_sdk_core::prelude::TradeType;


use uniswap_sdk_core::prelude::CurrencyAmount;

use uniswap_v3_sdk::extensions::EphemeralTickDataProvider;
use ethers::prelude::{BlockId, ContractError};
use uniswap_sdk_core::prelude::FractionLike;

use uniswap_sdk_core::prelude::ToPrimitive;






/// Stylus RPC endpoint url.
const RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";



fn calculate_amount(tokens: f64, decimals: u32) -> U256 {
    let decimal_adjustment = 10_u64.pow(decimals); 
    U256::from(tokens as u64) * U256::from(decimal_adjustment) 
}


fn read_abi_from_file(file_path: &str) -> Abi {
    let abi_str = fs::read_to_string(file_path).expect("Failed to read ABI file");
    serde_json::from_str(&abi_str).expect("Failed to parse ABI")
}

fn get_current_price(current_price: &FractionLike<CurrencyMeta<CurrencyLike<TokenMeta>>>) -> f64  {
    let numerator = current_price.numerator().to_f64().unwrap();
    let denominator = current_price.denominator().to_f64().unwrap();
    let current_price = numerator / denominator;
    current_price
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
            function balanceOf(address account) external view returns (uint256)

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
    let program_address = PROGRAM_ADDRESS;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let address: Address = program_address.parse()?;

    let privkey = " ".to_string();
    let wallet = LocalWallet::from_str(&privkey)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    println!("chain_id: {}", chain_id);
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.clone().with_chain_id(chain_id),
    ));

    



    
    // setup addresses, contracts
    let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse()?;
    let token_0_address: Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address: Address = TOKEN_1_ADDRESS.parse()?;
    // let pool_address: Address  = UNI_POOL_ADDRESS.parse()?;
    let position_manager_address: Address = POSITION_MANAGER_ADDRESS.parse()?;
    let quoter_address: Address = QUOTER_ADDRESS.parse()?;

    let token_0_address_alloy: alloy_primitives::Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address_alloy: alloy_primitives::Address = TOKEN_1_ADDRESS.parse()?;
    let factory_address_alloy: alloy_primitives::Address = UNI_FACTORY_ADDRESS.parse()?;
    // let pool_address_alloy: alloy_primitives::Address = UNI_POOL_ADDRESS.parse()?;

    let fee = FeeAmount::MEDIUM;
    let current_pool_address = compute_pool_address(factory_address_alloy, token_0_address_alloy, token_1_address_alloy, fee, None);
    println!("current_pool_address: {}", current_pool_address);




    // user accounts
    let account_address_lp: Address = " ".parse().unwrap();
    let account_address_trader: Address = " ".parse().unwrap();
    


    // contracts

    let token_0_contract = Erc20::new(token_0_address, Arc::clone(&client));
    let token_1_contract = Erc20::new(token_1_address, Arc::clone(&client));
    let pool_contract = Pool::new(alloy_address_to_ethers(current_pool_address), Arc::clone(&client));  
    let position_manager_contract = NonfungiblePositionManager::new(position_manager_address, Arc::clone(&client));
    let quoter_contract = Quoter::new(quoter_address, Arc::clone(&client));



    let token_0_balance = token_0_contract.balance_of(account_address_lp).call().await.unwrap();
    println!(
        "0 token balance for {}: {}",
        account_address_lp,
        format_units(token_0_balance, 18).unwrap()
    );

    let token_1_balance = token_1_contract.balance_of(account_address_lp).call().await.unwrap();
    println!(
        "1 token balance for {}: {}",
        account_address_lp,
        format_units(token_1_balance, 18).unwrap()
    );



    Ok(())
}

