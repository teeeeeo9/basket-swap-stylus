#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]



use ethers::{
    middleware::SignerMiddleware, prelude::abigen, providers::{Http, Middleware, Provider}, signers::{LocalWallet, Signer}, types::{Address, U256}
};
use eyre::eyre;
use rust_decimal::prelude::Zero;
use uniswap_sdk_core::{constants::TradeType, prelude::{BigInt, CurrencyTrait, FractionBase, Integer}};
use uniswap_v3_sdk::{entities::{Swap, Trade}, multicall::encode_multicall, self_permit::encode_permit, swap_router::SwapOptions, utils::{big_int_to_u256, encode_route_to_path, MethodParameters}};
use std::io::{BufRead, BufReader};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use alloy_primitives::{Bytes, FixedBytes};


pub fn read_secret_from_file(fpath: &str) -> eyre::Result<String> {
    let f = std::fs::File::open(fpath)?;
    let mut buf_reader = BufReader::new(f);
    let mut secret = String::new();
    buf_reader.read_line(&mut secret)?;
    Ok(secret.trim().to_string())
}

/// Converts [alloy_primitives::Address] to [ethers::types::H160]
pub fn alloy_address_to_ethers(address: alloy_primitives::Address) -> ethers::types::H160 {
    address.into_array().into()
}

/// Converts [ethers::types::H160] to [alloy_primitives::Address]
pub fn ethers_address_to_alloy(address: ethers::types::H160) -> alloy_primitives::Address {
    address.to_fixed_bytes().into()
}


pub fn calculate_amount(tokens: f64, decimals: u32) -> U256 {
    let decimal_adjustment = 10_u64.pow(decimals); 
    U256::from(tokens as u64) * U256::from(decimal_adjustment) 
}


pub fn calculate_amount_u64(tokens: f64, decimals: u32) -> u64 {
    let decimal_adjustment = 10_u64.pow(decimals); 
    tokens as u64 * decimal_adjustment
}


pub const PROGRAM_ADDRESS: &str = " ";
pub const UNI_FACTORY_ADDRESS: &str = "0x248AB79Bbb9bC29bB72f7Cd42F17e054Fc40188e";
pub const POSITION_MANAGER_ADDRESS: &str = "0x6b2937Bde17889EDCf8fbD8dE31C3C2a70Bc4d65";
pub const QUOTER_ADDRESS: &str = "0x2779a0CC1c3e0E44D2542EC3e79e3864Ae93Ef0B";
pub const TICK_LENS_ADDRESS: &str = "0x0fd18587734e5C2dcE2dccDcC7DD1EC89ba557d9";
pub const SWAP_ROUTER_ADDRESS: &str = "0x101F443B4d1b059569D643917553c771E1b9663E";

pub const TOKEN_0_ADDRESS: &str = " "; 
pub const TOKEN_1_ADDRESS: &str = " "; 
pub const TOKEN_2_ADDRESS: &str = " "; 



pub const RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";

pub const PRIV_KEY_PATH: &str = " ";



use alloy_sol_types::{sol, SolCall};


sol! {

    interface ISwapRouter {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            // uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);

        struct ExactInputParams {
            bytes path;
            address recipient;
            // uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
        }

        function exactInput(ExactInputParams calldata params) external payable returns (uint256 amountOut);

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactOutputSingle(ExactOutputSingleParams calldata params) external payable returns (uint256 amountIn);

        struct ExactOutputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
        }

        function exactOutput(ExactOutputParams calldata params) external payable returns (uint256 amountIn);
    }
}

