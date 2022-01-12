import * as anchor from "@project-serum/anchor";
import * as web3 from "@solana/web3.js";
import { Wallet } from "@project-serum/anchor";
import {
    Connection,
    Keypair,
    SystemProgram,
    Transaction,
} from "@solana/web3.js";
import { Token, TOKEN_PROGRAM_ID, NATIVE_MINT, u64 } from "@solana/spl-token";
import {PublicKey} from "@solana/web3.js";


const writer = anchor.workspace.WriterUtils;
// config class
export class Config {
    static readonly programId = writer.programId;

    conn: Connection;
    wallet: Wallet;

    constructor(conn: Connection, wallet: Wallet) {
        this.conn = conn;
        this.wallet = wallet;
    }

    async createAccount(space: number): Promise<Keypair> {
        const newAccount = Keypair.generate();
        const createTx = new Transaction().add(
            SystemProgram.createAccount({
                fromPubkey: this.wallet.publicKey,
                newAccountPubkey: newAccount.publicKey,
                programId: writer.programId,
                lamports: await this.conn.getMinimumBalanceForRentExemption(
                    space
                ),
                space,
            })
        );

        await web3.sendAndConfirmTransaction(this.conn, createTx, [
            this.wallet.payer,
            newAccount,
        ]);
        return newAccount;
    }

    async createWallet(lamports: number): Promise<Keypair> {
        const wallet = Keypair.generate();
        await this.conn.confirmTransaction(
            await this.conn.requestAirdrop(wallet.publicKey, lamports),
            "confirmed"
          );
        return wallet;
    }

    async createTokenAccount(
        token: Token,
        owner: PublicKey,
        amount: anchor.BN
    ): Promise<PublicKey> {
        if (token.publicKey == NATIVE_MINT) {
            const account = await Token.createWrappedNativeAccount(
                this.conn,
                TOKEN_PROGRAM_ID,
                owner,
                this.wallet.payer,
                amount.toNumber()
            );
            return account;
        } else {
            const account = await token.createAccount(owner);
            if (amount.toNumber() > 0) {
                await token.mintTo(account, this.wallet.payer, [], amount.toNumber());
            }
            return account;
        }
    }

    async store(account: Keypair, offset: number, input: Buffer) {
        const writeInstr = writer.instruction.write(
            new anchor.BN(offset),
            input,
            {
                accounts: { target: account.publicKey },
            }
        );
        const writeTx = new Transaction({
            feePayer: this.wallet.publicKey,
        }).add(writeInstr);

        await web3.sendAndConfirmTransaction(this.conn, writeTx, [
            account,
            this.wallet.payer,
        ]);
    }

    payer(): Keypair {
        return this.wallet.payer;
    }

    connection(): Connection {
        return this.conn;
    }

    async transaction(): Promise<Transaction> {
        return new Transaction({
            feePayer: this.wallet.payer.publicKey,
            recentBlockhash: (await this.conn.getRecentBlockhash()).blockhash,
        });
    }

    async sendAndConfirmTransaction(
        transaction: Transaction,
        signers: web3.Signer[]
    ): Promise<string> {
        return await web3.sendAndConfirmTransaction(
            this.conn,
            transaction,
            signers.concat(this.payer())
        );
    }

    async sendAndConfirmTransactionSet(
        ...transactions: [Transaction, web3.Signer[]][]
    ): Promise<string[]> {
        const signatures = await Promise.all(
            transactions.map(([t, s]) =>
                this.conn.sendTransaction(t, s)
            )
        );
        const result = await Promise.all(
            signatures.map((s) => this.conn.confirmTransaction(s))
        );

        const failedTx = result.filter((r) => r.value.err != null);

        if (failedTx.length > 0) {
            throw new Error(`Transactions failed: ${failedTx}`);
        }

        return signatures;
    }
}
