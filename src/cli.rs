use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "memo")]
#[command(about = "Vector-based memo system with semantic search", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Initialize memo configuration (optional, auto-init on first use)")]
    Init {
        /// Initialize in local directory (./.memo) instead of global (~/.memo)
        #[arg(short, long)]
        local: bool,
    },

    #[command(about = "Embed text or markdown file to vector database")]
    Embed {
        /// Text string, markdown file path, or directory path to embed
        input: String,

        /// Tags for the memory (comma-separated, e.g., "rust,cli,important")
        #[arg(short = 't', long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,
    },

    #[command(about = "Search memories by semantic similarity")]
    Search {
        query: String,

        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,

        #[arg(short = 't', long, default_value = "0.7")]
        threshold: f32,

        /// Filter by date after (format: YYYY-MM-DD or YYYY-MM-DD HH:MM)
        #[arg(long)]
        after: Option<String>,

        /// Filter by date before (format: YYYY-MM-DD or YYYY-MM-DD HH:MM)
        #[arg(long)]
        before: Option<String>,

        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,
    },

    #[command(about = "List all memories")]
    List {
        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,
    },

    #[command(about = "Clear all memories (DANGEROUS operation)")]
    Clear {
        /// Clear local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Clear global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,

        /// Skip confirmation prompt (use with caution)
        #[arg(short, long)]
        force: bool,
    },
}
