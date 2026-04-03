import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { expect } from "chai";
import BN from "bn.js";

const AIRDROP_LAMPORTS = 2 * anchor.web3.LAMPORTS_PER_SOL;

describe("escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.escrow as Program;
  const connection = provider.connection;

  const maker = anchor.web3.Keypair.generate();
  const taker = anchor.web3.Keypair.generate();

  const payer = (provider.wallet as anchor.Wallet & { payer: anchor.web3.Keypair }).payer;

  let mint: anchor.web3.PublicKey;

  it("locks deposits and releases only after both confirmations", async () => {
    const [makerAirdropSig, takerAirdropSig] = await Promise.all([
      connection.requestAirdrop(maker.publicKey, AIRDROP_LAMPORTS),
      connection.requestAirdrop(taker.publicKey, AIRDROP_LAMPORTS),
    ]);

    await Promise.all([
      connection.confirmTransaction(makerAirdropSig, "confirmed"),
      connection.confirmTransaction(takerAirdropSig, "confirmed"),
    ]);

    mint = await createMint(connection, payer, payer.publicKey, null, 6);

    const makerToken = await getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      mint,
      maker.publicKey
    );

    const takerToken = await getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      mint,
      taker.publicKey
    );

    const beneficiaryToken = await getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      mint,
      maker.publicKey
    );

    await Promise.all([
      mintTo(connection, payer, mint, makerToken.address, payer, 2_000_000),
      mintTo(connection, payer, mint, takerToken.address, payer, 2_000_000),
    ]);

    const seed = new BN(42);
    const makerAmount = new BN(500_000);
    const takerAmount = new BN(700_000);

    const seedBytes = seed.toArrayLike(Buffer, "le", 8);

    const [escrowPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), maker.publicKey.toBuffer(), seedBytes],
      program.programId
    );

    const [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), escrowPda.toBuffer()],
      program.programId
    );

    await program.methods
      .initializeEscrow(seed, makerAmount, takerAmount, maker.publicKey)
      .accounts({
        initializer: maker.publicKey,
        counterparty: taker.publicKey,
        mint,
        escrow: escrowPda,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    await program.methods
      .deposit(makerAmount)
      .accounts({
        depositor: maker.publicKey,
        escrow: escrowPda,
        depositorTokenAccount: makerToken.address,
        vault: vaultPda,
        mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([maker])
      .rpc();

    await program.methods
      .deposit(takerAmount)
      .accounts({
        depositor: taker.publicKey,
        escrow: escrowPda,
        depositorTokenAccount: takerToken.address,
        vault: vaultPda,
        mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([taker])
      .rpc();

    await program.methods
      .confirmDeposit()
      .accounts({
        signer: maker.publicKey,
        escrow: escrowPda,
      })
      .signers([maker])
      .rpc();

    await program.methods
      .confirmDeposit()
      .accounts({
        signer: taker.publicKey,
        escrow: escrowPda,
      })
      .signers([taker])
      .rpc();

    await program.methods
      .releaseFunds()
      .accounts({
        caller: maker.publicKey,
        escrow: escrowPda,
        vault: vaultPda,
        recipientTokenAccount: beneficiaryToken.address,
        initializer: maker.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([maker])
      .rpc();

    const beneficiaryAccount = await getAccount(connection, beneficiaryToken.address);
    const beneficiaryBalance = Number(beneficiaryAccount.amount);

    expect(beneficiaryBalance).to.equal(2_000_000 + 700_000);

  });
});
