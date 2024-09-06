// No imports needed: web3, anchor, pg, and more are globally available
const { SystemProgram } = web3;
const { PublicKey, Keypair } = web3;

describe("Defi Flash Loan Program", () => {
  const provider = anchor.AnchorProvider.env(); // Explicitly use AnchorProvider
  anchor.setProvider(provider); // Set the global provider

  const program = anchor.workspace.FlashLoan;

  // Create keypairs for accounts used in tests
  const loanVaultKp = Keypair.generate();
  const borrowerKp = Keypair.generate();
  const loanStatsKp = Keypair.generate();
  const loanStateKp = Keypair.generate();

  let loanAmount = new BN(500000); // Example loan amount
  let loanExpiration = new BN(Math.floor(Date.now() / 1000) + 60); // 1 min expiration

  // Setup token mint and token accounts for borrower and loan vault
  let tokenMint = null;
  let loanVaultTokenAccount = null;
  let borrowerTokenAccount = null;

  before(async () => {
    // Airdrop SOL to all necessary accounts
    await provider.connection.requestAirdrop(loanVaultKp.publicKey, 2 * web3.LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(borrowerKp.publicKey, 2 * web3.LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(loanStatsKp.publicKey, 1 * web3.LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(loanStateKp.publicKey, 1 * web3.LAMPORTS_PER_SOL);

    // Create a mint for the tokens
    tokenMint = await createMint(
      provider, // Anchor provider
      provider.wallet.publicKey, // Mint authority
      9 // Decimals for the mint
    );

    // Create token accounts
    loanVaultTokenAccount = await createTokenAccount(
      provider, tokenMint, loanVaultKp.publicKey
    );

    borrowerTokenAccount = await createTokenAccount(
      provider, tokenMint, borrowerKp.publicKey
    );

    // Mint tokens to the loan vault to fund the flash loans
    await mintTo(
      provider, tokenMint, loanVaultTokenAccount, provider.wallet.publicKey, [], 1000000 // 1 million tokens
    );
  });

  it("executes flash loan", async () => {
    // Prepare the flash loan transaction
    const txHash = await program.methods
      .executeFlashLoan(
        loanAmount,         // The amount to borrow
        loanExpiration,     // Loan expiration time
      )
      .accounts({
        loanVault: loanVaultTokenAccount,       // Loan pool
        borrowerAccount: borrowerTokenAccount,  // Borrower's token account
        borrower: borrowerKp.publicKey,         // Borrower signer
        loanStats: loanStatsKp.publicKey,       // Statistics account
        loanState: loanStateKp.publicKey,       // State account
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID, // Token program
        tokenMint: tokenMint,                   // The mint for the loan tokens
      })
      .signers([borrowerKp])
      .rpc();

    console.log(`Transaction hash: ${txHash}`);

    // Confirm the transaction
    await provider.connection.confirmTransaction(txHash);

    // Fetch the updated loan vault token account
    const updatedLoanVaultAccount = await getTokenAccount(provider, loanVaultTokenAccount);
    const updatedBorrowerAccount = await getTokenAccount(provider, borrowerTokenAccount);

    console.log("Loan Vault Token Amount:", updatedLoanVaultAccount.amount.toString());
    console.log("Borrower Token Amount:", updatedBorrowerAccount.amount.toString());

    // Check that the loan was transferred to the borrower
    assert.ok(new BN(updatedBorrowerAccount.amount).eq(loanAmount));

    // Check that the vault balance has decreased by the loan amount
    assert.ok(new BN(updatedLoanVaultAccount.amount).eq(new BN(1000000).sub(loanAmount)));

    // Check for successful repayment and loan state reset
    const loanState = await program.account.loanState.fetch(loanStateKp.publicKey);
    assert.ok(loanState.active === false); // Ensure the loan is no longer active
  });
});

// Helper function to create token mint
async function createMint(provider, authority, decimals) {
  const mint = await TokenInstructions.createMint(
    provider.connection,
    provider.wallet.payer,
    authority,
    null,
    decimals,
    TokenInstructions.TOKEN_PROGRAM_ID
  );
  return mint;
}

// Helper function to create token account
async function createTokenAccount(provider, mint, owner) {
  const tokenAccount = await TokenInstructions.createAccount(
    provider.connection,
    provider.wallet.payer,
    mint,
    owner,
    TokenInstructions.TOKEN_PROGRAM_ID
  );
  return tokenAccount;
}

// Helper function to mint tokens
async function mintTo(provider, mint, destination, authority, signers, amount) {
  await TokenInstructions.mintTo(
    provider.connection,
    provider.wallet.payer,
    mint,
    destination,
    authority,
    amount,
    signers
  );
}

// Helper function to get token account info
async function getTokenAccount(provider, tokenAccount) {
  return await TokenInstructions.getAccountInfo(provider.connection, tokenAccount);
}
