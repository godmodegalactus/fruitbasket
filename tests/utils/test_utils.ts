import { Wallet, BN } from "@project-serum/anchor";
import {
    NATIVE_MINT,
    Token,
    TOKEN_PROGRAM_ID,
    AccountLayout as TokenAccountLayout,
    u64,
} from "@solana/spl-token";
import {
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    SystemProgram,
    Transaction,
    TransactionInstruction,
} from "@solana/web3.js";
import { Pyth } from "./pyth";

export class TestUtils {
    public static readonly pythProgramId = Pyth.programId;

    public pyth: Pyth;

    private conn: Connection;
    private wallet: Wallet;
    private authority: Keypair;

    private recentBlockhash: string;

    constructor(conn: Connection, funded: Wallet) {
        this.conn = conn;
        this.wallet = funded;
        this.authority = this.wallet.payer;
        this.pyth = new Pyth(conn, funded);
    }

    async updateBlockhash() {
        this.recentBlockhash = (await this.conn.getRecentBlockhash()).blockhash;
    }

    payer(): Keypair {
        return this.wallet.payer;
    }

    connection(): Connection {
        return this.conn;
    }

    transaction(): Transaction {
        return new Transaction({
            feePayer: this.wallet.payer.publicKey,
            recentBlockhash: this.recentBlockhash,
        });
    }

    async createToken(
        decimals: number,
        authority: PublicKey = this.authority.publicKey
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

    async createWallet(lamports: number): Promise<Keypair> {
        const wallet = Keypair.generate();
        const fundTx = new Transaction().add(
            SystemProgram.transfer({
                fromPubkey: this.wallet.publicKey,
                toPubkey: wallet.publicKey,
                lamports,
            })
        );

        await this.sendAndConfirmTransaction(fundTx, [this.authority]);
        return wallet;
    }

    async createTokenAccount(
        token: Token,
        owner: PublicKey | HasPublicKey,
        amount: BN
    ): Promise<PublicKey> {
        if ("publicKey" in owner) {
            owner = owner.publicKey;
        }

        if (token.publicKey == NATIVE_MINT) {
            const account = await Token.createWrappedNativeAccount(
                this.conn,
                TOKEN_PROGRAM_ID,
                owner,
                this.authority,
                amount.toNumber()
            );
            return account;
        } else {
            const account = await token.createAccount(owner);
            if (amount.toNumber() > 0) {
                await token.mintTo(account, this.authority, [], amount.toNumber());
            }
            return account;
        }
    }

    async createTokenAccountTx(
        token: Token,
        owner: PublicKey | HasPublicKey,
        amount: number
    ): Promise<[PublicKey, Transaction]> {
        if ("publicKey" in owner) {
            owner = owner.publicKey;
        }

        let lamportBalanceNeeded =
            await Token.getMinBalanceRentForExemptAccount(this.conn);

        if (token.publicKey == NATIVE_MINT) {
            lamportBalanceNeeded += amount;
        }

        const newAccount = Keypair.generate();
        const transaction = this.transaction().add(
            SystemProgram.createAccount(
                toPublicKeys({
                    fromPubkey: this.wallet.payer,
                    newAccountPubkey: newAccount,
                    lamports: lamportBalanceNeeded,
                    space: TokenAccountLayout.span,
                    programId: TOKEN_PROGRAM_ID,
                })
            ),
            Token.createInitAccountInstruction(
                TOKEN_PROGRAM_ID,
                token.publicKey,
                newAccount.publicKey,
                owner
            )
        );

        transaction.sign(newAccount);
        return [newAccount.publicKey, transaction];
    }

    async findProgramAddress(
        programId: PublicKey,
        seeds: (HasPublicKey | ToBytes | Uint8Array | string)[]
    ): Promise<[PublicKey, number]> {
        const seed_bytes = seeds.map((s) => {
            if (typeof s == "string") {
                return Buffer.from(s);
            } else if ("publicKey" in s) {
                return s.publicKey.toBytes();
            } else if ("toBytes" in s) {
                return s.toBytes();
            } else {
                return s;
            }
        });
        return await PublicKey.findProgramAddress(seed_bytes, programId);
    }

    async sendAndConfirmTransaction(
        transaction: Transaction,
        signers: Signer[]
    ): Promise<string> {
        return await sendAndConfirmTransaction(
            this.conn,
            transaction,
            signers.concat(this.wallet.payer)
        );
    }

    async  sendAndConfirmTransactionSet(
        ...transactions: [Transaction, Signer[]][]
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

export function toBN(obj: any): any {
    if (typeof obj == "number") {
        return new BN(obj);
    } else if (typeof obj == "object") {
        const bnObj = {};

        for (const field in obj) {
            bnObj[field] = toBN(obj[field]);
        }

        return bnObj;
    }

    return obj;
}

export function toPublicKeys(
    obj: Record<string, string | PublicKey | HasPublicKey | any>
): any {
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

export function toBase58(
    obj: Record<string, string | PublicKey | HasPublicKey>
): any {
    const newObj = {};

    for (const key in obj) {
        const value = obj[key];

        if (value == undefined) {
            continue;
        } else if (typeof value == "string") {
            newObj[key] = value;
        } else if ("publicKey" in value) {
            newObj[key] = value.publicKey.toBase58();
        } else if ("toBase58" in value && typeof value.toBase58 == "function") {
            newObj[key] = value.toBase58();
        } else {
            newObj[key] = value;
        }
    }

    return newObj;
}

export class TestToken extends Token {
    decimals: number;

    constructor(conn: Connection, token: Token, decimals: number) {
        super(conn, token.publicKey, token.programId, token.payer);
        this.decimals = decimals;
    }

    amount(amount: u64 | number): u64 {
        if (typeof amount == "number") {
            amount = new u64(amount);
        }

        const one_unit = new u64(10).pow(new u64(this.decimals));
        const value = amount.mul(one_unit);

        return amount.mul(one_unit);
    }
}

interface ToBytes {
    toBytes(): Uint8Array;
}

interface HasPublicKey {
    publicKey: PublicKey;
}


export function getAmountDifference(beforeAmount: number, afterAmount: number): number {
    return afterAmount - beforeAmount;
}