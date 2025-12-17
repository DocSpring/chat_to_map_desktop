/*!
 * ChatToMap CLI - Debugging and testing tool
 *
 * This CLI provides access to all core functionality for debugging
 * and testing without needing to run the full desktop app.
 *
 * Usage:
 *   cargo run --bin ctm-cli -- list-chats
 *   cargo run --bin ctm-cli -- list-chats --verbose
 *   cargo run --bin ctm-cli -- list-chats --limit 20
 */

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ctm-cli")]
#[command(about = "ChatToMap CLI - iMessage debugging tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all iMessage chats with contact resolution
    ListChats {
        /// Show verbose output including identifiers
        #[arg(short, long)]
        verbose: bool,

        /// Limit number of results (default: all)
        #[arg(short, long)]
        limit: Option<usize>,

        /// Filter by name or identifier (case-insensitive)
        #[arg(short, long)]
        filter: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show contacts index statistics
    Contacts {
        /// Show all contacts (verbose)
        #[arg(short, long)]
        verbose: bool,
    },

    /// Check Full Disk Access permission
    CheckAccess,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListChats {
            verbose,
            limit,
            filter,
            json,
        } => {
            cmd_list_chats(verbose, limit, filter, json);
        }
        Commands::Contacts { verbose } => {
            cmd_contacts(verbose);
        }
        Commands::CheckAccess => {
            cmd_check_access();
        }
    }
}

fn cmd_list_chats(verbose: bool, limit: Option<usize>, filter: Option<String>, json: bool) {
    match chat_to_map_desktop::list_chats() {
        Ok(mut chats) => {
            // Apply filter if provided
            if let Some(ref filter_str) = filter {
                let filter_lower = filter_str.to_lowercase();
                chats.retain(|c| {
                    c.display_name.to_lowercase().contains(&filter_lower)
                        || c.chat_identifier.to_lowercase().contains(&filter_lower)
                });
            }

            // Apply limit if provided
            if let Some(limit) = limit {
                chats.truncate(limit);
            }

            if json {
                println!("{}", serde_json::to_string_pretty(&chats).unwrap());
                return;
            }

            println!("Found {} chats\n", chats.len());

            for (i, chat) in chats.iter().enumerate() {
                let resolved = if chat.display_name != chat.chat_identifier {
                    " *"
                } else {
                    ""
                };

                if verbose {
                    println!(
                        "{:3}. {}{}\n     ID: {} | Service: {} | Participants: {} | Messages: {}\n",
                        i + 1,
                        chat.display_name,
                        resolved,
                        chat.chat_identifier,
                        chat.service,
                        chat.participant_count,
                        chat.message_count
                    );
                } else {
                    println!(
                        "{:3}. {}{} ({}) - {} messages",
                        i + 1,
                        chat.display_name,
                        resolved,
                        chat.service,
                        chat.message_count
                    );
                }
            }

            if !verbose {
                println!("\n(* = contact name resolved)");
                println!("Use --verbose for more details, --json for JSON output");
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_contacts(verbose: bool) {
    use chat_to_map_desktop::contacts::ContactsIndex;

    match ContactsIndex::build(None) {
        Ok(index) => {
            println!("Contacts index: {} entries", index.len());

            if verbose {
                println!("\nNote: Verbose contact listing not yet implemented");
                println!("The index maps phone numbers and emails to contact names.");
            }
        }
        Err(e) => {
            eprintln!("Error building contacts index: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_check_access() {
    use imessage_database::{tables::table::get_connection, util::dirs::default_db_path};

    let db_path = default_db_path();
    println!("iMessage database path: {:?}", db_path);

    if !db_path.exists() {
        println!("Status: Database file not found");
        println!("This may be a non-macOS system or Messages has never been used.");
        std::process::exit(1);
    }

    match get_connection(&db_path) {
        Ok(_) => {
            println!("Status: Full Disk Access GRANTED");
            println!("The CLI can read the iMessage database.");
        }
        Err(e) => {
            println!("Status: Full Disk Access DENIED");
            println!("Error: {}", e);
            println!("\nTo grant access:");
            println!("1. Open System Preferences > Privacy & Security > Full Disk Access");
            println!("2. Add your terminal application (Terminal, iTerm2, etc.)");
            std::process::exit(1);
        }
    }
}
