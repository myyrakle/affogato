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
    pub upgrade: bool,
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
