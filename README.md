# defiflashloan
This project implements a decentralized flash loan service on Solana using the **Anchor framework**. The flash loan allows users to borrow tokens and repay them within the same transaction. This setup was developed and tested in **Solana Playground IDE**.(https://beta.solpg.io/) I only developed and ran this project in solana playground no local env.

## Features

- **Flash Loan**: Borrow tokens without collateral and repay within the same transaction.
- **Loan Vault**: A pool of tokens from which flash loans are drawn.
- **Dynamic Fees**: The loan fees are dynamically calculated based on the size of the loan.
- **Reentrancy Guard**: Protection against reentrancy attacks during loan execution.
- **Cross-Program Invocation (CPI)**: Supports interaction with other programs during the loan.

  # License
  This project is under MIT License 

