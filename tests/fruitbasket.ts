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
  const nb_tokens = 8;
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

  for( let i = 0; i < nb_tokens; ++i)
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

    for(let index = 0; index < nb_tokens; ++index){
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
    const exp = 1000000;
    let comp_btc = new ComponentInfo();
    comp_btc.tokenIndex = 1;
    comp_btc.amount = new anchor.BN(exp * 0.01); // 0.01 BTC
    comp_btc.decimal = 6;

    let comp_eth = new ComponentInfo();
    comp_eth.tokenIndex = 2;
    comp_eth.amount = new anchor.BN(exp * 0.1); // 0.1 ETC
    comp_eth.decimal = 6;

    let comp_sol = new ComponentInfo();
    comp_sol.tokenIndex = 3;
    comp_sol.amount = new anchor.BN(2 * web3.LAMPORTS_PER_SOL); // 2 SOL
    comp_sol.decimal = 9;

    let comp_srm = new ComponentInfo();
    comp_srm.tokenIndex = 4;
    comp_srm.amount = new anchor.BN(exp * 100); // 100 SRM
    comp_srm.decimal = 6;

    let comp_mngo = new ComponentInfo();
    comp_srm.tokenIndex = 5;
    comp_srm.amount = new anchor.BN(exp * 1000); // 1000 MNGO
    comp_srm.decimal = 6;

    let comp_sh1 = new ComponentInfo();
    comp_srm.tokenIndex = 6;
    comp_srm.amount = new anchor.BN(exp * 10000); // 10000 SHIT1
    comp_srm.decimal = 6;

    let comp_sh2 = new ComponentInfo();
    comp_srm.tokenIndex = 7;
    comp_srm.amount = new anchor.BN(exp * 100000); // 100000 SHIT1
    comp_srm.decimal = 6;

    // first basket
    let basket_nb = 0;
    const [basket_1, bump_b1] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket'), Buffer.from([basket_nb])], program.programId);
    const [basket_1_mint, bump_b1m] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_mint'), Buffer.from([basket_nb])], program.programId);
    const components_1 = [comp_btc, comp_eth, comp_sol];
    
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

    // second basket
    ++basket_nb;
    const [basket_2, bump_b2] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket'), Buffer.from([basket_nb])], program.programId);
    const [basket_2_mint, bump_b2m] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_mint'), Buffer.from([basket_nb])], program.programId);
    const components_2 = [comp_sol, comp_srm, comp_mngo];
    await program.rpc.addBasket(
      basket_nb,
      bump_b2,
      bump_b2m,
      "Solana coins",
      "Basket for coins base on solana",
      components_2,
      {
        accounts : {
          client : owner.publicKey,
          group : frt_bsk_group,
          basket : basket_2,
          basketMint : basket_2_mint,
          systemProgram : web3.SystemProgram.programId,
          tokenProgram : TOKEN_PROGRAM_ID,
          rent : web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [owner]
      }
    );

    // third basket
    ++basket_nb;
    const [basket_3, bump_b3] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket'), Buffer.from([basket_nb])], program.programId);
    const [basket_3_mint, bump_b3m] = await web3.PublicKey.findProgramAddress([Buffer.from('fruitbasket_mint'), Buffer.from([basket_nb])], program.programId);
    const components_3 = [comp_sh1, comp_sh2];
    await program.rpc.addBasket(
      basket_nb,
      bump_b3,
      bump_b3m,
      "Shit coins",
      "Basket for shit coins that have potential in future",
      components_3,
      {
        accounts : {
          client : owner.publicKey,
          group : frt_bsk_group,
          basket : basket_3,
          basketMint : basket_3_mint,
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
