import * as anchor from "@project-serum/anchor";
import * as web3 from "@solana/web3.js";
import { Wallet } from "@project-serum/anchor";
import {
    Connection,
    Keypair,
    PublicKey,
    SystemProgram,
    Transaction,
    TransactionInstruction,
} from "@solana/web3.js";


const writer = anchor.workspace.WriterUtils;

export class DataManager {
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
}
