use stylus_sdk::prelude::SolidityError;
use stylus_sdk::alloy_sol_types::sol;
use stylus_sdk::alloy_primitives::{U256, uint};

// Declare events and Solidity error types
sol! {
    error DenominatorIsZero();
    error ResultIsU256MAX();
    error SqrtPriceIsZero();
    error SqrtPriceIsLteQuotient();
    error ZeroValue();
    error LiquidityIsZero();
    error ProductDivAmount();
    error DenominatorIsLteProdOne();
    error LiquiditySub();
    error SafeCastToU160Overflow();
    error TestError();

}

#[derive(SolidityError)]
pub enum UniswapV3MathError {
    DenominatorIsZero(DenominatorIsZero),
    ResultIsU256MAX(ResultIsU256MAX),
    SqrtPriceIsZero(SqrtPriceIsZero),
    SqrtPriceIsLteQuotient(SqrtPriceIsLteQuotient),
    ZeroValue(ZeroValue),
    LiquidityIsZero(LiquidityIsZero),
    ProductDivAmount(ProductDivAmount),
    DenominatorIsLteProdOne(DenominatorIsLteProdOne),
    LiquiditySub(LiquiditySub),
    SafeCastToU160Overflow(SafeCastToU160Overflow),

}


#[derive(SolidityError)]
pub enum GenError {
    TestError(TestError),
}

pub const ONE: U256 = uint!(1_U256);
pub const TWO: U256 = uint!(2_U256);
pub const THREE: U256 = uint!(3_U256);
pub const Q96: U256 = U256::from_limbs([0, 4294967296, 0, 0]);
pub const Q128: U256 = U256::from_limbs([0, 0, 1, 0]);
pub const Q192: U256 = U256::from_limbs([0, 0, 0, 1]);
