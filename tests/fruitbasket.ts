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
  const srm = test_utils.createToken(6, wallet.publicKey);
  const mngo = test_utils.createToken(6, wallet.publicKey);
  const shit1 = test_utils.createToken(6, wallet.publicKey);
  const shit2 = test_utils.createToken(6, wallet.publicKey);

  let tokens = [usdc, btc, eth, sol, srm, mngo, shit1, shit2];
  let token_names = ["USDC", "BTC", "ETH", "SOL", "SRM", "MNGO", "SHIT1", "SHIT2"];

  let price_oracles = [];
  let produce_oracles = [];

  for( let i = 0; i < 8; ++i)
  {
    price_oracles.push(web3.Keypair.generate());
    produce_oracles.push(web3.Keypair.generate());
  }

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
      "USDC",
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

  it( "Tokens added ", async() => {
    
    let token_pools = await Promise.all(tokens.map( async(x) => await (await x).createAccount(owner.publicKey)));

    for(let index = 0; index < 8; ++index){
      await program.rpc.addToken(
        token_names[index],
        {
          accounts : {
            owner: owner.publicKey,
            fruitBasketGrp: frt_bsk_group,
            mint : (await tokens[index]).publicKey,
            priceOracle : price_oracles[index].publicKey,
            productOracle : produce_oracles[index].publicKey,
            tokenPool: token_pools[index],
            tokenProgram : TOKEN_PROGRAM_ID,
          },
          signers : [owner],
        }
      );
       ++index;
    };
  } );

  it( "Baskets created ", async() => {
    const basket_nb = 0;
    const [basket_1, bump_b1] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket'), Buffer.from([basket_nb])], program.programId);
    const [basket_1_mint, bump_b1m] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_mint'), Buffer.from([basket_nb])], program.programId);
    let c1 = new ComponentInfo();
    c1.tokenIndex = 2;
    c1.amount = new anchor.BN(10000);
    c1.decimal = 6;
    let c2 = new ComponentInfo();
    c1.tokenIndex = 3;
    c1.amount = new anchor.BN(100000);
    c1.decimal = 6;
    const components_1 = [c1, c2];
    mlog.log("clinet : " + owner.publicKey);
    mlog.log("group : " + frt_bsk_group);
    mlog.log("basket : " + basket_1);
    mlog.log("basket_1_mint : " + basket_1_mint);
    
    await program.rpc.addBasket(
      basket_nb,
      bump_b1,
      bump_b1m,
      "First tier coins",
      "Basket for first teer coins",
      components_1,
      {
        accounts : {
          client : owner.publicKey,
          group : frt_bsk_group,
          basket : basket_1,
          basketMint : basket_1_mint,
          systemProgram : web3.SystemProgram.programId,
          tokenProgram : TOKEN_PROGRAM_ID,
          rent : web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner]
      }
    );
  } );

  function ComponentInfo() {
    this.tokenIndex;
    this.amount;
    this.decimal;
  }
});
