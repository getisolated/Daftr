//! CLI du spike : `listen` (A, écoute + annonce + code) et `pair` (B, découvre et se lie).

mod cli;
mod listen;
mod pair;

fn main() -> anyhow::Result<()> {
    let command = cli::parse(std::env::args().skip(1).collect())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    match command {
        cli::Command::Listen(options) => runtime.block_on(listen::run(options)),
        cli::Command::Pair(options) => runtime.block_on(pair::run(options)),
    }
}
