use bitcoin::consensus::serialize;
use clap::Parser;
use bitcoinkernel::{ChainType, ChainstateManagerBuilder};
use rayon::prelude::*;
use std::path::PathBuf;

use bitcoin_slices::{bsl, Visit};
use electrs::db::{DBStore, Row, WriteBatch};
use electrs::index::scan_single_block_for_silent_payments_without_daemon;
use electrs::kernel::{create_context, setup_logging};
use log::info;

/// CLI tool for rebuilding the silent payment index. Note: requires that bitcoind is offline
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Data directory
    #[arg(long)]
    datadir: String,

    /// Electrs DB
    #[arg(long)]
    db: String,

    /// Network
    #[arg(long)]
    network: String,

    /// Birthday
    #[arg(long)]
    birthday: i32,
}

fn main() {
    let args = Args::parse();
    let chain_type = match args.network.to_lowercase().as_str() {
        "mainnet" => ChainType::Mainnet,
        "testnet" => ChainType::Testnet,
        "regtest" => ChainType::Regtest,
        "signet" => ChainType::Signet,
        _ => {
            eprintln!("Invalid network type: {}", args.network);
            std::process::exit(1);
        }
    };
    let data_dir = args.datadir;
    let blocks_dir = data_dir.clone() + "/blocks";

    // Set up the kernel
    let _ = setup_logging().unwrap();
    let context = create_context(chain_type);
    let chainman = ChainstateManagerBuilder::new(&context, &data_dir, &blocks_dir)
        .unwrap()
        .build()
        .unwrap();
    chainman.import_blocks().unwrap();

    // Open the database
    let store = DBStore::open(&PathBuf::from(args.db), None, false).unwrap();
    let batch_size = 2000;
    let tip_height = chainman.active_chain().height();
    let block_numbers = args.birthday..tip_height;
    block_numbers
        .collect::<Vec<_>>()
        .chunks(batch_size)
        .for_each(|chunk| {
            info!("indexing blocks {:?} to {:?}", chunk.first(), chunk.last(),);
            let mut batch = WriteBatch::default();
            let results: Vec<Row> = chunk
                .par_iter()
                .map(|&height| {
                    let block_index = chainman
                        .active_chain()
                        .at_height(height as usize)
                        .unwrap();
                    let raw_block: Vec<u8> = (&chainman.read_block_data(&block_index).unwrap())
                        .try_into()
                        .unwrap();
                    let undo = chainman.read_spent_outputs(&block_index).unwrap();
                    scan_single_block_for_silent_payments_without_daemon(
                        height as usize,
                        raw_block,
                        undo,
                        546,
                    )
                })
                .collect();
            let block_index = chainman
                .active_chain()
                .at_height(*chunk.last().unwrap() as usize)
                .unwrap();
            let last_block: Vec<u8> = (&chainman.read_block_data(&block_index).unwrap())
                .try_into()
                .unwrap();
            let block =
                bsl::Block::visit(&last_block, &mut bitcoin_slices::EmptyVisitor {}).unwrap();
            let block_hash = block.parsed().block_hash();
            batch.sp_tip_row = serialize(&block_hash).into_boxed_slice();
            batch.tweak_rows = results;
            store.write(&batch);
        });
}
