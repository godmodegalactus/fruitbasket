import * as anchor from '@project-serum/anchor';
import { Program, web3 } from '@project-serum/anchor';
import { Fruitbasket } from '../target/types/fruitbasket';
import {
  NATIVE_MINT,
  Token,
  TOKEN_PROGRAM_ID,
  AccountLayout as TokenAccountLayout,
  u64,
} from '@solana/spl-token';

import * as testutils from './utils/testutils';
import { token } from '@project-serum/anchor/dist/cjs/utils';
import mlog from 'mocha-logger';

type Connection = web3.Connection;

describe('fruitbasket', () => {

  // Configure the client to use the local cluster.
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);
  const connection = provider.connection;
  const wallet = provider.wallet;
  //configure test utils
  const test_utils = new testutils.TestUtils(connection, provider.wallet);

  const program = anchor.workspace.Fruitbasket as Program<Fruitbasket>;

  const owner = web3.Keypair.generate();

  // create some tokens
  const usdc = test_utils.createToken(6, wallet.publicKey);
  const btc = test_utils.createToken(6, wallet.publicKey);
  const eth = test_utils.createToken(6, wallet.publicKey);
  const sol = test_utils.createNativeToken();
  const alt1 = test_utils.createToken(6, wallet.publicKey);
  const alt2 = test_utils.createToken(6, wallet.publicKey);

  const mint_authority = web3.Keypair.generate();

  let frt_bsk_group = null;
  let frt_bsk_cache = null;
  it('Group initialized', async () => {
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(owner.publicKey, 10000000000),
      "confirmed"
    );

    const [tmp_group, bump_grp] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_group'), owner.publicKey.toBuffer()], program.programId);
    const [tmp_cache, bump_cache] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_cache'), owner.publicKey.toBuffer()], program.programId);

    frt_bsk_group = tmp_group;
    frt_bsk_cache = tmp_cache;
    mlog.log("group : " + frt_bsk_group);
    mlog.log("cache : " + frt_bsk_cache);
    await program.rpc.initializeGroup(
      bump_grp,
      bump_cache,
      (await usdc).publicKey,
      "usdc",
      {
        accounts:{
          owner: owner.publicKey,
          fruitBasketGrp: frt_bsk_group,
          cache: frt_bsk_cache,
          systemProgram: web3.SystemProgram.programId,
        },
        signers:[owner]
      }
    );
  });
});
