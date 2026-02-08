use std::io::{self, Write};

use mac_app_remover::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("list") => list_apps(),
        Some("remove") => {
            if let Some(app_name) = args.get(2) {
                remove_app(app_name);
            } else {
                eprintln!("Uso: mac-app-remover remove <NomeDoApp>");
                eprintln!("Exemplo: mac-app-remover remove \"Google Chrome\"");
            }
        }
        Some("search") => {
            if let Some(query) = args.get(2) {
                search_apps(query);
            } else {
                eprintln!("Uso: mac-app-remover search <termo>");
            }
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("=== Mac App Remover ===");
    println!();
    println!("Uso:");
    println!("  mac-app-remover list               - Lista todos os aplicativos instalados");
    println!("  mac-app-remover search <termo>      - Busca aplicativos por nome");
    println!("  mac-app-remover remove <NomeDoApp>  - Remove um aplicativo e seus arquivos residuais");
    println!();
    println!("Exemplos:");
    println!("  mac-app-remover list");
    println!("  mac-app-remover search chrome");
    println!("  mac-app-remover remove \"Google Chrome\"");
}

fn list_apps() {
    let apps = get_installed_apps();
    println!("=== Aplicativos Instalados ({}) ===\n", apps.len());
    for (i, app) in apps.iter().enumerate() {
        let name = app.file_stem().unwrap_or_default().to_string_lossy();
        let size = dir_size(app).unwrap_or(0);
        println!("  {:>3}. {:<40} {}", i + 1, name, format_size(size));
    }
}

fn search_apps(query: &str) {
    let apps = get_installed_apps();
    let query_lower = query.to_lowercase();
    let matches: Vec<_> = apps
        .iter()
        .filter(|app| {
            app.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query_lower)
        })
        .collect();

    if matches.is_empty() {
        println!("Nenhum aplicativo encontrado para: \"{}\"", query);
        return;
    }

    println!(
        "=== Resultados para \"{}\" ({} encontrados) ===\n",
        query,
        matches.len()
    );
    for app in &matches {
        let name = app.file_stem().unwrap_or_default().to_string_lossy();
        let size = dir_size(app).unwrap_or(0);
        println!("  - {:<40} {}", name, format_size(size));
    }
}

fn remove_app(app_name: &str) {
    let app_path = match find_app(app_name) {
        Some(p) => p,
        None => {
            eprintln!("Aplicativo \"{}\" nao encontrado.", app_name);
            eprintln!("Use 'mac-app-remover search {}' para buscar.", app_name);
            return;
        }
    };

    let bundle_id = get_bundle_id(&app_path);
    let app_stem = app_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let related = find_related_files(&app_stem, bundle_id.as_deref());

    println!("=== Remover: {} ===\n", app_stem);
    let app_size = dir_size(&app_path).unwrap_or(0);
    println!(
        "  Aplicativo: {} ({})",
        app_path.display(),
        format_size(app_size)
    );

    if let Some(ref id) = bundle_id {
        println!("  Bundle ID:  {}", id);
    }

    if !related.is_empty() {
        println!("\n  Arquivos residuais encontrados:");
        let mut total_residual: u64 = 0;
        for path in &related {
            let size = if path.is_dir() {
                dir_size(path).unwrap_or(0)
            } else {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
            };
            total_residual += size;
            println!("    - {} ({})", path.display(), format_size(size));
        }
        println!(
            "\n  Total a ser removido: {}",
            format_size(app_size + total_residual)
        );
    } else {
        println!("\n  Nenhum arquivo residual encontrado.");
        println!("  Total a ser removido: {}", format_size(app_size));
    }

    print!("\nDeseja continuar com a remocao? (s/N): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    if !matches!(
        input.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    ) {
        println!("Operacao cancelada.");
        return;
    }

    if is_app_running(&app_stem) {
        print!("O aplicativo esta em execucao. Deseja fecha-lo? (s/N): ");
        io::stdout().flush().unwrap();
        let mut input2 = String::new();
        io::stdin().read_line(&mut input2).unwrap();
        if matches!(
            input2.trim().to_lowercase().as_str(),
            "s" | "sim" | "y" | "yes"
        ) {
            quit_app(&app_stem);
            std::thread::sleep(std::time::Duration::from_secs(2));
        } else {
            println!("Feche o aplicativo antes de remover.");
            return;
        }
    }

    let mut errors = Vec::new();
    print!("Removendo {}... ", app_path.display());
    io::stdout().flush().unwrap();
    match remove_path(&app_path) {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("ERRO: {}", e);
            errors.push(format!("{}: {}", app_path.display(), e));
        }
    }

    for path in &related {
        print!("Removendo {}... ", path.display());
        io::stdout().flush().unwrap();
        match remove_path(path) {
            Ok(_) => println!("OK"),
            Err(e) => {
                println!("ERRO: {}", e);
                errors.push(format!("{}: {}", path.display(), e));
            }
        }
    }

    println!();
    if errors.is_empty() {
        println!("\"{}\" removido com sucesso!", app_stem);
    } else {
        println!("\"{}\" removido com alguns erros:", app_stem);
        for e in &errors {
            eprintln!("  - {}", e);
        }
        eprintln!("\nDica: Alguns arquivos podem precisar de permissao de administrador.");
        eprintln!("Tente: sudo mac-app-remover remove \"{}\"", app_name);
    }
}
