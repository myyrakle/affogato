use clap::Args;
use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Command {
    #[clap(flatten)]
    pub value: CommandFlags,
}

#[derive(Clone, Debug, Default, Deserialize, Args)]
pub struct CommandFlags {
    #[clap(
        short,
        long,
        default_value = "false",
        help = "upgrade mode for no downtime replacement"
    )]
    upgrade: bool,

    #[clap(short, long, default_value = "4443", help = "port to listen on")]
    pub port: u16,

    #[clap(short, long, default_value = "0.0.0.0", help = "address to listen on")]
    pub address: String,
}

impl CommandFlags {
    pub fn is_uprade_mode(&self) -> bool {
        self.upgrade && cfg!(target_os = "linux")
    }
}

pub fn parse_command() -> Command {
    use clap::Parser;
    Command::parse()
}
