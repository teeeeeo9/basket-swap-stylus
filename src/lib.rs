// #![no_main]
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;



#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;




use std::borrow::Borrow;
use std::ops::Add;
use num_bigint::{BigInt, ToBigInt};
use alloc::vec::Vec;

use stylus_sdk::stylus_proc::entrypoint;


use stylus_sdk;

use stylus_sdk::{alloy_primitives::{address, Address, U256,U128, Uint, I256}};
use stylus_sdk::prelude::sol_interface;

use alloy_sol_types::{self};
use stylus_sdk::prelude::*;
use utils::{GenError, TestError, UniswapV3MathError, ZeroValue};


mod utils;
mod utils_2;
mod uniswap_math;  
mod sqrt_price_math; 
mod full_math;

use uniswap_math::compute_swap_step;



pub const UNI_FACTORY_ADDRESS: &str = "0x248AB79Bbb9bC29bB72f7Cd42F17e054Fc40188e";
pub const UNI_QUOTER_ADDRESS: &str = "0x2779a0CC1c3e0E44D2542EC3e79e3864Ae93Ef0B";

sol_interface! {



    interface INonfungiblePositionManager {

    }

    interface IQuoter {
        function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn,uint160 sqrtPriceLimitX96) external returns (uint256 amountOut);
    }

    interface IUniswapV3Factory {
        function getPool(
            address tokenA,
            address tokenB,
            uint24 fee
          ) external view returns (address pool);

    }
    interface IUniswapV3Pool {
        function liquidity() external view returns (uint128);
        function slot0() external view returns (uint160 sqrtPriceX96,  int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function fee() external view returns (uint24);
    }
}


// Define some persistent storage using the Solidity ABI.
sol_storage! {
    #[entrypoint]
    pub struct Quoter {
        
        address factoryAddress; 
    }
  
}



#[external]
impl Quoter {
    fn get_quote(
        &self,
        amount_in: U256,
        token_in: Address,
        token_out: Address,
        fee: u32

    ) -> Result<(Uint<256, 4>, Uint<256, 4>, Uint<256, 4>, Uint<256, 4>), UniswapV3MathError> {



        let uni_factory_address: Address = UNI_FACTORY_ADDRESS.parse().unwrap();
        let factory_contract = IUniswapV3Factory::new(uni_factory_address);
        let pool_address = factory_contract.get_pool(
            self, 
            token_in,
            token_out,
            fee
        ).unwrap();

        let pool_contract = IUniswapV3Pool::new(pool_address);
        let liquidity = pool_contract.liquidity(self).unwrap(); 
        let slot0 = pool_contract.slot_0(self).unwrap(); 
        let sqrt_price_x96 = slot0.0;

    
        
        let token_1_pool = pool_contract.token_1(self).unwrap();
        let mut sqrt_ratio_target_x96 = U256::MIN + U256::from(1);
        if token_in == token_1_pool {
            sqrt_ratio_target_x96 = U256::MAX - U256::from(1);
        }

        let amount_remaining = I256::from_raw(amount_in);
        let res = compute_swap_step(sqrt_price_x96, sqrt_ratio_target_x96, liquidity, amount_remaining, fee);

        res

    }

   
}


