import * as anchor from "@project-serum/anchor";
import { Program, web3 } from "@project-serum/anchor";
import { Fruitbasket } from "../target/types/fruitbasket";
import {
  NATIVE_MINT,
  Token,
  TOKEN_PROGRAM_ID,
  AccountLayout as TokenAccountLayout,
  u64,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  AccountInfo,
} from "@solana/spl-token";

import { Market, OpenOrders, DexInstructions } from "@project-serum/serum";

import * as pyth from "./utils/pyth";
import * as serum from "./utils/serum";
import { token } from "@project-serum/anchor/dist/cjs/utils";
import mlog from "mocha-logger";
import { assert } from "chai";
import { TestToken, TestUtils } from "./utils/test_utils";
import { TokenAccount } from "@blockworks-foundation/mango-client";

type Connection = web3.Connection;

describe("fruitbasket", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);
  const connection = provider.connection;
  const wallet = provider.wallet;
  //configure test utils
  const program = anchor.workspace.Fruitbasket as Program<Fruitbasket>;
  type FruitBasketGroup = anchor.IdlAccounts<Fruitbasket>["fruitBasketGroup"];
  type Basket = anchor.IdlAccounts<Fruitbasket>["basket"];
  type BasketComponent =
    anchor.IdlTypes<Fruitbasket>["BasketComponentDescription"];

  const owner = web3.Keypair.generate();
  const test_utils = new TestUtils(provider.connection, provider.wallet);
  let serum_utils = new serum.SerumUtils(test_utils);
  let oracle = new pyth.Pyth(connection, wallet);
  const programId = program.programId;

  // create some tokens
  const usdc = test_utils.createToken(6, wallet.publicKey);
  const btc = test_utils.createToken(6, wallet.publicKey);
  const eth = test_utils.createToken(6, wallet.publicKey);
  const sol = test_utils.createNativeToken();
  const srm = test_utils.createToken(6, wallet.publicKey);
  const mngo = test_utils.createToken(6, wallet.publicKey);
  const shit1 = test_utils.createToken(6, wallet.publicKey);
  const shit2 = test_utils.createToken(6, wallet.publicKey);

  let quote_token: TestToken;
  let tokens = [btc, eth, sol, srm, mngo, shit1, shit2];
  const nb_tokens = tokens.length;
  let token_names = ["BTC", "ETH", "SOL", "SRM", "MNGO", "SHIT1", "SHIT2"];
  let token_prices = [
    40000000000n,
    4000000000n,
    200000000n,
    4000000n,
    1400000n,
    1000000n,
    100000n,
  ];
  let token_exp = [-6, -6, -6, -6, -6, -6, -6];
  assert.ok(tokens.length == token_names.length);
  assert.ok(tokens.length == token_prices.length);
  assert.ok(tokens.length == token_exp.length);

  let markets_by_tokens: Market[] = [];
  it("Market intialized", async () => {
    let t = await Promise.all(tokens);
    quote_token = await usdc;
    markets_by_tokens = await Promise.all(
      Array.from(Array(t.length).keys()).map((x) => {
        const marketPrice = Number(token_prices[x]) * (10**token_exp[x]);
        return serum_utils.createAndMakeMarket(t[x], quote_token, marketPrice, -token_exp[x]);
      })
    );
  });

  // check if serum is loaded
  let price_oracles = [];
  let produce_oracles = [];
  it("Oracles initialized", async () => {
    for (let i = 0; i < nb_tokens; ++i) {
      price_oracles.push(oracle.createPriceAccount());
      produce_oracles.push(oracle.createProductAccount());
    }
    let oracle_promises = [];
    for (let i = 0; i < nb_tokens; ++i) {
      oracle_promises.push(
        oracle.updatePriceAccount(await price_oracles[i], {
          exponent: token_exp[i],
          aggregatePriceInfo: {
            price: token_prices[i],
            conf: token_prices[i] / 100n, // 100 bps or 1% of the price of USDC
          },
        })
      );
    }

    for (let i = 0; i < nb_tokens; ++i) {
      await Promise.all(oracle_promises);
    }
  });

  let frt_bsk_group = null;
  let frt_bsk_cache = null;
  let fruitbasket_authority: web3.PublicKey;
  let fb_auth_bump: number;
  let quote_token_transaction_pool: web3.PublicKey;

  it("Group initialized", async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(owner.publicKey, 100000000000),
      "confirmed"
    );

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(wallet.publicKey, 100000000000),
      "confirmed"
    );

    [fruitbasket_authority, fb_auth_bump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from("fruitbasket_auth")],
        programId
      );
    const [tmp_group, bump_grp] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket_group"), owner.publicKey.toBuffer()],
      program.programId
    );
    const [tmp_cache, bump_cache] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket_cache"), owner.publicKey.toBuffer()],
      program.programId
    );

    frt_bsk_group = tmp_group;
    frt_bsk_cache = tmp_cache;
    mlog.log("group : " + frt_bsk_group);
    mlog.log("cache : " + frt_bsk_cache);
    quote_token_transaction_pool = await quote_token.createAccount(
      owner.publicKey
    );
    await program.rpc.initializeGroup(bump_grp, bump_cache, "USDC", {
      accounts: {
        owner: owner.publicKey,
        fruitBasketGrp: frt_bsk_group,
        cache: frt_bsk_cache,
        quoteTokenMint: quote_token.publicKey,
        quoteTokenTransactionPool: quote_token_transaction_pool,
        systemProgram: web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      signers: [owner],
    });
  });

  let open_orders_by_token: web3.Keypair[];
  let token_pools: web3.PublicKey[];
  it("Tokens added ", async () => {
    let token_list = await Promise.all(tokens);
    const openOrdersSpace = OpenOrders.getLayout(serum.DEX_ID).span;
    open_orders_by_token = await Promise.all(
      token_list.map(async (x) =>
        test_utils.createAccount(owner, serum.DEX_ID, openOrdersSpace)
      )
    );

    token_pools = await Promise.all(
      token_list.map(async (x) => x.createAccount(owner.publicKey))
    );
    let markets = markets_by_tokens;
    for (let index = 0; index < nb_tokens; ++index) {
      let marketKey = markets[index].publicKey;
      let open_orders = open_orders_by_token[index];
      await program.rpc.addToken(token_names[index], {
        accounts: {
          owner: owner.publicKey,
          fruitBasketGrp: frt_bsk_group,
          mint: token_list[index].publicKey,
          priceOracle: (await price_oracles[index]).publicKey,
          productOracle: (await produce_oracles[index]).publicKey,
          tokenPool: token_pools[index],
          market: marketKey,
          openOrdersAccount: open_orders.publicKey,
          fruitbasketAuthority: fruitbasket_authority,
          tokenProgram: TOKEN_PROGRAM_ID,
          dexProgram: serum.DEX_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner],
      });
    }
  });

  let basket_1: web3.PublicKey;
  let basket_2: web3.PublicKey;
  let basket_3: web3.PublicKey;
  let basket_1_mint: web3.PublicKey;
  let basket_2_mint: web3.PublicKey;
  let basket_3_mint: web3.PublicKey;

  it("Baskets created ", async () => {
    const exp = 1000000;
    let comp_btc = new ComponentInfo();
    comp_btc.tokenIndex = 0;
    comp_btc.amount = new anchor.BN(exp * 0.01); // 0.01 BTC
    comp_btc.decimal = 6;

    let comp_eth = new ComponentInfo();
    comp_eth.tokenIndex = 1;
    comp_eth.amount = new anchor.BN(exp * 0.1); // 0.1 ETC
    comp_eth.decimal = 6;

    let comp_sol = new ComponentInfo();
    comp_sol.tokenIndex = 2;
    comp_sol.amount = new anchor.BN(2 * web3.LAMPORTS_PER_SOL); // 2 SOL
    comp_sol.decimal = 9;

    let comp_srm = new ComponentInfo();
    comp_srm.tokenIndex = 3;
    comp_srm.amount = new anchor.BN(exp * 100); // 100 SRM
    comp_srm.decimal = 6;

    let comp_mngo = new ComponentInfo();
    comp_mngo.tokenIndex = 4;
    comp_mngo.amount = new anchor.BN(exp * 1000); // 1000 MNGO
    comp_mngo.decimal = 6;

    let comp_sh1 = new ComponentInfo();
    comp_sh1.tokenIndex = 5;
    comp_sh1.amount = new anchor.BN(exp * 10000); // 10000 SHIT1
    comp_sh1.decimal = 6;

    let comp_sh2 = new ComponentInfo();
    comp_sh2.tokenIndex = 6;
    comp_sh2.amount = new anchor.BN(exp * 100000); // 100000 SHIT1
    comp_sh2.decimal = 6;

    // first basket
    let basket_nb = 0;
    const [_basket_1, bump_b1] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket"), Buffer.from([basket_nb])],
      program.programId
    );
    const [_basket_1_mint, bump_b1m] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket_mint"), Buffer.from([basket_nb])],
      program.programId
    );
    const components_1 = [comp_btc, comp_eth, comp_sol];
    basket_1 = _basket_1;
    basket_1_mint = _basket_1_mint;

    await program.rpc.addBasket(
      basket_nb,
      bump_b1,
      bump_b1m,
      "First tier coins",
      "Basket for first teer coins",
      components_1,
      {
        accounts: {
          client: owner.publicKey,
          group: frt_bsk_group,
          basket: basket_1,
          basketMint: basket_1_mint,
          systemProgram: web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner],
      }
    );

    // second basket
    ++basket_nb;
    const [_basket_2, bump_b2] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket"), Buffer.from([basket_nb])],
      program.programId
    );
    const [_basket_2_mint, bump_b2m] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket_mint"), Buffer.from([basket_nb])],
      program.programId
    );
    const components_2 = [comp_sol, comp_srm, comp_mngo];
    basket_2 = _basket_2;
    basket_2_mint = _basket_2_mint;

    await program.rpc.addBasket(
      basket_nb,
      bump_b2,
      bump_b2m,
      "Solana coins",
      "Basket for coins base on solana",
      components_2,
      {
        accounts: {
          client: owner.publicKey,
          group: frt_bsk_group,
          basket: basket_2,
          basketMint: basket_2_mint,
          systemProgram: web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner],
      }
    );

    // third basket
    ++basket_nb;
    const [_basket_3, bump_b3] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket"), Buffer.from([basket_nb])],
      program.programId
    );
    const [_basket_3_mint, bump_b3m] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("fruitbasket_mint"), Buffer.from([basket_nb])],
      program.programId
    );
    const components_3 = [comp_sh1, comp_sh2];
    basket_3 = _basket_3;
    basket_3_mint = _basket_3_mint;

    await program.rpc.addBasket(
      basket_nb,
      bump_b3,
      bump_b3m,
      "Shit coins",
      "Basket for shit coins that have potential in future",
      components_3,
      {
        accounts: {
          client: owner.publicKey,
          group: frt_bsk_group,
          basket: basket_3,
          basketMint: basket_3_mint,
          systemProgram: web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner],
      }
    );
  });

  it("oracle group tests", async () => {
    let group_info: FruitBasketGroup =
      await program.account.fruitBasketGroup.fetch(frt_bsk_group);

    assert.ok(group_info.baseMint.equals(quote_token.publicKey));
    assert.ok(group_info.nbUsers == 0);
    assert.ok(group_info.numberOfBaskets == 3);
    assert.ok(group_info.tokenCount == nb_tokens);
    // for (let token_desc in group_info.tokenDescription){

    // }
  });

  it("cache updated", async () => {
    await Promise.all(
      price_oracles.map(async (x) => {
        await program.rpc.updatePrice({
          accounts: {
            group: frt_bsk_group,
            cache: frt_bsk_cache,
            oracleAi: (await x).publicKey,
          },
        });
      })
    );
  });
  let basket_1_price : anchor.BN;
  let basket_1_confidence : anchor.BN;
  it("basket priced", async () => {
    // price basket 1
    await program.rpc.updateBasketPrice({
      accounts: {
        basket: basket_1,
        cache: frt_bsk_cache,
      },
    });
    const basket_1_info: Basket = await program.account.basket.fetch(basket_1);
    basket_1_price = basket_1_info.lastPrice;
    basket_1_confidence = basket_1_info.confidence;
    assert.ok(basket_1_info.lastPrice.toNumber() == 1200000000);
    assert.ok(basket_1_info.decimal == 6);
    assert.ok(basket_1_info.confidence.toNumber() == 12000000);

    // price basket 2
    await program.rpc.updateBasketPrice({
      accounts: {
        basket: basket_2,
        cache: frt_bsk_cache,
      },
    });

    const basket_2_info: Basket = await program.account.basket.fetch(basket_2);
    assert.ok(basket_2_info.lastPrice.toNumber() > 0);
    assert.ok(basket_2_info.decimal == 6);
    assert.ok(basket_2_info.confidence.toNumber() > 0);

    // price basket 3
    await program.rpc.updateBasketPrice({
      accounts: {
        basket: basket_3,
        cache: frt_bsk_cache,
      },
    });
    const basket_3_info: Basket = await program.account.basket.fetch(basket_3);
    assert.ok(basket_3_info.lastPrice.toNumber() > 0);
    assert.ok(basket_3_info.decimal == 6);
    assert.ok(basket_3_info.confidence.toNumber() > 0);
  });
  let market_data;
  let client_1 = web3.Keypair.generate();
  let client_usdc_acc: web3.PublicKey;
  let client_basket_token_acc: web3.PublicKey;

  type BasketTradeContext = anchor.IdlAccounts<Fruitbasket>["basketTradeContext"];
  const ContextSide = {
    Buy: { buy: {} },
    Sell: { sell: {} },
  };

  it("Buy Basket", async () => {
    await connection.confirmTransaction(
      await connection.requestAirdrop(
        client_1.publicKey,
        10 * web3.LAMPORTS_PER_SOL
      )
    );

    let markets = markets_by_tokens;
    let token_list = await Promise.all(tokens);
    let max_price = new anchor.BN(10 ** 10);
    client_usdc_acc = await quote_token.createAccount(client_1.publicKey);
    // deposit some usdc
    quote_token.mintTo(
      client_usdc_acc,
      wallet.publicKey,
      [test_utils.payer()],
      max_price.toNumber()
    );

    let client_basket_token = new Token(
      connection,
      basket_1_mint,
      TOKEN_PROGRAM_ID,
      owner
    );
    client_basket_token_acc = await client_basket_token.createAccount(
      client_1.publicKey
    );
    const [buy_context, buy_context_bump] =
      await web3.PublicKey.findProgramAddress(
        [
          Buffer.from("fruitbasket_context"),
          client_1.publicKey.toBuffer(),
          Buffer.from([0]),
        ],
        programId
      );
    
    const usdc_before_transaction : TokenAccount = await quote_token.getAccountInfo(client_usdc_acc);
    
    usdc_before_transaction.amount
    
    await program.rpc.initBuyBasket(
      0,
      buy_context_bump,
      new anchor.BN(1000000), // buy 1 basket
      new anchor.BN(max_price),
      {
        accounts: {
          group: frt_bsk_group,
          user: client_1.publicKey,
          basket: basket_1,
          cache: frt_bsk_cache,
          payingAccount: client_usdc_acc,
          userBasketTokenAccount: client_basket_token_acc,
          payingTokenMint: quote_token.publicKey,
          buyContext: buy_context,
          quoteTokenTransactionPool: quote_token_transaction_pool,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: web3.SystemProgram.programId,
        },
        signers: [client_1],
      }
    );
    const basket_1_info: Basket = await program.account.basket.fetch(basket_1);
    const basket_components : [BasketComponent] = basket_1_info.components;
    const buy_context_info: BasketTradeContext = await program.account.basketTradeContext.fetch(buy_context);
    const worst_basket_price = 1224120000;
    
    assert.equal(buy_context_info.basket.toString(), basket_1.toString());
    assert.equal(buy_context_info.reverting, 0);
    assert.equal(buy_context_info.usdcAmountLeft.toNumber(), worst_basket_price);
    assert.equal(buy_context_info.payingAccount.toString(), client_usdc_acc.toString());
    assert.equal(buy_context_info.userBasketTokenAccount.toString(), client_basket_token_acc.toString());
    assert.equal(buy_context_info.initialUsdcTransferAmount.toNumber(), worst_basket_price);
    for( let i = 0; i < basket_1_info.numberOfComponents; ++i)
    {
      const component : BasketComponent = basket_components[i];

      assert.equal(buy_context_info.tokensTreated[component.tokenIndex], 0);
      assert.equal(buy_context_info.tokenAmounts[component.tokenIndex].toNumber(), component.amount.toNumber()); // check that amount of tokens to transfer matches with component amount
    }
    // after init buy context, we have to initialize each token one by one.
    for(let x =  token_list.length - 1; x >= 0; --x)
    {
      const token = token_list[x];
      const market = markets_by_tokens[x];
      const [vault_signer, _vault_bump] = await serum_utils.findVaultOwner(
        market.publicKey
      );
      // {
      //   const amount_before_transaction = (await quote_token.getAccountInfo(quote_token_transaction_pool)).amount;
      //   mlog.log( "index " + x + " amount_before_transaction " + amount_before_transaction );
      // }
      await program.rpc.processTokenForContext(
        {
          accounts : {
            fruitbasketGroup : frt_bsk_group,
            buyContext : buy_context,
            tokenMint : token.publicKey,
            quoteTokenMint : quote_token.publicKey,
            fruitbasket : basket_1,
            market : market.publicKey,
            openOrders : open_orders_by_token[x].publicKey,
            requestQueue : market._decoded.requestQueue,
            eventQueue : market._decoded.eventQueue,
            bids : market._decoded.bids,
            asks: market._decoded.asks,
            tokenVault: market._decoded.baseVault,
            quoteTokenVault : market._decoded.quoteVault,
            vaultSigner : vault_signer,
            tokenPool : token_pools[x],
            quoteTokenTransactionPool : quote_token_transaction_pool,
            fruitBasketAuthority : fruitbasket_authority,
            dexProgram : serum.DEX_ID,
            tokenProgram : TOKEN_PROGRAM_ID,
            rent : web3.SYSVAR_RENT_PUBKEY,
          }
        }
      );

      // {
      //   const amount_after_transaction = (await quote_token.getAccountInfo(quote_token_transaction_pool)).amount;
      //   mlog.log( "index " + x + " amount_after_transaction " + amount_after_transaction );
      // }
    }


  });

  function ComponentInfo() {
    this.tokenIndex;
    this.amount;
    this.decimal;
  }

});
