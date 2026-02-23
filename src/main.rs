use anyhow::Result;
use clap::Parser;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use strsim::jaro_winkler;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "This tool is a quick file searcher that uses parallel scanning and category filters to find your data.",
    long_about = "This tool is a high performance search utility that scans your file system in parallel to find exactly what you are looking for by using semantic matching and category filters."
)]
struct MainConfig {
    /// Name of the file or directory to search for.
    item_name: String,

    /// Filter search by a specific category.
    #[arg(short, long)]
    file_type: Option<String>,

    /// Display results using absolute system paths.
    #[arg(short, long)]
    output_absolute: bool,

    /// The starting directory for the search operation.
    #[arg(short, long, default_value = ".")]
    search_directory: String,
}

struct SearchContext {
    item_base: String,
    item_extension: String,
    item_name: String,
    type_extensions: Option<HashSet<String>>,
}

struct PrintConfig {
    item_name: String,
    exact_matches: Vec<PathBuf>,
    partial_matches: Vec<PathBuf>,
    all_paths: Vec<PathBuf>,
    output_absolute: bool,
    base_directory: PathBuf,
}

fn get_type_criteria(file_type: &str) -> Option<HashSet<String>> {
    let mut criteria_map = std::collections::HashMap::new();
    criteria_map.insert("audio", vec![".mp3", ".flac", ".ogg"]);
    criteria_map.insert("video", vec![".mp4", ".mkv", ".avi"]);
    criteria_map.insert("image", vec![".jpg", ".png", ".bmp"]);
    criteria_map.insert("text", vec![".cfg", ".md", ".log"]);
    criteria_map.insert("code", vec![".py", ".rs", ".kt"]);

    criteria_map.get(file_type).map(|extensions| {
        extensions
            .iter()
            .map(|extension| extension.to_string())
            .collect()
    })
}

fn file_matches(file_path: &Path, context: &SearchContext) -> bool {
    let file_name = match file_path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return false,
    };

    let file_extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext.to_lowercase()))
        .unwrap_or_default();

    let file_base = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    if let Some(extensions) = &context.type_extensions {
        if !context.item_extension.is_empty() {
            return file_name == format!("{}{}", context.item_base, context.item_extension);
        }
        return file_base == context.item_base && extensions.contains(&file_extension);
    }
    file_name == format!("{}{}", context.item_base, context.item_extension)
}

fn partial_match(
    path_item: &Path,
    search_term: &str,
    type_extensions: &Option<HashSet<String>>,
) -> bool {
    let name_result = path_item.file_name().and_then(|name| name.to_str());
    let name_lower = match name_result {
        Some(name) => name.to_lowercase(),
        None => return false,
    };

    let term_lower = search_term.to_lowercase();

    if let Some(extensions) = &type_extensions {
        let extension_result = path_item.extension().and_then(|ext| ext.to_str());
        let file_extension = match extension_result {
            Some(ext) => format!(".{}", ext.to_lowercase()),
            None => return false,
        };
        return name_lower.contains(&term_lower) && extensions.contains(&file_extension);
    }
    name_lower.contains(&term_lower)
}

fn format_path(found_path: &Path, print_config: &PrintConfig) -> String {
    if print_config.output_absolute {
        return found_path
            .canonicalize()
            .unwrap_or_else(|_| found_path.to_path_buf())
            .display()
            .to_string();
    }
    found_path
        .strip_prefix(&print_config.base_directory)
        .unwrap_or(found_path)
        .display()
        .to_string()
}

