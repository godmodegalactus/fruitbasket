import * as anchor from "@project-serum/anchor";
import { Market, DexInstructions } from "@project-serum/serum";
import * as web3 from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, u64 } from "@solana/spl-token";
import { Config } from "./config";
import { PublicKey } from "@solana/web3.js";
import { TestToken } from "./testutils";
export const DEX_ID = new PublicKey(
  "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"
);

//creates a order book
export interface CreateMarketInfo {
  token: TestToken;
  quoteToken: TestToken;
  tokenLotSize: number;
  quoteLotSize: number;
  feeRateBps: number;
}

export interface Order {
  price: number;
  size: number;
}

export class Serum {
  private config: Config;

  constructor(config: Config) {
    this.config = config;
  }

  public async createMarketsAndMakers(
    tokens: TestToken[],
    market_prices: bigint[],
    exps: number[]
  ): Promise<Market[]> {
    let quote_token = tokens[0];
    let other_tokens = tokens.slice(1);
    let markets = Promise.all(
      other_tokens.map(async (x) => {
        return this.createMarket({
          token: x,
          quoteToken: quote_token,
          quoteLotSize: 100000,
          tokenLotSize: 100000,
          feeRateBps: 30,
        });
      })
    );

    let maker1_tokens: [TestToken, anchor.BN][] = [];
    maker1_tokens.push([quote_token, new anchor.BN(10000000000)]);
    other_tokens.forEach((x) => maker1_tokens.push([x, new anchor.BN(10000)]));
    let maker_1 = await this.createMarketMaker(
      1 * web3.LAMPORTS_PER_SOL,
      maker1_tokens
    );

    let maker2_tokens: [TestToken, anchor.BN][] = [];
    maker2_tokens.push([quote_token, new anchor.BN(10000000000)]);
    other_tokens.forEach((x) =>
      maker2_tokens.push([x, new anchor.BN(1000000)])
    );
    let maker_2 = await this.createMarketMaker(
      1 * web3.LAMPORTS_PER_SOL,
      maker2_tokens
    );

    [...Array(other_tokens.length).keys()].map(async (x) => {
      let market_price = Number(market_prices[x + 1]) * 10 ** exps[x + 1];
      const bids = Maker.makeOrders([
        [market_price * 0.998, 100],
        [market_price * 0.995, 1000],
        [market_price * 0.99, 10000],
        [market_price * 0.9, 100000],
      ]);
      const asks = Maker.makeOrders([
        [market_price * 1.002, 100],
        [market_price * 1.005, 1000],
        [market_price * 1.01, 10000],
        [market_price * 1.1, 100000],
      ]);
      await maker_1.placeOrders(await markets[x], bids, asks);
      return await markets[x];
    });
    return markets;
  }

  public async createMarketMaker(
    lamports: number,
    tokens: [TestToken, anchor.BN][]
  ) {
    const account = await this.config.createWallet(lamports);
    const token_accounts = {};
    for (const [token, amount] of tokens) {
      const publicKey = await this.config.createTokenAccount(
        token,
        account.publicKey,
        amount
      );

      token_accounts[token.publicKey.toBase58()] = publicKey;
    }
    return new Maker(this.config, account, token_accounts);
  }

  public async createMarket(info: CreateMarketInfo): Promise<Market> {
    const market = web3.Keypair.generate();
    const requestQueue = web3.Keypair.generate();
    const eventQueue = web3.Keypair.generate();
    const bids = web3.Keypair.generate();
    const asks = web3.Keypair.generate();
    const quoteDustThreshold = new anchor.BN(100);

    const [vaultOwner, vaultOwnerBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from("market")],
        market.publicKey
      );

    const [tokenVault, quoteVault] = await Promise.all([
      this.config.createTokenAccount(info.token, vaultOwner, new anchor.BN(0)),
      this.config.createTokenAccount( info.quoteToken, vaultOwner, new anchor.BN(0)),
    ]);

