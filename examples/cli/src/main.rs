use jkcli::{JkArgument, JkCommand, JkFlag};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    isideload::init().expect("Failed to initialize error reporting");

    let root = JkCommand::new()
        .help("A simple CLI for isideload")
        .with_flag(
            JkFlag::new("debug")
                .with_short("d")
                .with_help("Enable debug output"),
        )
        .with_flag(
            JkFlag::new("version")
                .with_short("v")
                .with_short_circuit(|| {
                    // Print the version and exit
                    println!("isideload-cli version {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                })
        )
        .with_subcommand(
            "login",
            JkCommand::new().help("Logs in to your apple ID (used for testing, installation will do this automatically)")
                .with_argument(JkArgument::new().with_help("Apple ID Email").required(true))
                .with_argument(JkArgument::new().with_help("Apple ID Password").required(true)),
        )
        .with_subcommand("install", JkCommand::new().help("Installs an app to your device")
                .with_argument(JkArgument::new().with_help("Apple ID Email").required(true))
                .with_argument(JkArgument::new().with_help("Apple ID Password").required(true))
                .with_argument(JkArgument::new().with_help("Path to the app to install").required(true))
            )
        .subcommand_required(true);

    if let Some(args) = root.collect() {
        let level = if args.has_flag("debug") {
            Level::DEBUG
        } else {
            Level::INFO
        };
        let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        let (sub_name, sub_args) = args.first_subcommand().expect("No subcommand passed");
        let mut sub_args = sub_args.clone();

        match sub_name.as_str() {
            "login" => {
                let email: String = sub_args.next_argument().expect("No email provided");
                let password: String = sub_args.next_argument().expect("No password provided");
                println!(
                    "Logging in with email: {} and password: {}",
                    email, password
                );
            }
            "install" => {
                let email: String = sub_args.next_argument().expect("No email provided");
                let password: String = sub_args.next_argument().expect("No password provided");
                let app_path: String = sub_args.next_argument().expect("No app path provided");
                println!(
                    "Installing the app at {} with email: {} and password: {}",
                    app_path, email, password
                );
                todo!("Implement the install command");
            }
            _ => panic!("Unknown subcommand: {}", sub_name),
        }
    }
}
