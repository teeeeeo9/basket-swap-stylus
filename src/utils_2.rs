use alloy_primitives::{Address, I256, U256};
use core::ops::Neg;
use num_bigint::{BigInt, BigUint, Sign};


pub fn u256_to_big_uint(x: U256) -> BigUint {
    BigUint::from_bytes_be(&x.to_be_bytes::<32>())
}

pub fn u256_to_big_int(x: U256) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, &x.to_be_bytes::<32>())
}

pub fn i256_to_big_int(x: I256) -> BigInt {
    if x.is_positive() {
        u256_to_big_int(x.into_raw())
    } else {
        u256_to_big_int(x.neg().into_raw()).neg()
    }
}



pub const fn u128_to_uint256(x: u128) -> U256 {
    U256::from_limbs([x as u64, (x >> 64) as u64, 0, 0])
}

