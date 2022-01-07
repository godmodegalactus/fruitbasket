import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Fruitbasket } from '../target/types/fruitbasket';

describe('fruitbasket', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Fruitbasket as Program<Fruitbasket>;

  it('Is initialized!', async () => {
    // Add your test here.
    const tx = await program.rpc.initialize({});
    console.log("Your transaction signature", tx);
  });
});
