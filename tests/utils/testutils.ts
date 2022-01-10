
import * as anchor from '@project-serum/anchor';
import { Program, web3, Wallet } from '@project-serum/anchor';
import {
  NATIVE_MINT,
  Token,
  TOKEN_PROGRAM_ID,
  AccountLayout as TokenAccountLayout,
  u64,
} from '@solana/spl-token';

export class TestToken extends Token {
    decimals: number;
  
    constructor(conn: web3.Connection, token: Token, decimals: number) {
        super(conn, token.publicKey, token.programId, token.payer);
        this.decimals = decimals;
    }
  
    /**
     * Convert a token amount to the integer format for the mint
     * @param token The token mint
     * @param amount The amount of tokens
     */
    amount(amount: u64 | number): u64 {
        if (typeof amount == "number") {
            amount = new u64(amount);
        }
  
        const one_unit = new u64(10).pow(new u64(this.decimals));
        const value = amount.mul(one_unit);
  
        return amount.mul(one_unit);
    }
  }

  export class TestUtils{
    private conn : web3.Connection;
    private wallet : Wallet;
    private authority : web3.Keypair;

    constructor( connection : web3.Connection, wallet: Wallet ){
        this.conn = connection;
        this.wallet = wallet;
        this.authority = this.wallet.payer;
    }
    async createToken(
        decimals: number,
        authority: web3.PublicKey
    ): Promise<TestToken> {
        const token = await Token.createMint(
            this.conn,
            this.authority,
            authority,
            authority,
            decimals,
            TOKEN_PROGRAM_ID
        );

        return new TestToken(this.conn, token, decimals);
    }

    async createNativeToken() {
        const token = new Token(
            this.conn,
            NATIVE_MINT,
            TOKEN_PROGRAM_ID,
            this.authority
        );

        return new TestToken(this.conn, token, 9);
    }

  }
  