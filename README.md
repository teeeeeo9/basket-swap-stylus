<!-- ![Image](./header.png) -->

# Basket Swap

BasketSwap is a Dapp that is under development on Arbitrum with Stylus that enables users to easily exchange a basket of tokens for a basket of other tokens in a single, optimized transaction.Think of a tool that would allow you to easily convert your crypto holdings into stablecoins at the best prices at an urgent moment - that's BasketSwap.

The smart contract solves an optimization problem - turns a single BasketSwap call into a series of atomic swaps across different pools and finds  routes and amounts for each swap to maximize the value of the returned token basket for the user. 

The search for the best allocation is built using adaptive grid search algorithm.  It incorporates a diversification strategy to avoid local optima and dynamically adjusts the step size for efficient convergence. This approach is adaptable to various objective functions and works even when the underlying pricing functions are complex.

This algorithm requires a lot more memory than the EVM allows and it's now possible to implement though the use of Stylus.
