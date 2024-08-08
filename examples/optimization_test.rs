#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]



// use argmin::solver::powell::Powell;
use stylus_sdk::call::{call, static_call, Call, StaticCallContext};
use stylus_sdk::crypto::keccak;
use stylus_sdk::storage;

mod utils;
use crate::utils::*;

// use aperture_lens::bindings::pool_address;
use argmin::{
    core::{
        observers::ObserverMode, CostFunction, Error, Executor, Gradient, IterState, Problem, Solver, TerminationReason, TerminationStatus
    },
    solver::simulatedannealing::{Anneal, SATempFunc, SimulatedAnnealing},
};

// use argmin_observer_slog::SlogLogger;
use ethers::{
    addressbook::Contract, middleware::SignerMiddleware, prelude::abigen, providers::{Http, Middleware, Provider}, signers::{LocalWallet, Signer}, types::{Address, Bytes as EthersBytes, TransactionRequest, U256}
};
use rust_decimal::Decimal;

use std::{str::FromStr, sync::Mutex};
use std::sync::Arc;
use std::fs;

use uniswap_v3_sdk::{constants::FeeAmount, entities::{Route, TickDataProvider, TickTrait, Trade}, extensions::fraction_to_big_decimal, swap_router::SwapOptions};
use uniswap_v3_sdk::utils::compute_pool_address::compute_pool_address;
use uniswap_v3_sdk::entities::pool::Pool as PoolSDK;
use uniswap_v3_sdk::entities::position::Position;
use uniswap_v3_sdk::utils::nearest_usable_tick::nearest_usable_tick;
use uniswap_v3_sdk::nonfungible_position_manager::{add_call_parameters, AddLiquiditySpecificOptions, AddLiquidityOptions, MintSpecificOptions}; 
use uniswap_v3_sdk::extensions::EphemeralTickDataProvider;
use uniswap_v3_sdk::quoter::quote_call_parameters;



use uniswap_sdk_core::{entities::{fractions::{currency_amount::CurrencyMeta, price::PriceMeta}, token::Token}, prelude::{BigDecimal, CurrencyLike, FractionBase}};
use uniswap_sdk_core::entities::fractions::percent::Percent;
use uniswap_sdk_core::entities::token::TokenMeta;
use uniswap_sdk_core::prelude::TradeType;

use uniswap_sdk_core::prelude::CurrencyAmount;
use uniswap_sdk_core::prelude::FractionLike;
use uniswap_sdk_core::prelude::ToPrimitive;




use serde_json;

use alloy_ethers_typecast::{ethers_u256_to_alloy};
use alloy_primitives::{Uint, address};
use alloy_primitives::U256 as AlloyU256;
use alloy_primitives_old::Address as AddressOld;

use chrono::Utc;