fn display_results(print_config: PrintConfig) {
    if !print_config.exact_matches.is_empty() {
        println!("\nExact matches found:");
        for match_item in &print_config.exact_matches {
            println!("  {}", format_path(match_item, &print_config));
        }
    } else {
        println!("\nNo exact match found.");
    }

    if !print_config.partial_matches.is_empty() {
        println!("\nPartial matches:");
        for match_item in &print_config.partial_matches {
            println!("  {}", format_path(match_item, &print_config));
        }
    } else {
        let mut suggestions: Vec<_> = print_config
            .all_paths
            .iter()
            .filter_map(|path| path.file_name()?.to_str())
            .filter(|name| jaro_winkler(&print_config.item_name, name) > 0.8)
            .collect();

        suggestions.sort_by(|a, b| {
            jaro_winkler(&print_config.item_name, b)
                .partial_cmp(&jaro_winkler(&print_config.item_name, a))
                .unwrap()
        });

        if !suggestions.is_empty() {
            println!("\nSimilar matches:");
            for suggestion in suggestions.iter().take(5) {
                println!("  {}", suggestion);
            }
        }
    }
}

fn main() -> Result<()> {
    let config = MainConfig::parse();
    let search_path = PathBuf::from(&config.search_directory);

    let type_extensions = config
        .file_type
        .as_ref()
        .and_then(|type_name| get_type_criteria(type_name));
    let item_path = Path::new(&config.item_name);

    let search_context = Arc::new(SearchContext {
        item_base: item_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string(),
        item_extension: item_path
            .extension()
            .map(|e| format!(".{}", e.to_str().unwrap()))
            .unwrap_or_default()
            .to_string(),
        item_name: config.item_name.clone(),
        type_extensions,
    });

    let exact_matches = Arc::new(Mutex::new(Vec::new()));
    let partial_matches = Arc::new(Mutex::new(Vec::new()));
    let all_seen_paths = Arc::new(Mutex::new(Vec::new()));

    let progress_bar = ProgressBar::new_spinner();
    progress_bar.set_style(
        ProgressStyle::default_spinner()
            .template(
                "{spinner:.cyan} [Radar] Scanning: {pos} items ({per_sec}) [{elapsed_precise}]",
            )?
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"),
    );

    // High-level Abstraction:
    // Parallelized multi-threaded traversal utilizing a progress tracking heartbeat.
    WalkBuilder::new(&search_path).build_parallel().run(|| {
        let context_ref = Arc::clone(&search_context);
        let exact_ref = Arc::clone(&exact_matches);
        let partial_ref = Arc::clone(&partial_matches);
        let all_ref = Arc::clone(&all_seen_paths);
        let progress_ref = progress_bar.clone();

        Box::new(move |entry_result| {
            if let Ok(entry) = entry_result {
                let path_entry = entry.path().to_path_buf();
                progress_ref.inc(1);

                {
                    let mut all_paths = all_ref.lock().unwrap();
                    all_paths.push(path_entry.clone());
                }

                let is_exact_name =
                    path_entry.file_name().and_then(|n| n.to_str()) == Some(&context_ref.item_name);

                if is_exact_name || file_matches(&path_entry, &context_ref) {
                    exact_ref.lock().unwrap().push(path_entry);
                } else if partial_match(
                    &path_entry,
                    &context_ref.item_base,
                    &context_ref.type_extensions,
                ) {
                    partial_ref.lock().unwrap().push(path_entry);
                }
            }
            ignore::WalkState::Continue
        })
    });

    progress_bar.finish_and_clear();

    let final_exact = Arc::try_unwrap(exact_matches)
        .expect("Exact matches reference still held")
        .into_inner()
        .expect("Mutex poisoned");
    let final_partial = Arc::try_unwrap(partial_matches)
        .expect("Partial matches reference still held")
        .into_inner()
        .expect("Mutex poisoned");
    let final_all = Arc::try_unwrap(all_seen_paths)
        .expect("All paths reference still held")
        .into_inner()
        .expect("Mutex poisoned");

    display_results(PrintConfig {
        item_name: config.item_name,
        exact_matches: final_exact,
        partial_matches: final_partial,
        all_paths: final_all,
        output_absolute: config.output_absolute,
        base_directory: search_path,
    });

    Ok(())
}
