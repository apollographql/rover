mod cli;

use structopt::StructOpt;

fn main() {
    let cli = cli::Apollo::from_args();
    println!("{:?}", cli);
}