pub fn swap_call_parameters<TInput: CurrencyTrait, TOutput: CurrencyTrait, P: Clone>(
    trades: &mut [Trade<TInput, TOutput, P>],
    options: SwapOptions,
) -> Result<MethodParameters> {
    let SwapOptions {
        slippage_tolerance,
        recipient,
        deadline,
        input_token_permit,
        sqrt_price_limit_x96,
        fee,
    } = options;
    let mut sample_trade = trades[0].clone();
    let token_in = sample_trade.input_amount()?.currency.wrapped();
    let token_out = sample_trade.output_amount()?.currency.wrapped();

    // All trades should have the same starting and ending token.
    for trade in trades.iter_mut() {
        assert!(
            trade.input_amount()?.currency.wrapped().equals(&token_in),
            "TOKEN_IN_DIFF"
        );
        assert!(
            trade.output_amount()?.currency.wrapped().equals(&token_out),
            "TOKEN_OUT_DIFF"
        );
    }

    let num_swaps = trades.iter().map(|trade| trade.swaps.len()).sum::<usize>();

    let mut calldatas: Vec<Bytes> = Vec::with_capacity(num_swaps + 3);

    let mut total_amount_out = BigInt::zero();
    for trade in trades.iter_mut() {
        total_amount_out += trade
            .minimum_amount_out(slippage_tolerance.clone(), None)?
            .quotient();
    }
    let total_amount_out = big_int_to_u256(total_amount_out);

    // flag for whether a refund needs to happen
    let input_is_native = sample_trade.input_amount()?.currency.is_native();
    let must_refund = input_is_native && sample_trade.trade_type == TradeType::ExactOutput;
    // flags for whether funds should be sent first to the router
    let output_is_native = sample_trade.output_amount()?.currency.is_native();
    let router_must_custody = output_is_native || fee.is_some();

    let mut total_value = BigInt::zero();
    if input_is_native {
        for trade in trades.iter_mut() {
            total_value += trade
                .maximum_amount_in(slippage_tolerance.clone(), None)?
                .quotient();
        }
    }


    for trade in trades.iter_mut() {
        for Swap {
            route,
            input_amount,
            output_amount,
        } in trade.swaps.clone().iter_mut()
        {
            let amount_in = big_int_to_u256(
                trade
                    .maximum_amount_in(slippage_tolerance.clone(), Some(input_amount.clone()))?
                    .quotient(),
            );
            let amount_out = big_int_to_u256(
                trade
                    .minimum_amount_out(slippage_tolerance.clone(), Some(output_amount.clone()))?
                    .quotient(),
            );

            if route.pools.len() == 1 {
                calldatas.push(match trade.trade_type {
                    TradeType::ExactInput => ISwapRouter::exactInputSingleCall {
                        params: ISwapRouter::ExactInputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            fee: route.pools[0].fee as u32,
                            recipient: recipient,
                            // deadline,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                            sqrtPriceLimitX96: sqrt_price_limit_x96.unwrap_or_default(),
                        },
                    }
                    .abi_encode()
                    .into(),
                    TradeType::ExactOutput => ISwapRouter::exactOutputSingleCall {
                        params: ISwapRouter::ExactOutputSingleParams {
                            tokenIn: route.token_path[0].address(),
                            tokenOut: route.token_path[1].address(),
                            fee: route.pools[0].fee as u32,
                            recipient: recipient,
                            deadline,
                            amountOut: amount_out,
                            amountInMaximum: amount_in,
                            sqrtPriceLimitX96: sqrt_price_limit_x96.unwrap_or_default(),
                        },
                    }
                    .abi_encode()
                    .into(),
                });
            } else {
                assert!(sqrt_price_limit_x96.is_none(), "MULTIHOP_PRICE_LIMIT");

                let path = encode_route_to_path(route, trade.trade_type == TradeType::ExactOutput);

                calldatas.push(match trade.trade_type {
                    TradeType::ExactInput => ISwapRouter::exactInputCall {
                        params: ISwapRouter::ExactInputParams {
                            path,
                            recipient: recipient,
                            // deadline,
                            amountIn: amount_in,
                            amountOutMinimum: amount_out,
                        },
                    }
                    .abi_encode()
                    .into(),
                    TradeType::ExactOutput => ISwapRouter::exactOutputCall {
                        params: ISwapRouter::ExactOutputParams {
                            path,
                            recipient: recipient,
                            deadline,
                            amountOut: amount_out,
                            amountInMaximum: amount_in,
                        },
                    }
                    .abi_encode()
                    .into(),
                });
            }
        }
    }

   

    Ok(MethodParameters {
        calldata: encode_multicall(calldatas),
        value: big_int_to_u256(total_value),
    })
}



 fn main() -> eyre::Result<()> {    
Ok(())
}




pub fn keccak256<T: AsRef<[u8]>>(bytes: T) -> FixedBytes<32> {

            /// Calls [`tiny-keccak`] when the `tiny-keccak` feature is enabled or
            /// when no particular keccak feature flag is specified.
            ///
            /// [`tiny_keccak`]: https://docs.rs/tiny-keccak/latest/tiny_keccak/
            fn keccak256(bytes: &[u8]) -> FixedBytes<32> {
                use tiny_keccak::{Hasher, Keccak};

                let mut output = [0u8; 32];
                let mut hasher = Keccak::v256();
                hasher.update(bytes);
                hasher.finalize(&mut output);
                output.into()
            }
        
    

    keccak256(bytes.as_ref())
}