use ethers::prelude::{BlockId, ContractError};









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
    let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse()?;
    let token_0_address: Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address: Address = TOKEN_1_ADDRESS.parse()?;
    let token_2_address: Address = TOKEN_2_ADDRESS.parse()?;
    // let pool_address: Address  = UNI_POOL_ADDRESS.parse()?;
    let position_manager_address: Address = POSITION_MANAGER_ADDRESS.parse()?;
    let quoter_address: Address = QUOTER_ADDRESS.parse()?;

    let token_0_address_alloy: alloy_primitives::Address = TOKEN_0_ADDRESS.parse()?;
    let token_1_address_alloy: alloy_primitives::Address = TOKEN_1_ADDRESS.parse()?;
    let token_2_address_alloy: alloy_primitives::Address = TOKEN_2_ADDRESS.parse()?;
    let factory_address_alloy: alloy_primitives::Address = UNI_FACTORY_ADDRESS.parse()?;
    let quoter_address_alloy: alloy_primitives::Address = QUOTER_ADDRESS.parse()?;
    // let pool_address_alloy: alloy_primitives::Address = UNI_POOL_ADDRESS.parse()?;
    

    let fee = FeeAmount::MEDIUM;
    let current_pool_1_address = compute_pool_address(factory_address_alloy, token_0_address_alloy, token_1_address_alloy, fee, None);
    let current_pool_2_address = compute_pool_address(factory_address_alloy, token_0_address_alloy, token_2_address_alloy, fee, None);
    // println!("current_pool_address: {}", current_pool_address);


    // contracts
    let token_0_contract = Erc20::new(token_0_address, Arc::clone(&client));
    let token_1_contract = Erc20::new(token_1_address, Arc::clone(&client));
    let pool_2_contract = Pool::new(alloy_address_to_ethers(current_pool_2_address), Arc::clone(&client));
    let pool_1_contract = Pool::new(alloy_address_to_ethers(current_pool_1_address), Arc::clone(&client));    
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
    let token_2_sdk = Token {
        chain_id: 421614,
        decimals: 18,
        symbol: None, 
        name: None,
        meta: TokenMeta { 
            address: token_2_address_alloy, 
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


    // configure pool 1

    let liquidity = pool_1_contract.liquidity().call().await?;
    let slot0 = pool_1_contract.slot_0().call().await?;
    let sqrt_price_x96_1 = slot0.0;
    let tick_current = slot0.1;
    let block_number = client.get_block_number().await?;


    let ticks_provider = EphemeralTickDataProvider::new(
        current_pool_1_address,
        Arc::clone(&client),
        None,
        None,
        Some(BlockId::from(block_number)),
    )
    .await?;
    // let a = ticks_provider.get_tick(16140);
    // println!("a: {:?}", a);


    let configured_pool_1 = PoolSDK::new_with_tick_data_provider(
        token_0_sdk.clone(),
        token_1_sdk.clone(), 
        fee,
        ethers_u256_to_alloy(sqrt_price_x96_1),
        liquidity,
        ticks_provider
    ).expect("Failed to create pool"); 

    // configure pool 2
    let liquidity = pool_2_contract.liquidity().call().await?;
    let slot0 = pool_2_contract.slot_0().call().await?;
    let sqrt_price_x96_2 = slot0.0;
    let tick_current = slot0.1;
    let block_number = client.get_block_number().await?;


    let ticks_provider_2 = EphemeralTickDataProvider::new(
        current_pool_2_address,
        Arc::clone(&client),
        None,
        None,
        Some(BlockId::from(block_number)),
    )
    .await?;
    // let a = ticks_provider.get_tick(16140);
    // println!("a: {:?}", a);


    let configured_pool_2 = PoolSDK::new_with_tick_data_provider(
        token_0_sdk.clone(),
        token_2_sdk.clone(), 
        fee,
        ethers_u256_to_alloy(sqrt_price_x96_2),
        liquidity,
        ticks_provider_2
    ).expect("Failed to create pool"); 



    // quote
    // pool 1
    let amount_human_1: u64 = calculate_amount_u64(10.0, 18); // UPD
    let amount_in_1 = CurrencyAmount::from_raw_amount(token_1_sdk.clone(), amount_human_1).unwrap(); // UPD
    println!("amount0: {:?}", amount_in_1);
    let (amount_out_1, _) = configured_pool_1.get_output_amount(
        &amount_in_1, None
    ).unwrap();
    let amount_out_readable_1 = fraction_to_big_decimal(&amount_out_1) / 1e18; 
    println!("amount_out: {:?}", amount_out_1);
    println!("amount_out: {:?}", amount_out_readable_1);


    // pool 2
    let amount_human_2: u64 = calculate_amount_u64(10.0, 18); // UPD
    let amount_in_2 = CurrencyAmount::from_raw_amount(token_2_sdk.clone(), amount_human_2).unwrap(); // UPD
    println!("amount0: {:?}", amount_in_2);
    let (amount_out_2, _) = configured_pool_2.get_output_amount(
        &amount_in_2, None
    ).unwrap();
    let amount_out_readable_2 = fraction_to_big_decimal(&amount_out_2) / 1e18; 
    println!("amount_out: {:?}", amount_out_2);
    println!("amount_out: {:?}", amount_out_readable_2);




    // optimization - estimate outputs

    // Define the objective function to maximize
    fn objective_function<P, T>(q1: f64, q: f64, pool1: &PoolSDK<P>, pool2: &PoolSDK<P>, token_1_sdk: &Token, token_2_sdk: &Token) -> f64 
    where
    T: TickTrait,
    P: TickDataProvider<Tick = T>
    {
        let q2 = 1.0 - q1; 

        if q1 < 0.0 || q2 < 0.0 {
            return f64::NEG_INFINITY; // Return negative infinity for invalid inputs
        }

        let amount_in_1 = CurrencyAmount::from_raw_amount(token_1_sdk.clone(), (q1 * q * 10f64.powi(18)).round() as u128).unwrap();
        let amount_in_2 = CurrencyAmount::from_raw_amount(token_2_sdk.clone(), (q2 * q * 10f64.powi(18)).round() as u128).unwrap();

        let (amount_out_1, _) = pool1.get_output_amount(&amount_in_1, None).unwrap();
        let (amount_out_2, _) = pool2.get_output_amount(&amount_in_2, None).unwrap();

        let value_1:BigDecimal = fraction_to_big_decimal(&amount_out_1)/ 1e18;
        let value_2:BigDecimal = fraction_to_big_decimal(&amount_out_2)/ 1e18;
        value_1.to_f64().unwrap() + value_2.to_f64().unwrap() 
    }


    struct UniV3Optimizer<P, T>
    where
        T: TickTrait,
        P: TickDataProvider<Tick = T>,
    {
        pool1: PoolSDK<P>,
        pool2: PoolSDK<P>,  
        token_1_sdk: Token,
        token_2_sdk: Token,
        q: f64,
        /// lower bound
        lower_bound: Vec<f64>,
        /// upper bound
        upper_bound: Vec<f64>,
        rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    }

    impl<P, T>   UniV3Optimizer<P, T>
    where
        T: TickTrait,
        P: TickDataProvider<Tick = T>,{
        /// Constructor
        pub fn new(
            pool1: PoolSDK<P>,
            pool2: PoolSDK<P>,  
            token_1_sdk: Token,
            token_2_sdk: Token,
            q: f64,
            lower_bound: Vec<f64>,
            upper_bound: Vec<f64>,)
-> Self {
            UniV3Optimizer {
                pool1,
                pool2,
                token_1_sdk, 
                token_2_sdk, 
                q, 
                lower_bound,
                upper_bound,   
                rng: Arc::new(Mutex::new(Xoshiro256PlusPlus::from_entropy()))
            }
        }
    }

    impl<P, T> CostFunction for UniV3Optimizer<P, T> 
    where
    T: TickTrait,
    P: TickDataProvider<Tick = T>,
    {
        type Param = Vec<f64>;
        type Output = f64;
    
        fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
            Ok(-objective_function(param[0], self.q, &self.pool1, &self.pool2, &self.token_1_sdk, &self.token_2_sdk))
        }
    }

    impl<P, T> Anneal for UniV3Optimizer<P, T> 
    where
    T: TickTrait,
    P: TickDataProvider<Tick = T>,    
    {
        type Param = Vec<f64>;
        type Output = Vec<f64>;
        type Float = f64;
    
        /// Anneal a parameter vector
        fn anneal(&self, param: &Vec<f64>, temp: f64) -> Result<Vec<f64>, Error> {
            let mut param_n = param.clone();
            let mut rng = self.rng.lock().unwrap();
            let distr = Uniform::from(0..param.len());
            for _ in 0..(temp.floor() as u64 + 1) {

                let idx = rng.sample(distr);

                let val = rng.sample(Uniform::new_inclusive(-0.1, 0.1));
    
                param_n[idx] += val;
    
                param_n[idx] = param_n[idx].clamp(self.lower_bound[idx], self.upper_bound[idx]);
            }
            Ok(param_n)
        }
    }
    
     // Set up the optimization problem
    let optimizer = UniV3Optimizer::new(
        configured_pool_1.clone(),
        configured_pool_2.clone(),
        token_1_sdk.clone(),
        token_2_sdk.clone(),
        300.0,
        vec![0.0],
        vec![1.0],
    );

    // Define initial parameter vector
    let init_param: Vec<f64> = vec![1.0];
    // Define initial temperature
    let temp = 15.0;
    let solver = SimulatedAnnealing::new(temp).unwrap();

    let res = Executor::new(optimizer, solver)
    .configure(|state| {
        state
            .param(init_param)
            .max_iters(1000)
    })
    // .add_observer(SlogLogger::term(), ObserverMode::Always)
    .run().unwrap();
    println!("{res}");











    // // trade

    // // define trade 

    // let trade = Trade::from_route(
    //     Route::new(vec![configured_pool_1.clone()], token_1_sdk.clone(), token_0_sdk.clone()), // UPD
    //     amount_in_1.clone(),
    //     TradeType::ExactInput,
    // )
    // .unwrap();





    // // approve tokens
    // let pending = token_0_contract.approve( // UPD
    //     SWAP_ROUTER_ADDRESS.parse()?,
    //     amount_human.into(),
    // );
    // if let Some(receipt) = pending.send().await?.await? {
    //     println!("Receipt = {:?}", receipt);
    // }



    // let mut trades = [trade];
    // let timestamp_secs = Utc::now().timestamp(); 
    // let deadline = timestamp_secs + 60 * 20;

    // let swap_options:SwapOptions  = SwapOptions{
    //     slippage_tolerance: Percent::new(1, 1),
    //     recipient: ethers_address_to_alloy(wallet.address()),
    //     deadline:   AlloyU256::from(deadline as u64),
    //     input_token_permit: None,
    //     sqrt_price_limit_x96: None,
    //     fee: None,        

    // };

    // let swap_call_parameters =  utils::swap_call_parameters(&mut trades, swap_options).unwrap();
    // // println!("swap_call_parameters: {:?}", swap_call_parameters);
    // let calldata = swap_call_parameters.calldata;

    // // Construct the transaction
    // let tx = TransactionRequest::new()
    // .to(SWAP_ROUTER_ADDRESS)
    // .from(client.address())
    // .data(EthersBytes::from(calldata.0))
    // .value(0);





    // let pending_tx = client.send_transaction(tx, None).await?;

    // // Wait for the transaction to be mined
    // let receipt = pending_tx.confirmations(1).await?;
    
    // println!("Transaction Receipt: {}", serde_json::to_string_pretty(&receipt)?);






    Ok(())
}

