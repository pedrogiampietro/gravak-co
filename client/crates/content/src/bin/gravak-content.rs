//! `gravak-content`: builds and inspects the Gravak content.

use gravak_content::assets::Installation;
use gravak_content::build::{Merged, build, default_content_dir, default_installation};
use gravak_content::itemtype::{COLUMNS, decrypt, encrypt};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
gravak-content <comando> [opções]

Comandos:
  build                 junta content/ à instalação em content/build
  check                 confere content/ sem gerar nada
  items [texto]         lista os itens (todos, ou os que têm o texto no nome/ID)
  item <id>             mostra todas as colunas de um item
  decrypt <in> <out>    itemtype.dat -> texto
  encrypt <in> <out>    texto -> itemtype.dat

Opções:
  --assets <pasta>      instalação 5065 (padrão: GRAVAK_ASSETS ou R:\\5065)
  --content <pasta>     pasta content do repositório (padrão: encontrada a partir daqui)
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erro: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> gravak_content::Result<()> {
    let mut assets = default_installation();
    let mut content = default_content_dir();
    let mut positional = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--assets" => {
                assets = PathBuf::from(args.next().ok_or("--assets precisa de uma pasta")?)
            }
            "--content" => {
                content = PathBuf::from(args.next().ok_or("--content precisa de uma pasta")?)
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            _ => positional.push(arg),
        }
    }
    let Some(command) = positional.first() else {
        print!("{USAGE}");
        return Ok(());
    };
    match command.as_str() {
        "build" => {
            let mut installation = Installation::open(&assets)?;
            let report = build(&mut installation, &content)?;
            println!(
                "{} itens novos, {} alterados, {} ícones -> {}",
                report.added,
                report.changed,
                report.icons,
                report.output.display()
            );
        }
        "check" => {
            let mut installation = Installation::open(&assets)?;
            let merged = Merged::load(&mut installation, &content)?;
            let mut problems = 0;
            for def in merged.items.defs() {
                if let Some(icon) = &def.icon
                    && !content.join(icon).is_file()
                {
                    println!("item {}: ícone {icon} não encontrado", def.id);
                    problems += 1;
                }
            }
            println!(
                "{} definições, {} itens no resultado, {problems} problemas",
                merged.items.defs().count(),
                merged.merged.item_count()
            );
            if problems > 0 {
                return Err("content/ tem problemas".into());
            }
        }
        "items" => {
            let mut installation = Installation::open(&assets)?;
            let merged = Merged::load(&mut installation, &content)?;
            let filter = positional.get(1).map(|text| text.to_lowercase());
            for item in merged.merged.items() {
                let name = item.name();
                let id = item.id().to_string();
                if filter.as_ref().is_some_and(|filter| {
                    !name.to_lowercase().contains(filter) && !id.contains(filter)
                }) {
                    continue;
                }
                let mark = if merged.items.find(item.id()).is_some() {
                    "*"
                } else {
                    " "
                };
                println!("{mark}{id:>8}  {name}");
            }
        }
        "item" => {
            let id = positional
                .get(1)
                .and_then(|id| id.parse::<u32>().ok())
                .ok_or("item <id>")?;
            let mut installation = Installation::open(&assets)?;
            let merged = Merged::load(&mut installation, &content)?;
            let item = merged.merged.get(id).ok_or("item não encontrado")?;
            for (index, column) in COLUMNS.iter().enumerate() {
                println!(
                    "{index:>2} {:<18} {:<32} {}",
                    column.key,
                    column.label,
                    item.value(index)
                );
            }
            for index in COLUMNS.len()..item.len() {
                println!(
                    "{index:>2} {:<18} {:<32} {}",
                    "-",
                    "(coluna extra)",
                    item.text(index)
                );
            }
        }
        "decrypt" | "encrypt" => {
            let (Some(input), Some(output)) = (positional.get(1), positional.get(2)) else {
                return Err(format!("{command} <entrada> <saída>").into());
            };
            let mut bytes = std::fs::read(input)?;
            if command == "decrypt" {
                decrypt(&mut bytes);
            } else {
                encrypt(&mut bytes);
            }
            std::fs::write(output, bytes)?;
        }
        other => return Err(format!("comando desconhecido `{other}`\n\n{USAGE}").into()),
    }
    Ok(())
}