    const initMarketTx = (await this.config.transaction()).add(
      await this.createAccount(
        market.publicKey,
        Market.getLayout(DEX_ID).span,
        DEX_ID
      ),
      await this.createAccount(requestQueue.publicKey, 5132, DEX_ID),
      await this.createAccount(eventQueue.publicKey, 262156, DEX_ID),
      await this.createAccount(bids.publicKey, 65548, DEX_ID),
      await this.createAccount(asks.publicKey, 65548, DEX_ID),
      DexInstructions.initializeMarket(
        this.getPublicKeys({
          market,
          requestQueue,
          eventQueue,
          bids,
          asks,
          tokenVault,
          quoteVault,
          baseMint: info.token.publicKey,
          quoteMint: info.quoteToken.publicKey,
          baseLotSize: new anchor.BN(info.tokenLotSize),
          quoteLotSize: new anchor.BN(info.quoteLotSize),
          feeRateBps: new anchor.BN(info.feeRateBps),
          vaultSignerNonce: new anchor.BN(vaultOwnerBump),
          quoteDustThreshold,
          programId: DEX_ID,
        })
      )
    );

    await this.config.sendAndConfirmTransaction(initMarketTx, [
      market,
      requestQueue,
      eventQueue,
      bids,
      asks,
    ]);

    return await Market.load(
      this.config.connection(),
      market.publicKey,
      undefined,
      DEX_ID
    );
  }

  private getPublicKeys(obj: Record<string, string | PublicKey | any>): any {
    const newObj = {};

    for (const key in obj) {
      const value = obj[key];

      if (typeof value == "string") {
        newObj[key] = new PublicKey(value);
      } else if (typeof value == "object" && "publicKey" in value) {
        newObj[key] = value.publicKey;
      } else {
        newObj[key] = value;
      }
    }

    return newObj;
  }

  private async createAccount(
    account: PublicKey,
    space: number,
    programId: PublicKey
  ): Promise<web3.TransactionInstruction> {
    return web3.SystemProgram.createAccount({
      newAccountPubkey: account,
      fromPubkey: this.config.payer().publicKey,
      lamports: await this.config
        .connection()
        .getMinimumBalanceForRentExemption(space),
      space,
      programId,
    });
  }
}

export class Maker {
  private config: Config;
  public account: web3.Keypair;
  public tokenAccounts: { [mint: string]: PublicKey };

  constructor(
    config: Config,
    account: web3.Keypair,
    tokenAccounts: { [mint: string]: PublicKey }
  ) {
    this.config = config;
    this.account = account;
    this.tokenAccounts = tokenAccounts;
  }

  static makeOrders(orders: [number, number][]): Order[] {
    return orders.map(([price, size]) => ({ price, size }));
  }

  async placeOrders(market: Market, bids: Order[], asks: Order[]) {
    const token_account = this.tokenAccounts[market.baseMintAddress.toBase58()];
    const quote_token_acc =
      this.tokenAccounts[market.quoteMintAddress.toBase58()];

    const placeOrderDefaultParams = {
      owner: this.account.publicKey,
      clientId: undefined,
      openOrdersAddressKey: undefined,
      openOrdersAccount: undefined,
      feeDiscountPubkey: null,
    };

    const ask_orders = [];
    const bid_orders = [];

    for (const entry of asks) {
      const { transaction, signers } = await market.makePlaceOrderTransaction(
        this.config.connection(),
        {
          payer: token_account,
          side: "sell",
          price: entry.price,
          size: entry.size,
          orderType: "postOnly",
          selfTradeBehavior: "abortTransaction",
          ...placeOrderDefaultParams,
        }
      );

      ask_orders.push([transaction, [this.account, ...signers]]);
    }

    for (const entry of bids) {
      const { transaction, signers } = await market.makePlaceOrderTransaction(
        this.config.connection(),
        {
          payer: quote_token_acc,
          side: "buy",
          price: entry.price,
          size: entry.size,
          orderType: "postOnly",
          selfTradeBehavior: "abortTransaction",
          ...placeOrderDefaultParams,
        }
      );

      bid_orders.push([transaction, [this.account, ...signers]]);
    }

    const signatures_ask = await Promise.all(
      ask_orders.map(([t, s]) => this.config.conn.sendTransaction(t, s))
    );
    await Promise.all(
      signatures_ask.map((s) => this.config.conn.confirmTransaction(s))
    );

    const signatures_bid = await Promise.all(
      ask_orders.map(([t, s]) => this.config.conn.sendTransaction(t, s))
    );
    await Promise.all(
      signatures_bid.map((s) => this.config.conn.confirmTransaction(s))
    );
  }
}
