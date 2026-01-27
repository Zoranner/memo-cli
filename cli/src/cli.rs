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

        /// Force embed even if similar memories exist (skip duplicate check)
        #[arg(short, long)]
        force: bool,

        /// Similarity threshold for duplicate detection (0.0-1.0, overrides config)
        #[arg(long = "dup-threshold")]
        dup_threshold: Option<f32>,

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

    #[command(about = "Update an existing memory")]
    Update {
        /// Memory ID to update
        id: String,

        /// New content for the memory
        #[arg(short, long)]
        content: String,

        /// New tags (comma-separated, replaces existing tags)
        #[arg(short = 't', long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,
    },

    #[command(about = "Delete a memory by ID")]
    Delete {
        /// Memory ID to delete
        id: String,

        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,

        /// Skip confirmation prompt (use with caution)
        #[arg(short, long)]
        force: bool,
    },

    #[command(about = "Merge multiple memories into one")]
    Merge {
        /// Memory IDs to merge (space-separated)
        ids: Vec<String>,

        /// Content for the merged memory
        #[arg(short, long)]
        content: String,

        /// Tags for the merged memory (comma-separated)
        #[arg(short = 't', long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        /// Use local database (./.memo/brain)
        #[arg(short, long)]
        local: bool,

        /// Use global database (~/.memo/brain)
        #[arg(short, long)]
        global: bool,
    },
}
