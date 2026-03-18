import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { NftStakingCore } from "../target/types/nft_staking_core";
import { SystemProgram } from "@solana/web3.js";
import { MPL_CORE_PROGRAM_ID } from "@metaplex-foundation/mpl-core";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

const MILLISECONDS_PER_DAY = 86400000;
const POINTS_PER_STAKED_NFT_PER_DAY = 10_000_000;
const FREEZE_PERIOD_IN_DAYS = 7;
const TIME_TRAVEL_IN_DAYS = 9;

describe("nft-staking-core", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.nftStakingCore as Program<NftStakingCore>;

  // Generate a keypair for the collection
  const collectionKeypair = anchor.web3.Keypair.generate();

  // Find the update authority for the collection (PDA)
  const progAuth = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("prog_auth"), collectionKeypair.publicKey.toBuffer()],
    program.programId,
  )[0];

  // Generate a keypair for the nft asset
  const nftKeypair = anchor.web3.Keypair.generate();

  // Find the config account (PDA)
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("cfg"), collectionKeypair.publicKey.toBuffer()],
    program.programId,
  )[0];

  // Find the rewards mint account (PDA)
  const rewardsMint = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("rwrd"), config.toBuffer()],
    program.programId,
  )[0];

  let timeTravelAvailable = true;
  let mplCoreAvailable = true;

  function isUnsupportedProgramError(error: any): boolean {
    const msg = String(error?.message ?? "");
    const logs = Array.isArray(error?.logs) ? error.logs.join("\n") : "";
    return (
      msg.includes("Unsupported program id") || logs.includes("Unsupported program id")
    );
  }

  it("Create a collection", async () => {
    if (!mplCoreAvailable) {
      return;
    }
    const collectionName = "Test Collection";
    const collectionUri = "https://example.com/collection";
    try {
      const tx = await program.methods
        .createCollection(collectionName, collectionUri)
        .accountsPartial({
          payer: provider.wallet.publicKey,
          collection: collectionKeypair.publicKey,
          progAuth,
          systemProgram: SystemProgram.programId,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
        })
        .signers([collectionKeypair])
        .rpc();
      console.log("\nYour transaction signature", tx);
      console.log("Collection address", collectionKeypair.publicKey.toBase58());
    } catch (error) {
      if (isUnsupportedProgramError(error)) {
        mplCoreAvailable = false;
        console.log("mpl-core not available on validator; skipping dependent tests");
        return;
      }
      throw error;
    }
  });

  it("Mint an NFT", async () => {
    if (!mplCoreAvailable) {
      return;
    }
    const nftName = "Test NFT";
    const nftUri = "https://example.com/nft";
    const tx = await program.methods
      .mintNft(nftName, nftUri)
      .accountsPartial({
        payer: provider.wallet.publicKey,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        progAuth,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([nftKeypair])
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("NFT address", nftKeypair.publicKey.toBase58());
  });

  it("Initialize stake config", async () => {
    if (!mplCoreAvailable) {
      return;
    }
    const tx = await program.methods
      .initializeConfig(POINTS_PER_STAKED_NFT_PER_DAY, FREEZE_PERIOD_IN_DAYS)
      .accountsPartial({
        admin: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        progAuth,
        config,
        rewardsMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("Config address", config.toBase58());
    console.log("Points per staked NFT per day", POINTS_PER_STAKED_NFT_PER_DAY);
    console.log("Freeze period in days", FREEZE_PERIOD_IN_DAYS);
    console.log("Rewards mint address", rewardsMint.toBase58());
  });

  it("Stake an NFT", async () => {
    if (!mplCoreAvailable) {
      return;
    }
    const tx = await program.methods
      .stake()
      .accountsPartial({
        stakeholder: provider.wallet.publicKey,
        progAuth,
        config,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);
  });

  /**
   * Helper function to advance time with surfnet_timeTravel RPC method
   * @param params - Time travel params (absoluteEpoch, absoluteSlot, or absoluteTimestamp)
   */
  async function advanceTime(params: {
    absoluteEpoch?: number;
    absoluteSlot?: number;
    absoluteTimestamp?: number;
  }): Promise<boolean> {
    const rpcResponse = await fetch(provider.connection.rpcEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "surfnet_timeTravel",
        params: [params],
      }),
    });

    const result = (await rpcResponse.json()) as { error?: any; result?: any };
    if (result.error) {
      if (result.error.code === -32601) {
        return false;
      }
      throw new Error(`Time travel failed: ${JSON.stringify(result.error)}`);
    }

    await new Promise((resolve) => setTimeout(resolve, 3000));
    return true;
  }

  it("Time travel to the future", async () => {
    // Advance time in milliseconds
    const currentTimestamp = Date.now();
    timeTravelAvailable = await advanceTime({
      absoluteTimestamp:
        currentTimestamp + TIME_TRAVEL_IN_DAYS * MILLISECONDS_PER_DAY,
    });
    if (!timeTravelAvailable) {
      console.log("surfnet_timeTravel RPC not available; skipping time-based tests");
      return;
    }
    console.log("\nTime traveled in days", TIME_TRAVEL_IN_DAYS);
  });

  it("Claims Rewards for a staked NFT", async () => {
    if (!timeTravelAvailable) {
      return;
    }
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    try {
      const tx = await program.methods
        .claimRewards()
        .accountsPartial({
          stakeholder: provider.wallet.publicKey,
          progAuth,
          config,
          rewardsMint,
          stakeholderAta: userRewardsAta,
          asset: nftKeypair.publicKey,
          collection: collectionKeypair.publicKey,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .rpc();

      console.log("\nYour transaction signature", tx);
    } catch (error) {
      console.log(error.logs);
      throw error;
    }
    console.log(
      "User rewards balance",
      (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount,
    );
  });

  it("Burns staked NFT for rewards", async () => {
    if (!timeTravelAvailable) {
      return;
    }
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    try {
      const tx = await program.methods
        .burnStakedNft()
        .accountsPartial({
          stakeholder: provider.wallet.publicKey,
          progAuth,
          config,
          rewardsMint,
          stakeholderAta: userRewardsAta,
          asset: nftKeypair.publicKey,
          collection: collectionKeypair.publicKey,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .rpc();
      console.log("\nYour transaction signature", tx);
    } catch (error) {
      console.log(error.logs);
      throw error;
    }
    console.log(
      "User rewards balance",
      (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount,
    );
  });

  it("Unstake an NFT", async () => {
    if (!timeTravelAvailable) {
      return;
    }
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    try {
      const tx = await program.methods
        .unstake()
        .accountsPartial({
          stakeholder: provider.wallet.publicKey,
          progAuth,
          config,
          rewardsMint,
          stakeholderAta: userRewardsAta,
          asset: nftKeypair.publicKey,
          collection: collectionKeypair.publicKey,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .rpc();
      console.log("\nYour transaction signature", tx);
    } catch (error) {
      console.log(error.logs);
      throw error;
    }
    console.log(
      "User rewards balance",
      (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount,
    );
  });
});
